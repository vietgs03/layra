//! Phase 5: coordinate assignment + writing results back to the IR graph.
//!
//! Cross-axis positions come from Brandes-Köpf alignment (see `bk.rs`),
//! giving straight virtual chains and tight, balanced columns. A safety
//! pass restores minimum gaps in the rare degenerate case.

use crate::{bk, LayoutGraph, LayoutOptions};
use layra_core::{Direction, Graph, Point, Rect};

/// Compute (main-axis, cross-axis) center coordinates for every node.
/// Main axis = layer progression; cross axis = within-layer spread.
pub(crate) fn assign_coordinates(lg: &mut LayoutGraph, options: &LayoutOptions) {
    let n = lg.layer.len();
    lg.pos = vec![(0.0, 0.0); n];

    // -- Main axis: stack layers, each as tall as its tallest node. --
    let mut main = 0.0f32;
    let mut layer_main = Vec::with_capacity(lg.layers.len());
    for layer in &lg.layers {
        let extent = layer
            .iter()
            .map(|&i| lg.sizes[i].1)
            .fold(0.0f32, f32::max)
            .max(1.0);
        layer_main.push(main + extent / 2.0);
        main += extent + options.rank_spacing;
    }
    for (li, layer) in lg.layers.iter().enumerate() {
        for &i in layer {
            lg.pos[i].1 = layer_main[li];
        }
    }

    // -- Cross axis: Brandes-Köpf four-pass alignment. --
    let widths: Vec<f32> = lg.sizes.iter().map(|s| s.0.max(8.0)).collect();
    let xs = bk::assign_x(&bk::BkInput {
        layers: &lg.layers,
        pred: &lg.pred,
        succ: &lg.succ,
        widths: &widths,
        real_count: lg.real_count,
        spacing: options.node_spacing,
    });
    for (i, &x) in xs.iter().enumerate() {
        lg.pos[i].0 = x;
    }

    // Safety: BK guarantees separation within blocks; enforce the layer
    // invariant once more to be robust against degenerate inputs.
    for layer in &lg.layers {
        resolve_overlaps(layer, &lg.sizes, &mut lg.pos, options.node_spacing);
    }
}

/// Restore minimum gaps in a layer without changing order: forward pass
/// pushes right, backward pass pushes left, average centers the run.
fn resolve_overlaps(layer: &[usize], sizes: &[(f32, f32)], pos: &mut [(f32, f32)], gap: f32) {
    if layer.len() < 2 {
        return;
    }
    let half = |i: usize| sizes[i].0.max(8.0) / 2.0;

    let desired: Vec<f32> = layer.iter().map(|&i| pos[i].0).collect();

    // Forward: enforce left-to-right minimum separation.
    let mut fwd = desired.clone();
    for k in 1..layer.len() {
        let min_x = fwd[k - 1] + half(layer[k - 1]) + gap + half(layer[k]);
        if fwd[k] < min_x {
            fwd[k] = min_x;
        }
    }
    // Backward: same from the right.
    let mut bwd = desired;
    for k in (0..layer.len() - 1).rev() {
        let max_x = bwd[k + 1] - half(layer[k + 1]) - gap - half(layer[k]);
        if bwd[k] > max_x {
            bwd[k] = max_x;
        }
    }
    for (k, &i) in layer.iter().enumerate() {
        pos[i].0 = (fwd[k] + bwd[k]) / 2.0;
    }
}

/// Map abstract (cross, main) coordinates into final rects honoring the
/// requested direction, normalize to a small positive origin, fill subgraph
/// bounds, and seed edge waypoints from virtual-node chains.
pub(crate) fn apply(graph: &mut Graph, lg: &LayoutGraph, options: &LayoutOptions) {
    let to_xy = |cross: f32, main: f32| -> (f32, f32) {
        match graph.direction {
            Direction::TopBottom => (cross, main),
            Direction::BottomTop => (cross, -main),
            Direction::LeftRight => (main, cross),
            Direction::RightLeft => (-main, cross),
        }
    };

    // Compute raw positions for real nodes. Note: `lg.sizes` is in abstract
    // (cross, main) space, so use the graph's screen-space sizes here.
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let raw: Vec<(f32, f32)> = (0..lg.layer.len())
        .map(|i| {
            let (cx, cy) = to_xy(lg.pos[i].0, lg.pos[i].1);
            if i < lg.real_count {
                let size = graph.nodes[i].size;
                min_x = min_x.min(cx - size.width / 2.0);
                min_y = min_y.min(cy - size.height / 2.0);
            } else {
                min_x = min_x.min(cx);
                min_y = min_y.min(cy);
            }
            (cx, cy)
        })
        .collect();

    const MARGIN: f32 = 16.0;
    let dx = MARGIN - min_x;
    let dy = MARGIN - min_y;

    for (i, node) in graph.nodes.iter_mut().enumerate() {
        let (cx, cy) = raw[i];
        node.rect = Rect::new(
            cx + dx - node.size.width / 2.0,
            cy + dy - node.size.height / 2.0,
            node.size.width,
            node.size.height,
        );
    }

    // Seed edge polylines through their virtual-node chains.
    for (e, chain) in graph.edges.iter_mut().zip(&lg.edge_chains) {
        e.points = chain
            .iter()
            .map(|&i| {
                let (cx, cy) = raw[i];
                Point::new(cx + dx, cy + dy)
            })
            .collect();
    }

    // Subgraph bounds = union of member rects + padding.
    let node_rects: Vec<Rect> = graph.nodes.iter().map(|n| n.rect).collect();
    for sg in &mut graph.subgraphs {
        let mut iter = sg.nodes.iter().map(|id| node_rects[id.index()]);
        if let Some(first) = iter.next() {
            let bounds = iter.fold(first, |acc, r| acc.union(&r));
            sg.rect = bounds.inflate(options.cluster_padding);
        }
    }
}
