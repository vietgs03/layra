//! L3 contract: nested subgraph/cluster layout.
//!
//! A cluster nested inside another must have its rect fully enclosed by the
//! parent's rect, with at least the cluster padding of margin on every side
//! (proper nested pills). Two-level nesting is the minimum bar; the engine
//! must compute cluster rects nesting-aware, not from direct members only.

use layra_core::{Direction, Edge, EdgeKind, EdgeStyle, Graph, Node, Rect, Size, Subgraph};

fn node(g: &mut Graph, name: &str) -> layra_core::NodeId {
    let mut n = Node::new(name, name);
    n.size = Size {
        width: 100.0,
        height: 40.0,
    };
    g.add_node(n)
}

fn link(g: &mut Graph, a: layra_core::NodeId, b: layra_core::NodeId) {
    g.add_edge(Edge {
        source: a,
        target: b,
        label: None,
        style: EdgeStyle::Solid,
        kind: EdgeKind::Arrow,
        points: vec![],
        label_pos: None,
        end_labels: None,
        animated: false,
    });
}

/// `inner` must sit fully inside `outer` with `pad` margin all round.
fn assert_nested(outer: Rect, inner: Rect, pad: f32) {
    let m = pad - 0.5; // tolerance
    assert!(
        outer.x + m <= inner.x
            && outer.y + m <= inner.y
            && inner.right() + m <= outer.right()
            && inner.bottom() + m <= outer.bottom(),
        "inner {inner:?} not nested inside outer {outer:?} with padding {pad}"
    );
}

#[test]
fn two_level_nested_clusters_are_enclosed() {
    // outer = { hub, inner = { a, b } }
    let mut g = Graph::new(Direction::TopBottom);
    let hub = node(&mut g, "hub");
    let a = node(&mut g, "a");
    let b = node(&mut g, "b");
    link(&mut g, hub, a);
    link(&mut g, a, b);

    let inner = g.add_subgraph(Subgraph {
        name: "inner".into(),
        label: "Inner".into(),
        nodes: vec![a, b],
        parent: None, // set below
        rect: Default::default(),
    });
    let outer = g.add_subgraph(Subgraph {
        name: "outer".into(),
        label: "Outer".into(),
        nodes: vec![hub], // direct member only; `inner` nests within
        parent: None,
        rect: Default::default(),
    });
    g.subgraphs[inner.index()].parent = Some(outer);
    g.nodes[hub.index()].parent = Some(outer);
    g.nodes[a.index()].parent = Some(inner);
    g.nodes[b.index()].parent = Some(inner);

    let opts = layra_layout::LayoutOptions::default();
    layra_layout::layout(&mut g, &opts);

    let outer_rect = g.subgraphs[outer.index()].rect;
    let inner_rect = g.subgraphs[inner.index()].rect;
    assert!(
        outer_rect.width > 0.0 && outer_rect.height > 0.0,
        "outer empty"
    );
    assert!(
        inner_rect.width > 0.0 && inner_rect.height > 0.0,
        "inner empty"
    );
    assert_nested(outer_rect, inner_rect, opts.cluster_padding);

    // The inner cluster must enclose its own member nodes too.
    for &id in &[a, b] {
        let r = g.node(id).rect;
        assert!(
            inner_rect.x <= r.x && inner_rect.right() >= r.right(),
            "inner cluster does not enclose member {r:?}"
        );
    }
}
