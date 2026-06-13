use layra_core::{Direction, Edge, EdgeKind, EdgeStyle, Graph, Node, Size};

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
        animated: false,
    }
}

#[test]
fn lr_wide_nodes_no_main_axis_overlap() {
    let mut g = Graph::new(Direction::LeftRight);
    let a = g.add_node(node("a", 200.0, 30.0));
    let b = g.add_node(node("b", 200.0, 30.0));
    g.add_edge(edge(a, b));
    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());
    let r0 = g.nodes[0].rect;
    let r1 = g.nodes[1].rect;
    eprintln!("r0={:?}\nr1={:?}", r0, r1);
    assert!(r1.x >= r0.x + r0.width, "LR overlap: {:?} vs {:?}", r0, r1);
}
