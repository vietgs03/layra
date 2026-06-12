//! # layra-layout
//!
//! Sugiyama-framework layered layout for DAG-like diagrams.
//!
//! Pipeline:
//! 1. **Cycle breaking** — greedy DFS edge reversal so layering sees a DAG.
//! 2. **Layer assignment** — longest-path (O(V+E)).
//! 3. **Virtual nodes** — long edges are split so every edge spans one layer.
//! 4. **Crossing minimization** — barycenter sweeps with adaptive rounds.
//! 5. **Coordinate assignment** — barycenter-relaxation positioning
//!    (Brandes-Köpf planned as an upgrade).
//!
//! Input: a [`Graph`] whose nodes already carry measured [`Size`]s.
//! Output: the same graph with every node's `rect` filled in.

mod bk;
mod crossing;
mod layering;
mod position;

use layra_core::Graph;

/// Tunable layout parameters. Defaults match an editorial diagram look.
#[derive(Debug, Clone, Copy)]
pub struct LayoutOptions {
    /// Gap between adjacent nodes within a layer.
    pub node_spacing: f32,
    /// Gap between layers.
    pub rank_spacing: f32,
    /// Padding inside subgraph clusters.
    pub cluster_padding: f32,
    /// Maximum barycenter sweep rounds.
    pub max_sweeps: usize,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            node_spacing: 56.0,
            rank_spacing: 72.0,
            cluster_padding: 24.0,
            max_sweeps: 8,
        }
    }
}

/// Internal working structure shared by layout phases.
pub(crate) struct LayoutGraph {
    /// adjacency: for each node, outgoing neighbor indices (after cycle break)
    pub succ: Vec<Vec<usize>>,
    pub pred: Vec<Vec<usize>>,
    /// layer index per node (real nodes only at entry; virtual nodes appended)
    pub layer: Vec<usize>,
    /// node sizes including virtual nodes (virtual = zero size)
    pub sizes: Vec<(f32, f32)>,
    /// number of real nodes; indices >= this are virtual
    pub real_count: usize,
    /// per-edge chain of node indices traversed (real source, virtuals..., real target)
    pub edge_chains: Vec<Vec<usize>>,
    /// nodes per layer, in current order
    pub layers: Vec<Vec<usize>>,
    /// final x center, y center per node
    pub pos: Vec<(f32, f32)>,
    /// innermost cluster per node (virtual nodes inherit None)
    pub cluster: Vec<Option<u32>>,
}

/// Lay out `graph` in place: fills `node.rect`, `subgraph.rect`, and seeds
/// each edge's `points` with the layered waypoints (the router refines them).
pub fn layout(graph: &mut Graph, options: &LayoutOptions) {
    if graph.nodes.is_empty() {
        return;
    }

    let mut lg = layering::build(graph, options);
    layering::assign_layers(&mut lg);
    layering::insert_virtual_nodes(&mut lg, graph);
    crossing::minimize(&mut lg, options.max_sweeps);
    position::assign_coordinates(&mut lg, options);
    position::apply(graph, &lg, options);
}

#[cfg(test)]
mod tests {
    use super::*;
    use layra_core::{Direction, Edge, EdgeKind, EdgeStyle, Node, Size};

    fn node(name: &str, w: f32, h: f32) -> Node {
        let mut n = Node::new(name, name);
        n.size = Size::new(w, h);
        n
    }

    fn edge(source: layra_core::NodeId, target: layra_core::NodeId) -> Edge {
        Edge {
            source,
            target,
            label: None,
            style: EdgeStyle::Solid,
            kind: EdgeKind::Arrow,
            points: vec![],
            label_pos: None,
        }
    }

    #[test]
    fn linear_chain_layers_progress() {
        let mut g = Graph::new(Direction::TopBottom);
        let a = g.add_node(node("a", 100.0, 40.0));
        let b = g.add_node(node("b", 100.0, 40.0));
        let c = g.add_node(node("c", 100.0, 40.0));
        g.add_edge(edge(a, b));
        g.add_edge(edge(b, c));

        layout(&mut g, &LayoutOptions::default());

        let ya = g.node(a).rect.y;
        let yb = g.node(b).rect.y;
        let yc = g.node(c).rect.y;
        assert!(ya < yb && yb < yc, "layers must progress: {ya} {yb} {yc}");
    }

    #[test]
    fn no_overlap_within_layer() {
        let mut g = Graph::new(Direction::TopBottom);
        let root = g.add_node(node("root", 80.0, 40.0));
        let kids: Vec<_> = (0..5)
            .map(|i| g.add_node(node(&format!("k{i}"), 120.0, 40.0)))
            .collect();
        for &k in &kids {
            g.add_edge(edge(root, k));
        }

        layout(&mut g, &LayoutOptions::default());

        for i in 0..kids.len() {
            for j in (i + 1)..kids.len() {
                let a = g.node(kids[i]).rect;
                let b = g.node(kids[j]).rect;
                assert!(!a.intersects(&b), "{:?} overlaps {:?}", a, b);
            }
        }
    }

    #[test]
    fn cycle_does_not_hang() {
        let mut g = Graph::new(Direction::LeftRight);
        let a = g.add_node(node("a", 80.0, 40.0));
        let b = g.add_node(node("b", 80.0, 40.0));
        g.add_edge(edge(a, b));
        g.add_edge(edge(b, a));
        layout(&mut g, &LayoutOptions::default());
        // Just verifying termination + distinct layers.
        assert_ne!(g.node(a).rect, g.node(b).rect);
    }

    #[test]
    fn left_right_direction_flows_horizontally() {
        let mut g = Graph::new(Direction::LeftRight);
        let a = g.add_node(node("a", 100.0, 40.0));
        let b = g.add_node(node("b", 100.0, 40.0));
        g.add_edge(edge(a, b));
        layout(&mut g, &LayoutOptions::default());
        assert!(g.node(a).rect.x < g.node(b).rect.x);
    }
}
