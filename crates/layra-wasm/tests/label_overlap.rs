//! Layout-quality contract: an edge label must not sit on top of a node
//! it doesn't connect. Scans the whole blog corpus + a focused repro.
//!
//! Label box estimate mirrors the renderer (≈7px/char, 20px tall) so the
//! test reflects what users actually see.

use layra_core::{Document, Rect};

fn label_box(text: &str, cx: f32, cy: f32) -> Rect {
    let w = text.chars().count() as f32 * 7.0 + 12.0;
    let h = 20.0;
    Rect::new(cx - w / 2.0, cy - h / 2.0, w, h)
}

/// Returns the number of (edge label, intruded node) overlaps where the
/// node is neither endpoint of the edge.
fn label_node_overlaps(src: &str) -> Vec<String> {
    let (doc, _) = layra_parser::parse_document_lenient(src);
    let Document::Graph(mut g) = doc else {
        return Vec::new();
    };
    layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());
    layra_router::route(&mut g);

    let node_rects: Vec<Rect> = g.nodes.iter().map(|n| n.rect).collect();
    let mut hits = Vec::new();
    for e in &g.edges {
        let (Some(label), Some(pos)) = (&e.label, e.label_pos) else {
            continue;
        };
        let lbox = label_box(label, pos.x, pos.y);
        for (i, nr) in node_rects.iter().enumerate() {
            if i == e.source.index() || i == e.target.index() {
                continue;
            }
            // Shrink the node rect slightly: touching the border is fine,
            // only real intrusion counts.
            let nr = Rect::new(nr.x + 2.0, nr.y + 2.0, nr.width - 4.0, nr.height - 4.0);
            if nr.width > 0.0 && nr.height > 0.0 && lbox.intersects(&nr) {
                hits.push(format!(
                    "label {:?} intrudes node '{}'",
                    label, g.nodes[i].name
                ));
            }
        }
    }
    hits
}

#[test]
fn focused_repro_label_between_two_nodes() {
    // a -->|this is a long edge label| c, with b sitting between them on
    // the same rank — the label lands on b unless avoided.
    let src = "flowchart LR\n  a --> b\n  a -->|a fairly long edge label here| c\n  b --> c";
    let hits = label_node_overlaps(src);
    assert!(hits.is_empty(), "{} overlap(s): {hits:?}", hits.len());
}

#[test]
fn corpus_has_no_label_on_node() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");
    let mut total = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "mmd"))
        .collect();
    paths.sort();
    for p in paths {
        let src = std::fs::read_to_string(&p).unwrap();
        let hits = label_node_overlaps(&src);
        if !hits.is_empty() {
            total.push(format!(
                "{}: {}",
                p.file_name().unwrap().to_string_lossy(),
                hits.len()
            ));
        }
    }
    assert!(
        total.is_empty(),
        "{} diagram(s) with label-on-node: {total:?}",
        total.len()
    );
}
