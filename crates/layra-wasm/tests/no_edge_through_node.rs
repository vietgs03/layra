//! Quality contract: after routing, no edge segment may pass through a
//! node that is not its endpoint. This is the visible difference between
//! "Mermaid-ish" and "draw.io-ish" output.

use layra_core::{Document, Point, Rect};

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

fn check(src: &str) -> usize {
    let (doc, _) = layra_parser::parse_document_lenient(src);
    let Document::Graph(mut g) = doc else {
        return 0;
    };
    layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());
    layra_router::route(&mut g);

    let mut violations = 0;
    for e in &g.edges {
        for w in e.points.windows(2) {
            for (i, n) in g.nodes.iter().enumerate() {
                if i == e.source.index() || i == e.target.index() {
                    continue;
                }
                if seg_hits(w[0], w[1], &n.rect) {
                    violations += 1;
                }
            }
        }
    }
    violations
}

#[test]
fn skip_connection_routes_around_middle_node() {
    // a->b->c->d plus a long a->d edge that would naturally cut through b/c.
    let src = "flowchart TB\n  a --> b --> c --> d\n  a --> d";
    assert_eq!(check(src), 0);
}

#[test]
fn blog_corpus_has_no_edge_through_node() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");
    let mut total = 0;
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|e| e == "mmd") {
            let src = std::fs::read_to_string(&p).unwrap();
            let v = check(&src);
            if v > 0 {
                eprintln!("{p:?}: {v} violations");
            }
            total += v;
        }
    }
    assert_eq!(total, 0, "{total} edge-through-node violations in corpus");
}
