//! Cluster members must stay contiguous within each layer — otherwise the
//! cluster rect swallows unrelated nodes sitting between them.

use layra_core::*;

fn node(name: &str) -> Node {
    let mut n = Node::new(name, name);
    n.size = Size {
        width: 90.0,
        height: 36.0,
    };
    n
}

fn edge(a: NodeId, b: NodeId) -> Edge {
    Edge {
        source: a,
        target: b,
        label: None,
        style: EdgeStyle::Solid,
        kind: EdgeKind::Arrow,
        points: vec![],
        label_pos: None,
        end_labels: None,
    }
}

#[test]
fn cluster_members_are_contiguous_and_unswallowed() {
    // root fans out to: in1, out_a, in2, out_b  (interleaved on purpose)
    // in1+in2 belong to a cluster; naive barycenter leaves out_a between them.
    let mut g = Graph::new(Direction::TopBottom);
    let root = g.add_node(node("root"));
    let in1 = g.add_node(node("in1"));
    let out_a = g.add_node(node("out_a"));
    let in2 = g.add_node(node("in2"));
    let out_b = g.add_node(node("out_b"));
    for &k in &[in1, out_a, in2, out_b] {
        g.add_edge(edge(root, k));
    }
    let sg = g.add_subgraph(Subgraph {
        name: "c".into(),
        label: "Cluster".into(),
        nodes: vec![in1, in2],
        parent: None,
        rect: Rect::default(),
    });
    g.nodes[in1.index()].parent = Some(sg);
    g.nodes[in2.index()].parent = Some(sg);

    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());

    let cluster = g.subgraphs[0].rect;
    for &outsider in &[out_a, out_b, root] {
        let r = g.node(outsider).rect;
        assert!(
            !cluster.intersects(&r),
            "cluster {cluster:?} swallowed outsider {:?} {r:?}",
            g.node(outsider).name
        );
    }
}
