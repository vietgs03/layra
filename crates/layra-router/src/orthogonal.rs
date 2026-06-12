//! Orthogonal edge routing: localized A* with bend penalties.
//!
//! When a straight edge would cut through a node, we re-route it as an
//! orthogonal (axis-aligned) polyline around the obstacles — the
//! libavoid/draw.io look. The search is deliberately **local**:
//!
//! 1. Region = bbox(source, target) inflated by a margin; only obstacles
//!    intersecting the region participate.
//! 2. Candidate coordinates = obstacle borders (± clearance) + endpoints.
//!    Because every obstacle boundary is in the coordinate set, two
//!    adjacent grid lines can never strictly straddle an obstacle, so a
//!    midpoint test per step is exact.
//! 3. A* over grid intersections; state = (point, incoming direction) so
//!    each 90° turn costs `bend_penalty`. Heuristic = Manhattan distance
//!    (admissible: every move is axis-aligned).
//!
//! Cost stays tiny on real diagrams: the grid is typically < 40×40 and
//! only edges that actually collide pay for the search.

use layra_core::{Point, Rect};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const CLEARANCE: f32 = 10.0;
const REGION_MARGIN: f32 = 80.0;
const BEND_PENALTY: f32 = 40.0;

/// Route from `start` to `goal` around `obstacles` (rects the path must not
/// enter). Returns an orthogonal polyline including both endpoints, or
/// `None` when no route exists within the local region.
pub(crate) fn route_around(start: Point, goal: Point, obstacles: &[Rect]) -> Option<Vec<Point>> {
    // Local region: everything relevant to this edge.
    let region = Rect::new(
        start.x.min(goal.x),
        start.y.min(goal.y),
        (start.x - goal.x).abs().max(1.0),
        (start.y - goal.y).abs().max(1.0),
    )
    .inflate(REGION_MARGIN);

    let blocked: Vec<Rect> = obstacles
        .iter()
        .filter(|r| r.intersects(&region))
        .map(|r| r.inflate(CLEARANCE * 0.5))
        .collect();

    // Candidate grid lines: obstacle borders ± clearance, plus endpoints.
    let mut xs = vec![start.x, goal.x];
    let mut ys = vec![start.y, goal.y];
    for r in &blocked {
        xs.push(r.x - CLEARANCE);
        xs.push(r.right() + CLEARANCE);
        ys.push(r.y - CLEARANCE);
        ys.push(r.bottom() + CLEARANCE);
    }
    xs.retain(|v| v.is_finite());
    ys.retain(|v| v.is_finite());
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.5);

    let nx = xs.len();
    let ny = ys.len();
    if nx * ny > 10_000 {
        return None; // degenerate; caller keeps the straight route
    }

    let idx_of = |v: f32, axis: &[f32]| -> usize {
        axis.iter().position(|&a| (a - v).abs() < 0.5).unwrap_or(0)
    };
    let si = (idx_of(start.x, &xs), idx_of(start.y, &ys));
    let gi = (idx_of(goal.x, &xs), idx_of(goal.y, &ys));

    let inside_any = |p: Point| -> bool {
        blocked
            .iter()
            .any(|r| p.x > r.x && p.x < r.right() && p.y > r.y && p.y < r.bottom())
    };

    // A* over (grid point, incoming direction). Directions: 0..4, 4 = none.
    #[derive(PartialEq)]
    struct Open {
        f: f32,
        g: f32,
        node: (usize, usize, usize),
    }
    impl Eq for Open {}
    impl Ord for Open {
        fn cmp(&self, other: &Self) -> Ordering {
            other.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
        }
    }
    impl PartialOrd for Open {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    let h = |i: usize, j: usize| -> f32 { (xs[i] - xs[gi.0]).abs() + (ys[j] - ys[gi.1]).abs() };

    const DIRS: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let state_id = |i: usize, j: usize, d: usize| (j * nx + i) * 5 + d;

    let mut best = vec![f32::INFINITY; nx * ny * 5];
    let mut parent: Vec<u32> = vec![u32::MAX; nx * ny * 5];
    let mut heap = BinaryHeap::new();

    let start_state = (si.0, si.1, 4);
    best[state_id(si.0, si.1, 4)] = 0.0;
    heap.push(Open {
        f: h(si.0, si.1),
        g: 0.0,
        node: start_state,
    });

    let mut goal_state: Option<(usize, usize, usize)> = None;
    while let Some(Open {
        g, node: (i, j, d), ..
    }) = heap.pop()
    {
        if g > best[state_id(i, j, d)] {
            continue;
        }
        if (i, j) == gi {
            goal_state = Some((i, j, d));
            break;
        }
        for (nd, &(dx, dy)) in DIRS.iter().enumerate() {
            let ni = i as isize + dx;
            let nj = j as isize + dy;
            if ni < 0 || nj < 0 || ni as usize >= nx || nj as usize >= ny {
                continue;
            }
            let (ni, nj) = (ni as usize, nj as usize);
            // Midpoint test is exact (obstacle borders are grid lines).
            let mid = Point::new((xs[i] + xs[ni]) / 2.0, (ys[j] + ys[nj]) / 2.0);
            if inside_any(mid) {
                continue;
            }
            let step = (xs[ni] - xs[i]).abs() + (ys[nj] - ys[j]).abs();
            let bend = if d != 4 && d != nd { BEND_PENALTY } else { 0.0 };
            let ng = g + step + bend;
            let sid = state_id(ni, nj, nd);
            if ng < best[sid] {
                best[sid] = ng;
                parent[sid] = state_id(i, j, d) as u32;
                heap.push(Open {
                    f: ng + h(ni, nj),
                    g: ng,
                    node: (ni, nj, nd),
                });
            }
        }
    }

    let (mut ci, mut cj, mut cd) = goal_state?;
    let mut path = vec![Point::new(xs[ci], ys[cj])];
    while (ci, cj, cd) != start_state {
        let pid = parent[state_id(ci, cj, cd)];
        if pid == u32::MAX {
            return None;
        }
        let pid = pid as usize;
        cd = pid % 5;
        let cell = pid / 5;
        ci = cell % nx;
        cj = cell / nx;
        path.push(Point::new(xs[ci], ys[cj]));
    }
    path.reverse();

    // Merge collinear runs so the polyline only keeps real corners.
    let mut simplified: Vec<Point> = Vec::with_capacity(path.len());
    for p in path {
        if simplified.len() >= 2 {
            let a = simplified[simplified.len() - 2];
            let b = simplified[simplified.len() - 1];
            let collinear = ((a.x - b.x).abs() < 0.01 && (b.x - p.x).abs() < 0.01)
                || ((a.y - b.y).abs() < 0.01 && (b.y - p.y).abs() < 0.01);
            if collinear {
                *simplified.last_mut().unwrap() = p;
                continue;
            }
        }
        simplified.push(p);
    }
    Some(simplified)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg_hits_rect(a: Point, b: Point, r: &Rect) -> bool {
        // Sample-based check is fine for tests (axis-aligned segments).
        for t in 0..=20 {
            let f = t as f32 / 20.0;
            let p = Point::new(a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f);
            if p.x > r.x && p.x < r.right() && p.y > r.y && p.y < r.bottom() {
                return true;
            }
        }
        false
    }

    #[test]
    fn routes_around_blocking_node() {
        let blocker = Rect::new(80.0, -20.0, 60.0, 40.0);
        let path = route_around(Point::new(0.0, 0.0), Point::new(220.0, 0.0), &[blocker]).unwrap();

        assert!(path.len() >= 3, "must bend around the blocker: {path:?}");
        for w in path.windows(2) {
            assert!(
                !seg_hits_rect(w[0], w[1], &blocker),
                "segment {:?} -> {:?} passes through blocker",
                w[0],
                w[1]
            );
        }
        assert_eq!(path[0], Point::new(0.0, 0.0));
        assert_eq!(*path.last().unwrap(), Point::new(220.0, 0.0));
    }

    #[test]
    fn straight_when_clear() {
        let far = Rect::new(500.0, 500.0, 40.0, 40.0);
        let path = route_around(Point::new(0.0, 0.0), Point::new(100.0, 0.0), &[far]).unwrap();
        assert_eq!(path.len(), 2, "no obstacles in the way: {path:?}");
    }

    #[test]
    fn bend_penalty_prefers_fewer_corners() {
        // Two stacked blockers with a slot between them: the route should
        // still have a small number of corners, not a staircase.
        let a = Rect::new(60.0, -100.0, 40.0, 80.0);
        let b = Rect::new(60.0, 20.0, 40.0, 80.0);
        let path = route_around(Point::new(0.0, 0.0), Point::new(160.0, 0.0), &[a, b]).unwrap();
        assert!(path.len() <= 4, "expected few corners, got {path:?}");
    }
}
