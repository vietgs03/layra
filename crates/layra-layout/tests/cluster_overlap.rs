//! Regression: subgraph cluster rects must not overlap non-member nodes.
//! Found by reviewer agent on demo.svg (Data Plane vs Notification Svc,
//! 14.5px overlap) — cluster padding was applied post-hoc without layout
//! reserving room for it.

use layra_core::{Direction, Edge, EdgeKind, EdgeStyle, Graph, Node, Size, Subgraph};

fn node(name: &str) -> Node {
    let mut n = Node::new(name, name);
    n.size = Size {
        width: 110.0,
        height: 38.0,
    };
    n
}

fn edge(a: layra_core::NodeId, b: layra_core::NodeId, dashed: bool) -> Edge {
    Edge {
        source: a,
        target: b,
        label: None,
        style: if dashed {
            EdgeStyle::Dashed
        } else {
            EdgeStyle::Solid
        },
        kind: EdgeKind::Arrow,
        points: vec![],
        label_pos: None,
        end_labels: None,
        animated: false,
    }
}

#[test]
fn cluster_rect_does_not_overlap_outside_nodes() {
    // Mirrors the demo topology: mq inside the cluster feeds notif outside it.
    let mut g = Graph::new(Direction::LeftRight);
    let api = g.add_node(node("api"));
    let mq = g.add_node(node("mq"));
    let notif = g.add_node(node("notif"));
    g.add_edge(edge(api, mq, false));
    g.add_edge(edge(mq, notif, true));

    let sg = g.add_subgraph(Subgraph {
        name: "data".into(),
        label: "Data Plane".into(),
        nodes: vec![mq],
        parent: None,
        rect: Default::default(),
    });
    g.nodes[mq.index()].parent = Some(sg);

    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());

    let cluster = g.subgraphs[0].rect;
    for &outside in &[api, notif] {
        let r = g.node(outside).rect;
        assert!(
            !cluster.intersects(&r),
            "cluster {cluster:?} overlaps outside node {r:?}"
        );
    }
}
