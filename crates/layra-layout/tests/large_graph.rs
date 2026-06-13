//! L7 contract: large layered graphs must lay out cleanly — no node-node
//! overlap and a bounded overall aspect ratio (not a pathological sliver).

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

/// Build a connected layered graph: `layers` layers of `per_layer` nodes,
/// every node wired to two nodes in the next layer (a dense DAG).
fn layered_graph(layers: usize, per_layer: usize) -> Graph {
    let mut g = Graph::new(Direction::TopBottom);
    let mut ids = Vec::with_capacity(layers * per_layer);
    for l in 0..layers {
        for k in 0..per_layer {
            // Varied widths to stress coordinate assignment.
            let w = 80.0 + ((l + k) % 4) as f32 * 30.0;
            ids.push(g.add_node(node(&format!("n{l}_{k}"), w, 40.0)));
        }
    }
    for l in 0..layers - 1 {
        for k in 0..per_layer {
            let src = ids[l * per_layer + k];
            let a = ids[(l + 1) * per_layer + k];
            let b = ids[(l + 1) * per_layer + (k + 1) % per_layer];
            g.add_edge(edge(src, a));
            g.add_edge(edge(src, b));
        }
    }
    g
}

/// True iff two rects overlap with more than `tol` interpenetration on both
/// axes (touching edges are fine).
fn overlaps(a: &layra_core::Rect, b: &layra_core::Rect, tol: f32) -> bool {
    let ax2 = a.x + a.width;
    let ay2 = a.y + a.height;
    let bx2 = b.x + b.width;
    let by2 = b.y + b.height;
    let x_overlap = (ax2.min(bx2) - a.x.max(b.x)).max(0.0);
    let y_overlap = (ay2.min(by2) - a.y.max(b.y)).max(0.0);
    x_overlap > tol && y_overlap > tol
}

/// Minimum cross-axis (width, for TB) a layer strictly needs: sum of member
/// widths plus one `node_spacing` gap between each adjacent pair. The widest
/// layer sets the lower bound for the whole diagram's width.
fn min_required_width(g: &Graph, layers: usize, per_layer: usize, spacing: f32) -> f32 {
    let mut widest = 0.0f32;
    for l in 0..layers {
        let mut sum = 0.0f32;
        for k in 0..per_layer {
            sum += g.nodes[l * per_layer + k].size.width;
        }
        sum += spacing * (per_layer as f32 - 1.0);
        widest = widest.max(sum);
    }
    widest
}

#[test]
fn synthetic_200_node_graph_has_no_overlap_and_bounded_aspect() {
    // 200 nodes: 20 layers x 10.
    let mut g = layered_graph(20, 10);
    assert_eq!(g.nodes.len(), 200);

    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());

    // No node-node overlap. O(n^2) is fine for 200 nodes in a test.
    let mut worst = (0usize, 0usize, 0.0f32);
    for i in 0..g.nodes.len() {
        for j in (i + 1)..g.nodes.len() {
            let a = g.nodes[i].rect;
            let b = g.nodes[j].rect;
            let ax2 = a.x + a.width;
            let ay2 = a.y + a.height;
            let bx2 = b.x + b.width;
            let by2 = b.y + b.height;
            let xo = (ax2.min(bx2) - a.x.max(b.x)).max(0.0);
            let yo = (ay2.min(by2) - a.y.max(b.y)).max(0.0);
            let pen = xo.min(yo);
            if pen > worst.2 {
                worst = (i, j, pen);
            }
            assert!(
                !overlaps(&a, &b, 0.5),
                "nodes {} and {} overlap: {:?} vs {:?}",
                g.nodes[i].name,
                g.nodes[j].name,
                a,
                b
            );
        }
    }

    // Bounded aspect ratio: neither dimension may be more than 8x the other.
    let bounds = g.bounds();
    let aspect = bounds.width.max(bounds.height) / bounds.width.min(bounds.height);
    assert!(
        aspect <= 8.0,
        "aspect ratio {aspect:.2} too extreme ({:.0}x{:.0})",
        bounds.width,
        bounds.height
    );
}

#[test]
fn wide_layered_graph_stays_compact() {
    // A wide-and-shallow graph (5 layers x 40) is intrinsically wide, so the
    // aspect ratio cannot be bounded without overlapping. The meaningful
    // quality bar is *width inflation*: the laid-out width must stay close to
    // the minimum a layer strictly requires (no pathological stretching).
    let mut g = layered_graph(5, 40);
    assert_eq!(g.nodes.len(), 200);
    let opts = layra_layout::LayoutOptions::default();
    let min_w = min_required_width(&g, 5, 40, opts.node_spacing);
    layra_layout::layout(&mut g, &opts);

    for i in 0..g.nodes.len() {
        for j in (i + 1)..g.nodes.len() {
            assert!(
                !overlaps(&g.nodes[i].rect, &g.nodes[j].rect, 0.5),
                "overlap between {} and {}",
                g.nodes[i].name,
                g.nodes[j].name
            );
        }
    }

    let bounds = g.bounds();
    let inflation = bounds.width / min_w;
    assert!(
        inflation <= 1.4,
        "width inflation {inflation:.2}x too high (width {:.0}, min {:.0})",
        bounds.width,
        min_w
    );
}

#[test]
fn deep_layered_graph_compacts_width() {
    // 20 x 10: compaction should keep the diagram near its minimum width
    // rather than the ~1.7x Brandes-Köpf alone produces.
    let mut g = layered_graph(20, 10);
    let opts = layra_layout::LayoutOptions::default();
    let min_w = min_required_width(&g, 20, 10, opts.node_spacing);
    layra_layout::layout(&mut g, &opts);
    let inflation = g.bounds().width / min_w;
    assert!(
        inflation <= 1.4,
        "deep graph width inflation {inflation:.2}x too high"
    );
}
