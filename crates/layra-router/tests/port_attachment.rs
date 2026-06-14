//! L8 contract: port-aware edge attachment. Edges must leave/enter nodes
//! from the side facing the other endpoint (right/left for LR, top/bottom for
//! TB), and the attachment point along that side should shift toward the other
//! endpoint instead of always sitting at the border center.

use layra_core::{Direction, Edge, EdgeKind, EdgeStyle, Graph, Node, Rect, Size};

fn node(name: &str, w: f32, h: f32) -> Node {
    let mut n = Node::new(name, name);
    n.size = Size {
        width: w,
        height: h,
    };
    n
}

fn edge(a: layra_core::NodeId, b: layra_core::NodeId) -> Edge {
    Edge {
        source: a,
        target: b,
        label: None,
        style: EdgeStyle::Solid,
        kind: EdgeKind::Arrow,
        points: vec![],
        label_pos: None,
        end_labels: None,
        crowfoot: None,
        animated: false,
    }
}

/// Place a node at an explicit rect (bypassing layout) so the router sees a
/// known geometry.
fn place(g: &mut Graph, id: layra_core::NodeId, r: Rect) {
    g.nodes[id.index()].rect = r;
    g.nodes[id.index()].size = Size {
        width: r.width,
        height: r.height,
    };
}

#[test]
fn lr_chain_endpoints_on_facing_sides() {
    // Full pipeline on a simple LR chain: each edge leaves the right side of
    // its source and enters the left side of its target (within tolerance).
    let mut g = Graph::new(Direction::LeftRight);
    let a = g.add_node(node("a", 80.0, 40.0));
    let b = g.add_node(node("b", 80.0, 40.0));
    let c = g.add_node(node("c", 80.0, 40.0));
    g.add_edge(edge(a, b));
    g.add_edge(edge(b, c));
    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());
    layra_router::route(&mut g);

    const TOL: f32 = 1.5;
    for e in &g.edges {
        let src = g.nodes[e.source.index()].rect;
        let dst = g.nodes[e.target.index()].rect;
        let start = e.points[0];
        let end = *e.points.last().unwrap();
        assert!(
            (start.x - src.right()).abs() <= TOL,
            "edge must start on src right edge: start.x={} src.right={}",
            start.x,
            src.right()
        );
        assert!(
            (end.x - dst.x).abs() <= TOL,
            "edge must end on dst left edge: end.x={} dst.x={}",
            end.x,
            dst.x
        );
        // y stays within the node's vertical extent (it's a real border point).
        assert!(start.y >= src.y - TOL && start.y <= src.bottom() + TOL);
        assert!(end.y >= dst.y - TOL && end.y <= dst.bottom() + TOL);
    }
}

#[test]
fn lr_port_shifts_toward_offset_target() {
    // Source at top-left, target down-and-to-the-right. The source's right
    // port should sit *below* its vertical center (facing the lower target),
    // and the target's left port should sit *above* its center (facing the
    // higher source). Center-only attachment (the old behavior) would put
    // both at the node midline.
    let mut g = Graph::new(Direction::LeftRight);
    let a = g.add_node(node("a", 40.0, 40.0));
    let b = g.add_node(node("b", 40.0, 40.0));
    let e = g.add_edge(edge(a, b));
    place(&mut g, a, Rect::new(0.0, 0.0, 40.0, 40.0)); // center y = 20
    place(&mut g, b, Rect::new(200.0, 120.0, 40.0, 40.0)); // center y = 140
    layra_router::route(&mut g);

    let edge = &g.edges[e.index()];
    let start = edge.points[0];
    let end = *edge.points.last().unwrap();

    // Still on the facing sides.
    assert!(
        (start.x - 40.0).abs() <= 1.5,
        "start on src right: {start:?}"
    );
    assert!((end.x - 200.0).abs() <= 1.5, "end on dst left: {end:?}");

    // Port-aware: source port shifted down toward the lower target.
    assert!(
        start.y > 24.0,
        "source port should face the lower target (y>center 20), got {}",
        start.y
    );
    // Target port shifted up toward the higher source.
    assert!(
        end.y < 136.0,
        "target port should face the higher source (y<center 140), got {}",
        end.y
    );
    // Both still within the node borders.
    assert!(start.y <= 40.0, "src port stays on border");
    assert!(end.y >= 120.0, "dst port stays on border");
}

#[test]
fn tb_port_shifts_toward_offset_target() {
    // Same idea for top-bottom: source above-left, target below-right. The
    // source's bottom port shifts right toward the target; the target's top
    // port shifts left toward the source.
    let mut g = Graph::new(Direction::TopBottom);
    let a = g.add_node(node("a", 40.0, 40.0));
    let b = g.add_node(node("b", 40.0, 40.0));
    let e = g.add_edge(edge(a, b));
    place(&mut g, a, Rect::new(0.0, 0.0, 40.0, 40.0)); // center x = 20
    place(&mut g, b, Rect::new(160.0, 200.0, 40.0, 40.0)); // center x = 180
    layra_router::route(&mut g);

    let edge = &g.edges[e.index()];
    let start = edge.points[0];
    let end = *edge.points.last().unwrap();

    // On the facing sides (src bottom, dst top).
    assert!(
        (start.y - 40.0).abs() <= 1.5,
        "start on src bottom: {start:?}"
    );
    assert!((end.y - 200.0).abs() <= 1.5, "end on dst top: {end:?}");

    // Port-aware: source bottom port shifted right toward the target.
    assert!(
        start.x > 24.0,
        "source port should face the right target (x>center 20), got {}",
        start.x
    );
    assert!(
        end.x < 176.0,
        "target port should face the left source (x<center 180), got {}",
        end.x
    );
    assert!(start.x <= 40.0 && end.x >= 160.0, "ports stay on borders");
}

#[test]
fn aligned_nodes_attach_at_center() {
    // When source and target share a cross-axis position, port-aware
    // attachment degenerates to the side center (no spurious offset).
    let mut g = Graph::new(Direction::LeftRight);
    let a = g.add_node(node("a", 40.0, 40.0));
    let b = g.add_node(node("b", 40.0, 40.0));
    let e = g.add_edge(edge(a, b));
    place(&mut g, a, Rect::new(0.0, 0.0, 40.0, 40.0)); // center y = 20
    place(&mut g, b, Rect::new(200.0, 0.0, 40.0, 40.0)); // center y = 20
    layra_router::route(&mut g);

    let edge = &g.edges[e.index()];
    let start = edge.points[0];
    let end = *edge.points.last().unwrap();
    assert!((start.y - 20.0).abs() <= 1.5, "aligned src port centered");
    assert!((end.y - 20.0).abs() <= 1.5, "aligned dst port centered");
}
