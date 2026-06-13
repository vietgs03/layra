//! Orthogonal edge routing: A* over a visibility grid with bend penalties.
//!
//! Orthogonal (axis-aligned) routing is the **default** for flowchart
//! edges — the libavoid / draw.io / AWS-architecture look. Every edge is
//! routed as clean horizontal/vertical segments that avoid node rects,
//! with rounded corners applied by the renderer.
//!
//! The search is deliberately **local**:
//!
//! 1. Region = bbox(start, goal) inflated by a margin; only obstacles
//!    intersecting the region participate.
//! 2. Candidate coordinates = obstacle borders (± clearance) + endpoints.
//!    Because every obstacle boundary is in the coordinate set, two
//!    adjacent grid lines can never strictly straddle an obstacle, so a
//!    midpoint test per step is exact.
//! 3. A* over grid intersections; state = (point, incoming direction) so
//!    each 90° turn costs `bend_penalty`. Heuristic = Manhattan distance
//!    (admissible: every move is axis-aligned).
//!
//! Cost stays tiny on real diagrams: the grid is typically < 40×40 and the
//! corridor between two ranks contains only a handful of obstacles.

use layra_core::Direction;
use layra_core::{Point, Rect};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const CLEARANCE: f32 = 10.0;
const REGION_MARGIN: f32 = 80.0;
const BEND_PENALTY: f32 = 40.0;
/// How far the edge travels straight out of a node border before it is free
/// to turn. Gives every connector a clean perpendicular stub at both ends.
pub(crate) const STUB: f32 = 14.0;

/// A border attachment point plus the outward unit normal at that point.
pub(crate) type Port = (Point, (f32, f32));

/// Choose source/target attachment ports based on the layout direction and
/// the relative position of the two node rects. Vertical layouts attach to
/// top/bottom borders; horizontal layouts to left/right borders.
///
/// **Port-aware**: along the attachment side, the connection point slides
/// toward the *other* endpoint (clamped inside the node, with a small inset
/// so it never lands on a corner) instead of always sitting at the border
/// centre. Edges between offset nodes then leave/enter facing each other,
/// which reads cleaner and shortens the routed path. When the two nodes are
/// aligned on the cross axis the offset is zero, so it degenerates to the
/// border centre.
pub(crate) fn ports(src: Rect, dst: Rect, dir: Direction) -> (Port, Port) {
    let sc = src.center();
    let dc = dst.center();
    let dx = dc.x - sc.x;
    let dy = dc.y - sc.y;
    let vertical_main = matches!(dir, Direction::TopBottom | Direction::BottomTop);

    // Prefer the layout's main axis; switch to the cross axis only when the
    // two nodes share a rank (negligible main-axis separation).
    let use_vertical = if vertical_main {
        dy.abs() >= 8.0 || dx.abs() < 8.0
    } else {
        dy.abs() >= 8.0 && dx.abs() < 8.0
    };

    if use_vertical {
        // Attach on top/bottom borders; slide x toward the other node.
        let src_x = facing_coord(dc.x, src.x, src.right());
        let dst_x = facing_coord(sc.x, dst.x, dst.right());
        if dy >= 0.0 {
            (
                (Point::new(src_x, src.bottom()), (0.0, 1.0)),
                (Point::new(dst_x, dst.y), (0.0, -1.0)),
            )
        } else {
            (
                (Point::new(src_x, src.y), (0.0, -1.0)),
                (Point::new(dst_x, dst.bottom()), (0.0, 1.0)),
            )
        }
    } else {
        // Attach on left/right borders; slide y toward the other node.
        let src_y = facing_coord(dc.y, src.y, src.bottom());
        let dst_y = facing_coord(sc.y, dst.y, dst.bottom());
        if dx >= 0.0 {
            (
                (Point::new(src.right(), src_y), (1.0, 0.0)),
                (Point::new(dst.x, dst_y), (-1.0, 0.0)),
            )
        } else {
            (
                (Point::new(src.x, src_y), (-1.0, 0.0)),
                (Point::new(dst.right(), dst_y), (1.0, 0.0)),
            )
        }
    }
}

/// Pick the along-border coordinate for a port: slide toward the other
/// endpoint (`other_center`), clamped to the node's border span `[lo, hi]`
/// with a small inset so the port never sits on a corner. When the endpoints
/// are aligned on this axis the result is the border centre.
fn facing_coord(other_center: f32, lo: f32, hi: f32) -> f32 {
    // Inset keeps the stub clear of the corner radius / adjacent border.
    const INSET: f32 = 6.0;
    let (lo, hi) = (lo + INSET, hi - INSET);
    if hi <= lo {
        // Node too small to offset meaningfully; use the centre.
        return (lo + hi) / 2.0;
    }
    other_center.clamp(lo, hi)
}

/// Move a port point outward along its normal by `STUB` so the connector
/// always leaves/enters a node perpendicular to its border.
pub(crate) fn stub_point(port: Port) -> Point {
    let ((p, (nx, ny)), s) = (port, STUB);
    Point::new(p.x + nx * s, p.y + ny * s)
}

/// Route from `start` to `goal` around `obstacles` (rects the path must not
/// enter). Returns an orthogonal polyline including both endpoints, or
/// `None` when no route exists within the local region.
pub(crate) fn route_around(start: Point, goal: Point, obstacles: &[Rect]) -> Option<Vec<Point>> {
    let region = bbox(start, goal).inflate(REGION_MARGIN);
    let blocked: Vec<Rect> = obstacles
        .iter()
        .filter(|r| r.intersects(&region))
        .map(|r| r.inflate(CLEARANCE * 0.5))
        .collect();
    astar_grid(start, goal, &blocked)
}

fn bbox(a: Point, b: Point) -> Rect {
    Rect::new(
        a.x.min(b.x),
        a.y.min(b.y),
        (a.x - b.x).abs().max(1.0),
        (a.y - b.y).abs().max(1.0),
    )
}

/// A* over an orthogonal visibility grid. `blocked` are the inflated
/// obstacle rects the path may not enter; `start`/`goal` must not be
/// strictly inside any of them.
fn astar_grid(start: Point, goal: Point, blocked: &[Rect]) -> Option<Vec<Point>> {
    // Candidate grid lines: obstacle borders ± clearance, plus endpoints.
    let mut xs = vec![start.x, goal.x];
    let mut ys = vec![start.y, goal.y];
    for r in blocked {
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
        return None; // degenerate; caller keeps a simpler route
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

    Some(simplify_collinear(path))
}

/// Merge collinear runs so the polyline only keeps real corners.
pub(crate) fn simplify_collinear(path: Vec<Point>) -> Vec<Point> {
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
        // Drop zero-length steps that survive port/stub assembly.
        if let Some(&last) = simplified.last() {
            if (last.x - p.x).abs() < 0.01 && (last.y - p.y).abs() < 0.01 {
                continue;
            }
        }
        simplified.push(p);
    }
    simplified
}

/// Axis-aligned fallback connector between two stub points: a single
/// "Z" (vertical-horizontal-vertical or horizontal-vertical-horizontal)
/// honoring the layout's main axis. Always orthogonal; used only when the
/// A* search is skipped or fails so output stays axis-aligned regardless.
pub(crate) fn orthogonal_connector(a: Point, b: Point, dir: Direction) -> Vec<Point> {
    let vertical_main = matches!(dir, Direction::TopBottom | Direction::BottomTop);
    if (a.x - b.x).abs() < 0.5 || (a.y - b.y).abs() < 0.5 {
        return vec![a, b]; // already a straight axis-aligned segment
    }
    if vertical_main {
        let mid = (a.y + b.y) / 2.0;
        vec![a, Point::new(a.x, mid), Point::new(b.x, mid), b]
    } else {
        let mid = (a.x + b.x) / 2.0;
        vec![a, Point::new(mid, a.y), Point::new(mid, b.y), b]
    }
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

    fn axis_aligned(path: &[Point]) -> bool {
        path.windows(2)
            .all(|w| (w[0].x - w[1].x).abs() < 0.5 || (w[0].y - w[1].y).abs() < 0.5)
    }

    #[test]
    fn routes_around_blocking_node() {
        let blocker = Rect::new(80.0, -20.0, 60.0, 40.0);
        let path = route_around(Point::new(0.0, 0.0), Point::new(220.0, 0.0), &[blocker]).unwrap();

        assert!(path.len() >= 3, "must bend around the blocker: {path:?}");
        assert!(axis_aligned(&path), "must be axis-aligned: {path:?}");
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

    #[test]
    fn connector_is_axis_aligned() {
        let p = orthogonal_connector(
            Point::new(0.0, 0.0),
            Point::new(50.0, 100.0),
            Direction::TopBottom,
        );
        assert!(axis_aligned(&p), "fallback must be orthogonal: {p:?}");
    }

    #[test]
    fn ports_pick_perpendicular_attachment() {
        let src = Rect::new(0.0, 0.0, 40.0, 20.0);
        let dst = Rect::new(0.0, 100.0, 40.0, 20.0);
        let ((ps, ns), (pd, nd)) = ports(src, dst, Direction::TopBottom);
        assert_eq!(ps, Point::new(20.0, 20.0)); // bottom-center of src
        assert_eq!(ns, (0.0, 1.0));
        assert_eq!(pd, Point::new(20.0, 100.0)); // top-center of dst
        assert_eq!(nd, (0.0, -1.0));
    }
}
