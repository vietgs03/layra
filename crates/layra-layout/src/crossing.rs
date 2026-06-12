//! Phase 4: crossing minimization via barycenter sweeps.
//!
//! Classic median/barycenter heuristic: sweep down then up, reordering each
//! layer by the mean position of its neighbors in the fixed adjacent layer.
//! Stops early when an entire sweep changes nothing.

use crate::LayoutGraph;

pub(crate) fn minimize(lg: &mut LayoutGraph, max_sweeps: usize) {
    if lg.layers.len() < 2 {
        enforce_cluster_contiguity(lg);
        return;
    }

    // position-in-layer index for every node
    let mut order = vec![0usize; lg.layer.len()];
    for layer in &lg.layers {
        for (i, &n) in layer.iter().enumerate() {
            order[n] = i;
        }
    }

    // Scratch buffer reused across sorts (avoids per-layer alloc).
    let mut keyed: Vec<(f64, usize)> = Vec::new();

    for _ in 0..max_sweeps {
        let mut changed = false;

        // Downward sweep: order layer i by predecessors in layer i-1.
        for i in 1..lg.layers.len() {
            changed |= sort_by_barycenter(&mut lg.layers[i], &lg.pred, &mut order, &mut keyed);
        }
        // Upward sweep: order layer i by successors in layer i+1.
        for i in (0..lg.layers.len() - 1).rev() {
            changed |= sort_by_barycenter(&mut lg.layers[i], &lg.succ, &mut order, &mut keyed);
        }

        if !changed {
            break;
        }
    }

    enforce_cluster_contiguity(lg);
}

/// Subgraph-aware constraint: members of the same cluster must be adjacent
/// within each layer, or the cluster rect will swallow whatever node sits
/// between them. Stable regroup: each cluster block moves to the position
/// of its first member; relative order inside and outside is preserved.
fn enforce_cluster_contiguity(lg: &mut LayoutGraph) {
    use std::collections::HashMap;

    for layer in &mut lg.layers {
        if layer.len() < 3 {
            continue;
        }
        // Quick check: is any cluster split across the layer?
        let mut last_seen: HashMap<u32, usize> = HashMap::new();
        let mut split = false;
        for (i, &n) in layer.iter().enumerate() {
            if let Some(c) = lg.cluster[n] {
                if let Some(&prev) = last_seen.get(&c) {
                    if i - prev > 1 {
                        split = true;
                        break;
                    }
                }
                last_seen.insert(c, i);
            }
        }
        if !split {
            continue;
        }

        // Rebuild: walk the layer; at a cluster's first member, splice in
        // every member of that cluster (preserving their relative order).
        let mut rebuilt = Vec::with_capacity(layer.len());
        let mut emitted: HashMap<u32, bool> = HashMap::new();
        for &n in layer.iter() {
            match lg.cluster[n] {
                Some(c) => {
                    if !emitted.get(&c).copied().unwrap_or(false) {
                        emitted.insert(c, true);
                        rebuilt.extend(layer.iter().copied().filter(|&m| lg.cluster[m] == Some(c)));
                    }
                }
                None => rebuilt.push(n),
            }
        }
        *layer = rebuilt;
    }
}

/// Sort `layer` by mean neighbor index. Nodes without neighbors keep their
/// current relative position (barycenter = own index). Updates `order` for
/// just this layer (the only one whose positions changed). Returns whether
/// the order changed.
fn sort_by_barycenter(
    layer: &mut [usize],
    neighbors: &[Vec<usize>],
    order: &mut [usize],
    keyed: &mut Vec<(f64, usize)>,
) -> bool {
    keyed.clear();
    keyed.extend(layer.iter().map(|&n| {
        let ns = &neighbors[n];
        let key = if ns.is_empty() {
            order[n] as f64
        } else {
            ns.iter().map(|&m| order[m] as f64).sum::<f64>() / ns.len() as f64
        };
        (key, n)
    }));

    keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut changed = false;
    for (slot, &(_, n)) in keyed.iter().enumerate() {
        if layer[slot] != n {
            changed = true;
        }
        layer[slot] = n;
        order[n] = slot;
    }
    changed
}
