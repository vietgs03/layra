//! # layra-router
//!
//! Edge routing in two tiers:
//! 1. **Fast path** — clip endpoints to node borders, pass layout
//!    waypoints through. Most edges in a layered layout are already clear.
//! 2. **Collision repair** — edges whose polyline cuts through a node are
//!    re-routed orthogonally around the obstacles with a localized A*
//!    (bend-penalized, libavoid-style). Only colliding edges pay.

mod grid;
mod orthogonal;

use layra_core::{Graph, Point, Rect};

pub fn route(graph: &mut Graph) {
    let rects: Vec<Rect> = graph.nodes.iter().map(|n| n.rect).collect();
    // Spatial index: collision candidates per region instead of all nodes.
    let index = grid::SpatialGrid::build(&rects);
    let mut candidates: Vec<usize> = Vec::new();
    // Global repair budget: quality routing for every edge on real
    // diagrams (collisions are rare), graceful degradation on synthetic
    // stress graphs where thousands of edges collide at once.
    let mut repair_budget: u32 = 600;

    for edge in &mut graph.edges {
        let src = rects[edge.source.index()];
        let dst = rects[edge.target.index()];

        if edge.points.len() < 2 {
            edge.points = vec![src.center(), dst.center()];
        }

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

        // Clip first segment at the source border, last at the target border.
        let n = edge.points.len();
        let first_inner = edge.points[1];
        let last_inner = edge.points[n - 2];
        edge.points[0] = clip_to_rect(src, src.center(), first_inner);
        edge.points[n - 1] = clip_to_rect(dst, dst.center(), last_inner);

        // Collision repair: if any segment passes through a node that is
        // neither endpoint, re-route orthogonally around the obstacles.
        // The grid keeps this O(local) instead of O(all nodes) per edge.
        let bbox = polyline_bbox(&edge.points).inflate(4.0);
        index.query(&bbox, &mut candidates);
        let collides = edge.points.windows(2).any(|w| {
            candidates.iter().any(|&i| {
                i != edge.source.index()
                    && i != edge.target.index()
                    && segment_intersects_rect(w[0], w[1], &rects[i].inflate(2.0))
            })
        });

        if collides && repair_budget > 0 {
            repair_budget -= 1;
            let region = bbox.inflate(120.0);
            index.query(&region, &mut candidates);
            // Budget guard: in very dense neighborhoods the A* grid grows
            // quadratically with obstacle count and the visual win shrinks
            // (everything is packed anyway). Repair only sane regions.
            const MAX_OBSTACLES: usize = 48;
            if candidates.len() <= MAX_OBSTACLES {
                let obstacles: Vec<Rect> = candidates
                    .iter()
                    .filter(|&&i| i != edge.source.index() && i != edge.target.index())
                    .map(|&i| rects[i])
                    .collect();
                let start = edge.points[0];
                let goal = edge.points[edge.points.len() - 1];
                if let Some(path) = orthogonal::route_around(start, goal, &obstacles) {
                    edge.points = path;
                }
            }
        }

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
        let dist = 12.0 + (ordinal / 2) as f32 * 16.0;
        edge.label_pos = Some(Point::new(
            mid.x + nx * side * dist,
            mid.y + ny * side * dist,
        ));
    }
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

/// Conservative segment-vs-rect test via Liang-Barsky clipping.
fn segment_intersects_rect(a: Point, b: Point, r: &Rect) -> bool {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let checks = [
        (-dx, a.x - r.x),
        (dx, r.right() - a.x),
        (-dy, a.y - r.y),
        (dy, r.bottom() - a.y),
    ];
    for (p, q) in checks {
        if p.abs() < f32::EPSILON {
            if q < 0.0 {
                return false;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            if t > t1 {
                return false;
            }
            if t > t0 {
                t0 = t;
            }
        } else {
            if t < t0 {
                return false;
            }
            if t < t1 {
                t1 = t;
            }
        }
    }
    t0 < t1
}

/// Intersect the ray `from -> to` (with `from` inside `rect`) against the
/// rect border. Falls back to `from` for degenerate rays.
fn clip_to_rect(rect: Rect, from: Point, to: Point) -> Point {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
        return from;
    }

    let mut t = f32::MAX;
    if dx > 0.0 {
        t = t.min((rect.right() - from.x) / dx);
    } else if dx < 0.0 {
        t = t.min((rect.x - from.x) / dx);
    }
    if dy > 0.0 {
        t = t.min((rect.bottom() - from.y) / dy);
    } else if dy < 0.0 {
        t = t.min((rect.y - from.y) / dy);
    }
    if !t.is_finite() || t == f32::MAX {
        return from;
    }
    Point::new(from.x + dx * t, from.y + dy * t)
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
    fn clips_to_border() {
        let rect = Rect::new(0.0, 0.0, 100.0, 50.0);
        let p = clip_to_rect(rect, rect.center(), Point::new(200.0, 25.0));
        assert!((p.x - 100.0).abs() < 0.01);
        assert!((p.y - 25.0).abs() < 0.01);
    }

    #[test]
    fn midpoint_of_straight_line() {
        let pts = [Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
        let (m, dir) = polyline_midpoint_dir(&pts);
        assert!((m.x - 5.0).abs() < 0.01);
        assert!(dir.x > 0.0 && dir.y == 0.0);
    }
}
