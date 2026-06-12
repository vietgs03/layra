//! Connected-component decomposition.
//!
//! Disconnected subgraphs (e.g. two independent clusters in one diagram)
//! must not share the Sugiyama run: their layers interleave on the cross
//! axis and cluster rects end up overlapping. Instead each weakly
//! connected component is laid out alone, then components are packed
//! side-by-side along the cross axis with a gap.
//!
//! Nodes sharing a subgraph are treated as connected even without edges,
//! so a cluster is never split across components.

use crate::LayoutOptions;
use layra_core::{Direction, Graph, Point, Rect};

const COMPONENT_GAP: f32 = 56.0;

/// Returns `false` when the graph is a single component (caller runs the
/// normal pipeline); `true` when it decomposed, laid out, and packed.
pub(crate) fn layout_componentwise(graph: &mut Graph, options: &LayoutOptions) -> bool {
    let n = graph.nodes.len();

    // Union-find over edges + subgraph membership.
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let union = |parent: &mut [usize], a: usize, b: usize| {
        let (ra, rb) = (find(parent, a), find(parent, b));
        if ra != rb {
            parent[ra] = rb;
        }
    };

    for e in &graph.edges {
        union(&mut parent, e.source.index(), e.target.index());
    }
    for sg in &graph.subgraphs {
        for w in sg.nodes.windows(2) {
            union(&mut parent, w[0].index(), w[1].index());
        }
    }

    // Bucket nodes by root.
    let mut comp_of = vec![0usize; n];
    let mut roots: Vec<usize> = Vec::new();
    #[allow(clippy::needless_range_loop)] // index used for both lookup and write
    for i in 0..n {
        let r = find(&mut parent, i);
        let idx = match roots.iter().position(|&x| x == r) {
            Some(idx) => idx,
            None => {
                roots.push(r);
                roots.len() - 1
            }
        };
        comp_of[i] = idx;
    }
    if roots.len() < 2 {
        return false;
    }

    // Build a sub-Graph per component, preserving index maps.
    let ncomp = roots.len();
    let mut node_map: Vec<Vec<usize>> = vec![Vec::new(); ncomp]; // comp → original node idx
    for (i, &c) in comp_of.iter().enumerate() {
        node_map[c].push(i);
    }

    let horizontal = matches!(graph.direction, Direction::LeftRight | Direction::RightLeft);

    let mut cross_cursor = 0.0f32;
    #[allow(clippy::needless_range_loop)] // c indexes three parallel structures
    for c in 0..ncomp {
        let mut sub = Graph::new(graph.direction);
        // local index lookup
        let mut local_of = vec![usize::MAX; n];
        for (li, &gi) in node_map[c].iter().enumerate() {
            local_of[gi] = li;
            sub.add_node(graph.nodes[gi].clone());
        }
        let mut edge_map: Vec<usize> = Vec::new(); // local edge → original edge
        for (ei, e) in graph.edges.iter().enumerate() {
            if comp_of[e.source.index()] == c {
                let mut e2 = e.clone();
                e2.source = layra_core::NodeId(local_of[e.source.index()] as u32);
                e2.target = layra_core::NodeId(local_of[e.target.index()] as u32);
                sub.add_edge(e2);
                edge_map.push(ei);
            }
        }
        let mut sg_map: Vec<usize> = Vec::new();
        for (si, sg) in graph.subgraphs.iter().enumerate() {
            if sg.nodes.first().is_some_and(|id| comp_of[id.index()] == c) {
                let mut s2 = sg.clone();
                s2.nodes = sg
                    .nodes
                    .iter()
                    .map(|id| layra_core::NodeId(local_of[id.index()] as u32))
                    .collect();
                sub.add_subgraph(s2);
                sg_map.push(si);
            }
        }
        // Re-point node.parent at local subgraph indices.
        for node in &mut sub.nodes {
            if let Some(p) = node.parent {
                node.parent = sg_map
                    .iter()
                    .position(|&orig| orig == p.index())
                    .map(|local| layra_core::SubgraphId(local as u32));
            }
        }

        crate::layout_single(&mut sub, options);

        // Pack: shift this component to the running cross-axis cursor.
        let b = sub.bounds();
        let (dx, dy) = if horizontal {
            (0.0, cross_cursor - b.y)
        } else {
            (cross_cursor - b.x, 0.0)
        };
        cross_cursor += if horizontal { b.height } else { b.width } + COMPONENT_GAP;

        let shift_rect = |r: Rect| -> Rect { Rect::new(r.x + dx, r.y + dy, r.width, r.height) };
        for (li, &gi) in node_map[c].iter().enumerate() {
            graph.nodes[gi].rect = shift_rect(sub.nodes[li].rect);
        }
        for (le, &ge) in edge_map.iter().enumerate() {
            graph.edges[ge].points = sub.edges[le]
                .points
                .iter()
                .map(|p| Point::new(p.x + dx, p.y + dy))
                .collect();
            graph.edges[ge].label_pos = sub.edges[le]
                .label_pos
                .map(|p| Point::new(p.x + dx, p.y + dy));
        }
        for (ls, &gs) in sg_map.iter().enumerate() {
            graph.subgraphs[gs].rect = shift_rect(sub.subgraphs[ls].rect);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use layra_core::{Direction, Edge, EdgeKind, EdgeStyle, Graph, Node, Size, Subgraph};

    fn node(name: &str) -> Node {
        let mut n = Node::new(name, name);
        n.size = Size {
            width: 100.0,
            height: 40.0,
        };
        n
    }

    #[test]
    fn disconnected_clusters_do_not_overlap() {
        // Two clusters with no edges between them (the LB diagram shape).
        let mut g = Graph::new(Direction::LeftRight);
        let a1 = g.add_node(node("a1"));
        let a2 = g.add_node(node("a2"));
        let b1 = g.add_node(node("b1"));
        let b2 = g.add_node(node("b2"));
        for (s, t) in [(a1, a2), (b1, b2)] {
            g.add_edge(Edge {
                source: s,
                target: t,
                label: None,
                style: EdgeStyle::Solid,
                kind: EdgeKind::Arrow,
                points: vec![],
                label_pos: None,
                end_labels: None,
            });
        }
        let sga = g.add_subgraph(Subgraph {
            name: "A".into(),
            label: "A".into(),
            nodes: vec![a1, a2],
            parent: None,
            rect: Default::default(),
        });
        let sgb = g.add_subgraph(Subgraph {
            name: "B".into(),
            label: "B".into(),
            nodes: vec![b1, b2],
            parent: None,
            rect: Default::default(),
        });
        g.nodes[a1.index()].parent = Some(sga);
        g.nodes[a2.index()].parent = Some(sga);
        g.nodes[b1.index()].parent = Some(sgb);
        g.nodes[b2.index()].parent = Some(sgb);

        crate::layout(&mut g, &crate::LayoutOptions::default());

        let ra = g.subgraphs[0].rect;
        let rb = g.subgraphs[1].rect;
        assert!(
            !ra.intersects(&rb),
            "disconnected clusters overlap: {ra:?} vs {rb:?}"
        );
    }
}
