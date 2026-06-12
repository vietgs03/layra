//! Phase 1–3: cycle breaking, layer assignment, virtual-node insertion.

use crate::LayoutGraph;
use layra_core::{Direction, Graph};

/// Build the working graph and break cycles with a DFS: any edge that closes
/// a cycle (back edge) is reversed for layout purposes only.
pub(crate) fn build(graph: &Graph, options: &crate::LayoutOptions) -> LayoutGraph {
    let n = graph.nodes.len();
    let mut succ = vec![Vec::new(); n];
    let mut pred = vec![Vec::new(); n];

    // DFS cycle detection: 0 = white, 1 = gray (on stack), 2 = black.
    let mut color = vec![0u8; n];
    // We process edges in DFS discovery order so reversals are deterministic.
    let mut adj = vec![Vec::new(); n];
    for e in &graph.edges {
        adj[e.source.index()].push(e.target.index());
    }

    fn dfs(
        u: usize,
        adj: &[Vec<usize>],
        color: &mut [u8],
        succ: &mut [Vec<usize>],
        pred: &mut [Vec<usize>],
    ) {
        color[u] = 1;
        for &v in &adj[u] {
            if u == v {
                continue; // self loop: drop from layout, renderer draws it locally
            }
            if color[v] == 1 {
                // Back edge: reverse to break the cycle.
                succ[v].push(u);
                pred[u].push(v);
            } else {
                succ[u].push(v);
                pred[v].push(u);
                if color[v] == 0 {
                    dfs(v, adj, color, succ, pred);
                }
            }
        }
        color[u] = 2;
    }

    for u in 0..n {
        if color[u] == 0 {
            dfs(u, &adj, &mut color, &mut succ, &mut pred);
        }
    }

    // Sizes live in abstract (cross, main) space: for horizontal layouts the
    // main axis is x, so width/height swap. `position::apply` maps back.
    //
    // Cluster members get their size inflated by the cluster padding so the
    // post-hoc subgraph rect (drawn around true node sizes) never overlaps
    // neighboring non-member nodes — the cheap version of dagre's border
    // nodes.
    let horizontal = matches!(graph.direction, Direction::LeftRight | Direction::RightLeft);
    let pad2 = options.cluster_padding * 2.0;
    let sizes = graph
        .nodes
        .iter()
        .map(|nd| {
            let pad = if nd.parent.is_some() { pad2 } else { 0.0 };
            let (w, h) = (nd.size.width + pad, nd.size.height + pad);
            if horizontal {
                (h, w)
            } else {
                (w, h)
            }
        })
        .collect();

    LayoutGraph {
        succ,
        pred,
        layer: vec![0; n],
        sizes,
        real_count: n,
        edge_chains: Vec::new(),
        layers: Vec::new(),
        pos: Vec::new(),
    }
}

/// Longest-path layering: each node sits one layer below its deepest
/// predecessor. O(V+E) via memoized DFS.
pub(crate) fn assign_layers(lg: &mut LayoutGraph) {
    let n = lg.real_count;
    let mut memo: Vec<Option<usize>> = vec![None; n];

    fn depth(u: usize, pred: &[Vec<usize>], memo: &mut [Option<usize>]) -> usize {
        if let Some(d) = memo[u] {
            return d;
        }
        // Mark to guard against residual cycles (shouldn't happen post-break).
        memo[u] = Some(0);
        let d = pred[u]
            .iter()
            .map(|&p| depth(p, pred, memo) + 1)
            .max()
            .unwrap_or(0);
        memo[u] = Some(d);
        d
    }

    for u in 0..n {
        lg.layer[u] = depth(u, &lg.pred, &mut memo);
    }
}

/// Split edges spanning multiple layers by inserting zero-size virtual nodes,
/// and record the chain for each original edge so the router can recover the
/// full polyline.
pub(crate) fn insert_virtual_nodes(lg: &mut LayoutGraph, graph: &Graph) {
    lg.edge_chains = Vec::with_capacity(graph.edges.len());

    // Rebuild succ/pred from scratch including virtuals: simpler and cheap.
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); lg.real_count];
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); lg.real_count];

    for e in &graph.edges {
        let (u, v) = (e.source.index(), e.target.index());
        if u == v {
            lg.edge_chains.push(vec![u, v]);
            continue;
        }
        // Use layout-direction order (cycle breaking may have flipped it).
        let (from, to, flipped) = if lg.layer[u] <= lg.layer[v] {
            (u, v, false)
        } else {
            (v, u, true)
        };

        let mut chain = vec![from];
        let mut prev = from;
        let span = lg.layer[to].saturating_sub(lg.layer[from]);
        if span > 1 {
            for step in 1..span {
                let vid = lg.layer.len();
                lg.layer.push(lg.layer[from] + step);
                lg.sizes.push((0.0, 0.0));
                succ.push(Vec::new());
                pred.push(Vec::new());
                succ[prev].push(vid);
                pred[vid].push(prev);
                chain.push(vid);
                prev = vid;
            }
        }
        succ[prev].push(to);
        pred[to].push(prev);
        chain.push(to);

        if flipped {
            chain.reverse();
        }
        lg.edge_chains.push(chain);
    }

    lg.succ = succ;
    lg.pred = pred;

    // Bucket nodes into layers.
    let max_layer = lg.layer.iter().copied().max().unwrap_or(0);
    lg.layers = vec![Vec::new(); max_layer + 1];
    for (i, &l) in lg.layer.iter().enumerate() {
        lg.layers[l].push(i);
    }
}
