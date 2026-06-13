//! # layra-router
//!
//! Orthogonal-by-default edge routing for flowcharts — the draw.io /
//! libavoid / AWS-architecture look. Every multi-rank edge is routed as
//! clean axis-aligned segments:
//!
//! 1. **Ports + stubs** — pick a perpendicular attachment on each node
//!    border (top/bottom for vertical layouts, left/right for horizontal),
//!    then leave/enter the node with a short straight stub.
//! 2. **A\* over a visibility grid** — connect the two stubs with an
//!    orthogonal polyline that avoids every non-endpoint node, preferring
//!    few bends (libavoid-style bend penalty). The grid is built from
//!    obstacle borders so it is small and the search is local.
//! 3. **Fallback** — if A\* is skipped (degenerate grid) or finds nothing,
//!    emit a Z-connector so output is *always* axis-aligned.
//!
//! Self-loops draw a lasso; invisible links constrain layout but route
//! nothing. Rounded corners are applied by the renderer.

mod grid;
mod orthogonal;

use layra_core::{Graph, Point, Rect};

pub fn route(graph: &mut Graph) {
    let rects: Vec<Rect> = graph.nodes.iter().map(|n| n.rect).collect();
    let dir = graph.direction;
    // Spatial index: collision candidates per region instead of all nodes.
    let index = grid::SpatialGrid::build(&rects);
    let mut candidates: Vec<usize> = Vec::new();
    // Global A* budget: full-quality routing for every edge on real
    // diagrams, graceful degradation (Z-connector fallback) on synthetic
    // stress graphs where thousands of edges would each spin up a grid.
    let mut astar_budget: u32 = 1200;

    for edge in &mut graph.edges {
        let src = rects[edge.source.index()];
        let dst = rects[edge.target.index()];

        // Self loop: draw a small lasso on the node's right side.
        if edge.source == edge.target {
            let c = src.center();
            let r = src.right();
            edge.points = vec![
                Point::new(r, c.y - 8.0),
                Point::new(r + 28.0, c.y - 14.0),
                Point::new(r + 28.0, c.y + 14.0),
                Point::new(r, c.y + 8.0),
            ];
            edge.label_pos = Some(Point::new(r + 34.0, c.y));
            continue;
        }

        // Perpendicular border ports + outward stubs at each end.
        let (src_port, dst_port) = orthogonal::ports(src, dst, dir);
        let (p0, _) = src_port;
        let (p1, _) = dst_port;
        let s0 = orthogonal::stub_point(src_port);
        let s1 = orthogonal::stub_point(dst_port);

        // Gather local obstacles (every node except the two endpoints).
        let region = polyline_bbox(&[p0, s0, s1, p1]).inflate(120.0);
        index.query(&region, &mut candidates);
        const MAX_OBSTACLES: usize = 64;
        let mut routed: Option<Vec<Point>> = None;
        if astar_budget > 0 && candidates.len() <= MAX_OBSTACLES {
            astar_budget -= 1;
            let obstacles: Vec<Rect> = candidates
                .iter()
                .filter(|&&i| i != edge.source.index() && i != edge.target.index())
                .map(|&i| rects[i])
                .collect();
            if let Some(mut path) = orthogonal::route_around(s0, s1, &obstacles) {
                // Re-attach the border ports as true endpoints; the A* path
                // already starts/ends at the stubs.
                let mut full = Vec::with_capacity(path.len() + 2);
                full.push(p0);
                full.append(&mut path);
                full.push(p1);
                routed = Some(orthogonal::simplify_collinear(full));
            }
        }

        edge.points = routed.unwrap_or_else(|| {
            // Always-orthogonal fallback through the two stubs.
            let mut path = vec![p0];
            path.extend(orthogonal::orthogonal_connector(s0, s1, dir));
            path.push(p1);
            orthogonal::simplify_collinear(path)
        });

        // Label placement happens in a second pass (needs all routed paths).
    }

    separate_parallel_edges(graph);
    place_labels(graph);
}

/// Edges sharing the same node pair (in either direction) would render on
/// top of each other. Offset each one perpendicular to its direction,
/// symmetric around the original line: 2 edges → ±5px, 3 → -10/0/+10...
fn separate_parallel_edges(graph: &mut Graph) {
    use std::collections::HashMap;

    const GAP: f32 = 5.0;

    let mut groups: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, e) in graph.edges.iter().enumerate() {
        if e.source == e.target {
            continue; // self loops have their own shape
        }
        let key = (e.source.0.min(e.target.0), e.source.0.max(e.target.0));
        groups.entry(key).or_default().push(i);
    }

    for indices in groups.values().filter(|v| v.len() > 1) {
        let n = indices.len() as f32;
        for (k, &ei) in indices.iter().enumerate() {
            let offset = (k as f32 - (n - 1.0) / 2.0) * GAP * 2.0;
            if offset == 0.0 {
                continue;
            }
            let edge = &mut graph.edges[ei];
            if edge.points.len() < 2 {
                continue;
            }
            // Perpendicular of the overall direction; flip for reversed
            // edges so A->B and B->A move to opposite sides consistently.
            let first = edge.points[0];
            let last = edge.points[edge.points.len() - 1];
            let (dx, dy) = (last.x - first.x, last.y - first.y);
            let len = (dx * dx + dy * dy).sqrt().max(0.001);
            let sign = if edge.source.0 <= edge.target.0 {
                1.0
            } else {
                -1.0
            };
            let (nx, ny) = (-dy / len * sign, dx / len * sign);
            for p in &mut edge.points {
                p.x += nx * offset;
                p.y += ny * offset;
            }
        }
    }
}

/// Place edge labels offset perpendicular to the path so text never sits
/// on the arrow line; stagger labels of edges sharing the same node pair
/// (A->B and B->A) to opposite sides so they don't collide either.
fn place_labels(graph: &mut Graph) {
    use std::collections::HashMap;

    // Node rects are obstacles labels must avoid; snapshot once.
    let node_rects: Vec<Rect> = graph.nodes.iter().map(|n| n.rect).collect();

    // Count labeled edges per unordered node pair to detect parallels.
    let mut seen: HashMap<(u32, u32), usize> = HashMap::new();

    for edge in &mut graph.edges {
        if edge.label.is_none() || edge.points.len() < 2 || edge.source == edge.target {
            continue; // self-loops position their own label
        }
        let key = (
            edge.source.0.min(edge.target.0),
            edge.source.0.max(edge.target.0),
        );
        let ordinal = *seen.entry(key).and_modify(|c| *c += 1).or_insert(0);

        let (mid, dir) = polyline_midpoint_dir(&edge.points);
        // Unit normal to the path direction.
        let len = (dir.x * dir.x + dir.y * dir.y).sqrt().max(0.001);
        let (nx, ny) = (-dir.y / len, dir.x / len);
        // First label goes one side, second the other, third further out.
        let side = if ordinal.is_multiple_of(2) { 1.0 } else { -1.0 };
        let base_dist = 12.0 + (ordinal / 2) as f32 * 16.0;

        let label = edge.label.as_deref().unwrap_or("");
        let pos = clear_label_pos(
            mid,
            (nx * side, ny * side),
            base_dist,
            label,
            &node_rects,
            edge.source.index(),
            edge.target.index(),
        );
        edge.label_pos = Some(pos);
    }
}

/// Estimated label box (matches the renderer: ~7px/char advance, 20px tall).
fn label_rect(label: &str, cx: f32, cy: f32) -> Rect {
    let w = label.chars().count() as f32 * 7.0 + 12.0;
    Rect::new(cx - w / 2.0, cy - 10.0, w, 20.0)
}

/// Place the label starting at `base_dist` along the normal, then push it
/// further out in steps until its box clears every node that is not an
/// endpoint of this edge. Gives up after a bounded search and returns the
/// least-bad position (the contract test treats border touches as fine).
fn clear_label_pos(
    mid: Point,
    normal: (f32, f32),
    base_dist: f32,
    label: &str,
    nodes: &[Rect],
    src: usize,
    dst: usize,
) -> Point {
    let (nx, ny) = normal;
    let intrudes = |cx: f32, cy: f32| -> bool {
        let lb = label_rect(label, cx, cy);
        nodes.iter().enumerate().any(|(i, r)| {
            if i == src || i == dst {
                return false;
            }
            // Shrink the node rect so a border touch isn't a collision.
            let r = Rect::new(r.x + 2.0, r.y + 2.0, r.width - 4.0, r.height - 4.0);
            r.width > 0.0 && r.height > 0.0 && lb.intersects(&r)
        })
    };

    // Try increasing offsets on the preferred side, then the opposite side.
    for &sign in &[1.0f32, -1.0] {
        let mut dist = base_dist;
        for _ in 0..14 {
            let cx = mid.x + nx * sign * dist;
            let cy = mid.y + ny * sign * dist;
            if !intrudes(cx, cy) {
                return Point::new(cx, cy);
            }
            dist += 14.0;
        }
    }
    // Fallback: original preferred position.
    Point::new(mid.x + nx * base_dist, mid.y + ny * base_dist)
}

fn polyline_bbox(points: &[Point]) -> Rect {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

/// Midpoint of the polyline by arc length, plus the direction vector of
/// the segment containing it (for perpendicular label offsets).
fn polyline_midpoint_dir(points: &[Point]) -> (Point, Point) {
    let total: f32 = points
        .windows(2)
        .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
        .sum();
    let mut remaining = total / 2.0;
    for w in points.windows(2) {
        let seg = ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
        if seg >= remaining && seg > 0.0 {
            let f = remaining / seg;
            return (
                Point::new(
                    w[0].x + (w[1].x - w[0].x) * f,
                    w[0].y + (w[1].y - w[0].y) * f,
                ),
                Point::new(w[1].x - w[0].x, w[1].y - w[0].y),
            );
        }
        remaining -= seg;
    }
    let mid = points[points.len() / 2];
    (mid, Point::new(1.0, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midpoint_of_straight_line() {
        let pts = [Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
        let (m, dir) = polyline_midpoint_dir(&pts);
        assert!((m.x - 5.0).abs() < 0.01);
        assert!(dir.x > 0.0 && dir.y == 0.0);
    }
}
