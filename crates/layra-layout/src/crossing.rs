//! Phase 4: crossing minimization via barycenter sweeps.
//!
//! Classic median/barycenter heuristic: sweep down then up, reordering each
//! layer by the mean position of its neighbors in the fixed adjacent layer.
//! Stops early when an entire sweep changes nothing.

use crate::LayoutGraph;

pub(crate) fn minimize(lg: &mut LayoutGraph, max_sweeps: usize) {
    if lg.layers.len() < 2 {
        return;
    }

    // position-in-layer index for every node
    let mut order = vec![0usize; lg.layer.len()];
    let rebuild_order = |layers: &[Vec<usize>], order: &mut [usize]| {
        for layer in layers {
            for (i, &n) in layer.iter().enumerate() {
                order[n] = i;
            }
        }
    };
    rebuild_order(&lg.layers, &mut order);

    for _ in 0..max_sweeps {
        let mut changed = false;

        // Downward sweep: order layer i by predecessors in layer i-1.
        for i in 1..lg.layers.len() {
            changed |= sort_by_barycenter(&mut lg.layers[i], &lg.pred, &order);
            rebuild_order(&lg.layers, &mut order);
        }
        // Upward sweep: order layer i by successors in layer i+1.
        for i in (0..lg.layers.len() - 1).rev() {
            changed |= sort_by_barycenter(&mut lg.layers[i], &lg.succ, &order);
            rebuild_order(&lg.layers, &mut order);
        }

        if !changed {
            break;
        }
    }
}

/// Sort `layer` by mean neighbor index. Nodes without neighbors keep their
/// current relative position (barycenter = own index). Returns whether the
/// order changed.
fn sort_by_barycenter(layer: &mut [usize], neighbors: &[Vec<usize>], order: &[usize]) -> bool {
    let mut keyed: Vec<(f64, usize)> = layer
        .iter()
        .map(|&n| {
            let ns = &neighbors[n];
            let key = if ns.is_empty() {
                order[n] as f64
            } else {
                ns.iter().map(|&m| order[m] as f64).sum::<f64>() / ns.len() as f64
            };
            (key, n)
        })
        .collect();

    let before: Vec<usize> = keyed.iter().map(|&(_, n)| n).collect();
    keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut changed = false;
    for (slot, &(_, n)) in keyed.iter().enumerate() {
        if before[slot] != n {
            changed = true;
        }
        layer[slot] = n;
    }
    changed
}
