//! L4 contract (corpus): no node label's rendered text advance exceeds its
//! laid-out node box width. The text-measure stage sizes boxes from an
//! upper-bound metric table, so after layout the label must always fit.

use layra_core::Document;

/// Reference rendered advance for the *title* line of a label at the
/// renderer's title font size (14px). Uses the engine's own measurement,
/// which is the documented upper bound, so this asserts the pipeline keeps
/// the box wide enough for what it will draw.
fn title_advance(label: &str) -> f32 {
    let title = label.split('\n').next().unwrap_or("");
    layra_text::measure_line(title, 14.0)
}

fn check_corpus_file(src: &str) -> Vec<String> {
    let (doc, _) = layra_parser::parse_document_lenient(src);
    let Document::Graph(mut g) = doc else {
        return Vec::new();
    };
    layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());

    let mut overflows = Vec::new();
    for n in &g.nodes {
        if n.label.is_empty() || !n.sections.is_empty() {
            continue; // compartment nodes size differently
        }
        // The drawable interior is the box minus a small inset; the title
        // is centered, so it must fit within the full width. Diamonds /
        // circles only expose part of their width to text, but the measure
        // stage already inflates those boxes (shape_factor), so the title
        // advance must still be <= the box width.
        let advance = title_advance(&n.label);
        if advance > n.rect.width + 0.5 {
            overflows.push(format!(
                "{:?} (label {:?}): advance {advance:.1} > box {:.1}",
                n.name, n.label, n.rect.width
            ));
        }
    }
    overflows
}

#[test]
fn no_label_overflows_node_box_on_corpus() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");
    let mut all = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|e| e == "mmd") {
            let src = std::fs::read_to_string(&p).unwrap();
            for v in check_corpus_file(&src) {
                all.push(format!("{}: {v}", p.display()));
            }
        }
    }
    assert!(all.is_empty(), "label overflow(s):\n{}", all.join("\n"));
}

#[test]
fn wide_caps_label_fits_its_box() {
    // All-caps wide letters used to under-measure and overflow.
    let src = "flowchart LR\n  g[\"WWWW MMMM GROWTH\"] --> x[\"OK\"]\n";
    let (doc, _) = layra_parser::parse_document_lenient(src);
    let Document::Graph(mut g) = doc else {
        panic!("graph");
    };
    layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
    layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());
    for n in &g.nodes {
        assert!(
            title_advance(&n.label) <= n.rect.width + 0.5,
            "label {:?} overflows box {:.1}",
            n.label,
            n.rect.width
        );
    }
}
