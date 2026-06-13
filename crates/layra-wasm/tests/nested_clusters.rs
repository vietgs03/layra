//! L3 contract (parser path): a 2-level nested subgraph parsed from Mermaid
//! source must produce nested cluster rects — inner fully inside outer with
//! padding.

use layra_core::{Document, Rect};

fn assert_nested(outer: Rect, inner: Rect, pad: f32) {
    let m = pad - 0.5;
    assert!(
        outer.x + m <= inner.x
            && outer.y + m <= inner.y
            && inner.right() + m <= outer.right()
            && inner.bottom() + m <= outer.bottom(),
        "inner {inner:?} not nested inside outer {outer:?} with padding {pad}"
    );
}

#[test]
fn nested_clusters_via_parser_enclose() {
    let src = "flowchart TB\n\
        subgraph outer[\"Outer\"]\n\
          hub[\"Hub\"]\n\
          subgraph inner[\"Inner\"]\n\
            a[\"A\"]\n\
            b[\"B\"]\n\
          end\n\
        end\n\
        hub --> a\n\
        a --> b\n";
    let (doc, _) = layra_parser::parse_document_lenient(src);
    let Document::Graph(mut g) = doc else {
        panic!("expected graph");
    };
    layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
    let opts = layra_layout::LayoutOptions::default();
    layra_layout::layout(&mut g, &opts);

    let outer = g.subgraphs.iter().position(|s| s.name == "outer").unwrap();
    let inner = g.subgraphs.iter().position(|s| s.name == "inner").unwrap();
    assert!(g.subgraphs[outer].parent.is_none(), "outer is top-level");
    assert_eq!(
        g.subgraphs[inner].parent.map(|p| p.index()),
        Some(outer),
        "inner must record outer as its parent"
    );
    assert_nested(
        g.subgraphs[outer].rect,
        g.subgraphs[inner].rect,
        opts.cluster_padding,
    );
}

#[test]
fn nested_clusters_render_without_panic() {
    let src = "flowchart LR\n\
        subgraph cloud[\"Cloud\"]\n\
          subgraph vpc[\"VPC\"]\n\
            lb[\"LB\"] --> app[\"App\"]\n\
          end\n\
          app --> db[(\"DB\")]\n\
        end\n";
    let svg = layra_wasm::render_svg(src, false).expect("render nested");
    assert!(svg.contains("Cloud") && svg.contains("VPC"));
}
