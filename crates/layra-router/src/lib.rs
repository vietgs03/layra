//! # layra-router
//!
//! Edge routing. Today: clip endpoints to node borders, pass layout
//! waypoints through, and place labels at the polyline midpoint.
//!
//! Planned: orthogonal routing on a visibility graph with A* and bend
//! penalties (libavoid-style) for draw.io-quality edges.

use layra_core::{Graph, Point, Rect};

pub fn route(graph: &mut Graph) {
    let rects: Vec<Rect> = graph.nodes.iter().map(|n| n.rect).collect();

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

        // Label at geometric midpoint of the polyline.
        if edge.label.is_some() {
            edge.label_pos = Some(polyline_midpoint(&edge.points));
        }
    }
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

fn polyline_midpoint(points: &[Point]) -> Point {
    let total: f32 = points
        .windows(2)
        .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
        .sum();
    let mut remaining = total / 2.0;
    for w in points.windows(2) {
        let seg = ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
        if seg >= remaining && seg > 0.0 {
            let f = remaining / seg;
            return Point::new(
                w[0].x + (w[1].x - w[0].x) * f,
                w[0].y + (w[1].y - w[0].y) * f,
            );
        }
        remaining -= seg;
    }
    points[points.len() / 2]
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
        let m = polyline_midpoint(&pts);
        assert!((m.x - 5.0).abs() < 0.01);
    }
}
