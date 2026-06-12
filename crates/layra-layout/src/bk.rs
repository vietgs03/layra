//! Brandes-Köpf horizontal coordinate assignment.
//!
//! "Fast and Simple Horizontal Coordinate Assignment" (Brandes & Köpf,
//! GD 2001), with the published errata applied. Four alignment passes
//! (up/down × left/right), each O(V+E): vertical alignment to median
//! neighbors (skipping type-1 conflicts so long virtual chains stay
//! straight), then horizontal block compaction. Final coordinate = mean
//! of the middle two of the four candidates, after aligning the four
//! layouts to the narrowest one.
//!
//! Operates purely on the abstract (cross-axis) coordinate; the caller
//! maps it back into screen space.

pub(crate) struct BkInput<'a> {
    /// Layers in final (crossing-minimized, cluster-contiguous) order.
    pub layers: &'a [Vec<usize>],
    pub pred: &'a [Vec<usize>],
    pub succ: &'a [Vec<usize>],
    /// Cross-axis sizes (already direction-transposed by the caller).
    pub widths: &'a [f32],
    /// Indices >= real_count are virtual (edge bend dummies).
    pub real_count: usize,
    pub spacing: f32,
}

/// Compute the cross-axis center coordinate for every node.
pub(crate) fn assign_x(input: &BkInput) -> Vec<f32> {
    let n = input.pred.len();
    if n == 0 {
        return Vec::new();
    }

    let conflicts = mark_type1_conflicts(input);

    // Four passes: (downward?, leftward?) — downward = layers scanned
    // top→bottom aligning to upper neighbors; leftward = nodes scanned
    // left→right preferring leftmost medians.
    let mut candidates: Vec<Vec<f32>> = Vec::with_capacity(4);
    for &down in &[true, false] {
        for &left in &[true, false] {
            candidates.push(one_pass(input, &conflicts, down, left));
        }
    }

    // Align all four to the narrowest layout, per the paper: leftmost
    // layouts shift by (min_target - min_self), rightmost by
    // (max_target - max_self).
    let widths: Vec<(f32, f32)> = candidates
        .iter()
        .map(|xs| {
            let mut lo = f32::MAX;
            let mut hi = f32::MIN;
            for (i, &x) in xs.iter().enumerate() {
                lo = lo.min(x - input.widths[i] / 2.0);
                hi = hi.max(x + input.widths[i] / 2.0);
            }
            (lo, hi)
        })
        .collect();
    let narrowest = (0..4)
        .min_by(|&a, &b| {
            let wa = widths[a].1 - widths[a].0;
            let wb = widths[b].1 - widths[b].0;
            wa.partial_cmp(&wb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();
    for k in 0..4 {
        // passes 0,2 are "leftward", 1,3 "rightward" by construction order
        let leftward = k % 2 == 0;
        let shift = if leftward {
            widths[narrowest].0 - widths[k].0
        } else {
            widths[narrowest].1 - widths[k].1
        };
        for x in &mut candidates[k] {
            *x += shift;
        }
    }

    // Final: average of the middle two of the four sorted candidates.
    let mut out = vec![0.0f32; n];
    let mut four = [0.0f32; 4];
    for (i, slot) in out.iter_mut().enumerate() {
        for k in 0..4 {
            four[k] = candidates[k][i];
        }
        four.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        *slot = (four[1] + four[2]) / 2.0;
    }
    out
}

/// Type-1 conflicts: a non-inner segment crossing an inner segment
/// (virtual→virtual). The inner segment wins; the crossing edge must not
/// be used for alignment. Returned as a set of (upper, lower) pairs.
fn mark_type1_conflicts(input: &BkInput) -> std::collections::HashSet<(usize, usize)> {
    let mut conflicts = std::collections::HashSet::new();
    let pos = positions(input.layers, input.pred.len());
    let is_virtual = |v: usize| v >= input.real_count;

    for li in 0..input.layers.len().saturating_sub(1) {
        let lower = &input.layers[li + 1];
        let upper_len = input.layers[li].len();
        let mut k0 = 0usize;
        let mut l = 0usize;

        for (l1, &v) in lower.iter().enumerate() {
            // Inner segment: v virtual with a virtual upper neighbor.
            let inner_upper = if is_virtual(v) {
                input.pred[v].iter().copied().find(|&u| is_virtual(u))
            } else {
                None
            };
            if inner_upper.is_some() || l1 == lower.len() - 1 {
                let k1 = inner_upper.map_or(upper_len.saturating_sub(1), |u| pos[u]);
                while l <= l1 {
                    let w = lower[l];
                    for &u in &input.pred[w] {
                        if pos[u] < k0 || pos[u] > k1 {
                            conflicts.insert((u, w));
                        }
                    }
                    l += 1;
                }
                k0 = k1;
            }
        }
    }
    conflicts
}

fn positions(layers: &[Vec<usize>], n: usize) -> Vec<usize> {
    let mut pos = vec![0usize; n];
    for layer in layers {
        for (i, &v) in layer.iter().enumerate() {
            pos[v] = i;
        }
    }
    pos
}

fn one_pass(
    input: &BkInput,
    conflicts: &std::collections::HashSet<(usize, usize)>,
    down: bool,
    left: bool,
) -> Vec<f32> {
    let n = input.pred.len();
    let pos = positions(input.layers, n);

    // --- Vertical alignment ---
    let mut root: Vec<usize> = (0..n).collect();
    let mut align: Vec<usize> = (0..n).collect();

    let layer_range: Vec<usize> = if down {
        (0..input.layers.len()).collect()
    } else {
        (0..input.layers.len()).rev().collect()
    };

    for &li in &layer_range {
        // Skip the first scanned layer (it has no "previous" neighbors).
        if (down && li == 0) || (!down && li == input.layers.len() - 1) {
            continue;
        }
        let layer = &input.layers[li];
        let mut r: isize = if left { -1 } else { isize::MAX };

        let node_iter: Vec<usize> = if left {
            layer.clone()
        } else {
            layer.iter().rev().copied().collect()
        };

        for v in node_iter {
            if align[v] != v {
                continue;
            }
            let neigh = if down { &input.pred[v] } else { &input.succ[v] };
            if neigh.is_empty() {
                continue;
            }
            let mut sorted: Vec<usize> = neigh.clone();
            sorted.sort_by_key(|&u| pos[u]);
            let d = sorted.len();
            // Median pair (single median when odd).
            let medians: Vec<usize> = if d % 2 == 1 {
                vec![sorted[d / 2]]
            } else if left {
                vec![sorted[(d - 1) / 2], sorted[d / 2]]
            } else {
                vec![sorted[d / 2], sorted[(d - 1) / 2]]
            };

            for u in medians {
                if align[v] != v {
                    break;
                }
                let conflicted = if down {
                    conflicts.contains(&(u, v))
                } else {
                    conflicts.contains(&(v, u))
                };
                let ok_pos = if left {
                    (pos[u] as isize) > r
                } else {
                    (pos[u] as isize) < r
                };
                if !conflicted && ok_pos && align[u] == u {
                    align[u] = v;
                    root[v] = root[u];
                    align[v] = root[v];
                    r = pos[u] as isize;
                }
            }
        }
    }

    // --- Horizontal compaction ---
    // Iterative (explicit stack) to avoid recursion depth issues.
    let mut sink: Vec<usize> = (0..n).collect();
    let mut shift = vec![f32::INFINITY; n];
    let mut x = vec![f32::NAN; n];
    let pos_in_layer = &pos;

    // Neighbor toward the alignment side within the layer.
    let layer_of = {
        let mut lo = vec![0usize; n];
        for (li, layer) in input.layers.iter().enumerate() {
            for &v in layer {
                lo[v] = li;
            }
        }
        lo
    };
    let side_neighbor = |w: usize| -> Option<usize> {
        let layer = &input.layers[layer_of[w]];
        let p = pos_in_layer[w];
        if left {
            (p > 0).then(|| layer[p - 1])
        } else {
            (p + 1 < layer.len()).then(|| layer[p + 1])
        }
    };
    let delta =
        |a: usize, b: usize| -> f32 { (input.widths[a] + input.widths[b]) / 2.0 + input.spacing };

    #[allow(clippy::too_many_arguments)] // recursive helper mirrors the paper's signature
    fn place_block(
        v: usize,
        x: &mut [f32],
        sink: &mut [usize],
        shift: &mut [f32],
        root: &[usize],
        align: &[usize],
        side_neighbor: &dyn Fn(usize) -> Option<usize>,
        delta: &dyn Fn(usize, usize) -> f32,
        left: bool,
    ) {
        if !x[v].is_nan() {
            return;
        }
        x[v] = 0.0;
        let mut w = v;
        loop {
            if let Some(nb) = side_neighbor(w) {
                let u = root[nb];
                place_block(u, x, sink, shift, root, align, side_neighbor, delta, left);
                if sink[v] == v {
                    sink[v] = sink[u];
                }
                let gap = delta(nb, w);
                if sink[v] != sink[u] {
                    let s = if left {
                        x[v] - x[u] - gap
                    } else {
                        x[u] - x[v] - gap
                    };
                    if s < shift[sink[u]] {
                        shift[sink[u]] = s;
                    }
                } else if left {
                    x[v] = x[v].max(x[u] + gap);
                } else {
                    x[v] = x[v].min(x[u] - gap);
                }
            }
            w = align[w];
            if w == v {
                break;
            }
        }
    }

    for v in 0..n {
        if root[v] == v {
            place_block(
                v,
                &mut x,
                &mut sink,
                &mut shift,
                &root,
                &align,
                &side_neighbor,
                &delta,
                left,
            );
        }
    }
    for v in 0..n {
        x[v] = x[root[v]];
        let s = shift[sink[root[v]]];
        if s < f32::INFINITY {
            if left {
                x[v] += s;
            } else {
                x[v] -= s;
            }
        }
    }
    x
}
