//! L1 quality contract: orthogonal routing is the DEFAULT for flowcharts.
//!
//! Every visible, non-self-loop edge must be a piecewise axis-aligned
//! polyline (each segment horizontal OR vertical within tolerance) and no
//! segment may pass through a node that is not its endpoint. This is the
//! draw.io / AWS-architecture look, replacing diagonal straight lines.

use layra_core::{Document, EdgeStyle, Point, Rect};

const TOL: f32 = 1.0;

fn is_axis_aligned(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= TOL || (a.y - b.y).abs() <= TOL
}

fn seg_hits(a: Point, b: Point, r: &Rect) -> bool {
    for t in 0..=32 {
        let f = t as f32 / 32.0;
        let p = Point::new(a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f);
        if p.x > r.x + 0.5 && p.x < r.right() - 0.5 && p.y > r.y + 0.5 && p.y < r.bottom() - 0.5 {
            return true;
        }
    }
    false
}

struct Report {
    diagonal: usize,
    through_node: usize,
}

fn analyze(src: &str) -> Report {
    let (doc, _) = layra_parser::parse_document_lenient(src);
    let mut rep = Report {
        diagonal: 0,
        through_node: 0,
    };
    let Document::Graph(mut g) = doc else {
        return rep;
    };
    layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());
    layra_router::route(&mut g);

    for e in &g.edges {
        // Self-loops draw a lasso (curved); invisible links draw nothing.
        if e.source == e.target || e.style == EdgeStyle::Invisible || e.points.len() < 2 {
            continue;
        }
        for w in e.points.windows(2) {
            if !is_axis_aligned(w[0], w[1]) {
                rep.diagonal += 1;
            }
            for (i, n) in g.nodes.iter().enumerate() {
                if i == e.source.index() || i == e.target.index() {
                    continue;
                }
                if seg_hits(w[0], w[1], &n.rect) {
                    rep.through_node += 1;
                }
            }
        }
    }
    rep
}

#[test]
fn diamond_edges_are_axis_aligned() {
    // Classic diamond: b and c sit offset left/right of a, so naive
    // straight routing draws diagonals a->b and a->c.
    let src = "flowchart TB\n  a --> b\n  a --> c\n  b --> d\n  c --> d";
    let rep = analyze(src);
    assert_eq!(rep.diagonal, 0, "every segment must be axis-aligned");
    assert_eq!(rep.through_node, 0, "no edge may cross a non-endpoint node");
}

#[test]
fn skip_edges_route_orthogonally_around_nodes() {
    // a->b->c->d chain plus a long a->d skip that must weave around b/c.
    let src = "flowchart TB\n  a --> b --> c --> d\n  a --> d";
    let rep = analyze(src);
    assert_eq!(rep.diagonal, 0, "axis-aligned even when routing around");
    assert_eq!(rep.through_node, 0);
}

#[test]
fn synthetic_grid_is_orthogonal() {
    // A wide fan of offset targets: each edge from the hub to a leaf is
    // horizontally offset, so diagonal routing would be rampant.
    let mut src = String::from("flowchart TB\n");
    for i in 0..8 {
        src.push_str(&format!("  hub --> leaf{i}\n"));
    }
    for i in 0..7 {
        src.push_str(&format!("  leaf{i} --> sink\n"));
    }
    let rep = analyze(&src);
    assert_eq!(rep.diagonal, 0, "fan edges must be axis-aligned");
    assert_eq!(rep.through_node, 0);
}

#[test]
fn corpus_edges_are_axis_aligned() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");
    let mut diagonal = 0;
    let mut through = 0;
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|e| e == "mmd") {
            let s = std::fs::read_to_string(&p).unwrap();
            let rep = analyze(&s);
            if rep.diagonal > 0 || rep.through_node > 0 {
                eprintln!(
                    "{p:?}: {} diagonal, {} through-node",
                    rep.diagonal, rep.through_node
                );
            }
            diagonal += rep.diagonal;
            through += rep.through_node;
        }
    }
    assert_eq!(diagonal, 0, "{diagonal} diagonal segments across corpus");
    assert_eq!(through, 0, "{through} through-node segments across corpus");
}
