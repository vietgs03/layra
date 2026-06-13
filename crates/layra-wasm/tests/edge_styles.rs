//! L6 contract: edge style/kind/animation must survive the full pipeline
//! (parse -> layout -> orthogonal route -> render) and emit the right SVG.
//!
//! Matrix: every edge *style* (solid/thick/dashed/dotted/invisible) crossed
//! with every *direction* (TB/LR/BT/RL) routes and renders with the correct
//! `stroke-dasharray` / `stroke-width`, every arrowhead *kind* emits the right
//! marker, and the `{animate}` hint adds a flowing `<animate>`.

use layra_wasm::render_svg;

/// Pull the `<path ...>` for the first edge (`<g data-edge="0">...</g>`).
/// Returns the whole group body (may be empty for invisible edges).
fn first_edge_group(svg: &str) -> String {
    let start = svg.find(r#"<g data-edge="0""#).expect("edge group present");
    let body = &svg[start..];
    let end = body.find("</g>").expect("group closes");
    body[..end].to_string()
}

const DIRECTIONS: &[&str] = &["TB", "LR", "BT", "RL"];

#[test]
fn solid_edge_routes_and_renders_arrow_no_dash() {
    for dir in DIRECTIONS {
        let svg = render_svg(
            &format!("flowchart {dir}\n  a[\"A\"] --> b[\"B\"]\n"),
            false,
        )
        .unwrap();
        let g = first_edge_group(&svg);
        assert!(g.contains("<path"), "[{dir}] solid edge must draw a path");
        assert!(
            g.contains(r#"stroke-width="1.6""#),
            "[{dir}] solid stroke-width"
        );
        assert!(
            !g.contains("stroke-dasharray"),
            "[{dir}] solid must have no dash"
        );
        assert!(
            g.contains(r#"marker-end="url(#arrow)""#),
            "[{dir}] solid arrowhead"
        );
    }
}

#[test]
fn thick_edge_routes_and_renders_wide_no_dash() {
    for dir in DIRECTIONS {
        let svg = render_svg(
            &format!("flowchart {dir}\n  a[\"A\"] ==> b[\"B\"]\n"),
            false,
        )
        .unwrap();
        let g = first_edge_group(&svg);
        assert!(g.contains("<path"), "[{dir}] thick edge must draw a path");
        assert!(
            g.contains(r#"stroke-width="3""#),
            "[{dir}] thick stroke-width 3"
        );
        assert!(
            !g.contains("stroke-dasharray"),
            "[{dir}] thick must have no dash"
        );
    }
}

#[test]
fn dashed_edge_routes_and_renders_dash_7_5() {
    for dir in DIRECTIONS {
        let svg = render_svg(
            &format!("flowchart {dir}\n  a[\"A\"] -.-> b[\"B\"]\n"),
            false,
        )
        .unwrap();
        let g = first_edge_group(&svg);
        assert!(
            g.contains(r#"stroke-dasharray="7 5""#),
            "[{dir}] dashed dasharray"
        );
    }
}

#[test]
fn dotted_edge_routes_and_renders_dash_2_4() {
    for dir in DIRECTIONS {
        // `-..->` (two dots) is the dotted operator, distinct from `-.->`.
        let svg = render_svg(
            &format!("flowchart {dir}\n  a[\"A\"] -..-> b[\"B\"]\n"),
            false,
        )
        .unwrap();
        let g = first_edge_group(&svg);
        assert!(
            g.contains(r#"stroke-dasharray="2 4""#),
            "[{dir}] dotted dasharray, got: {g}"
        );
    }
}

#[test]
fn invisible_edge_routes_but_draws_nothing() {
    for dir in DIRECTIONS {
        let svg = render_svg(
            &format!("flowchart {dir}\n  a[\"A\"] ~~~ b[\"B\"]\n"),
            false,
        )
        .unwrap();
        let g = first_edge_group(&svg);
        assert!(
            !g.contains("<path"),
            "[{dir}] invisible edge must not draw a path, got: {g}"
        );
    }
}

#[test]
fn arrowhead_kinds_emit_correct_markers() {
    // open line: no marker
    let svg = render_svg("flowchart LR\n  a[\"A\"] --- b[\"B\"]\n", false).unwrap();
    let g = first_edge_group(&svg);
    assert!(g.contains("<path"), "open edge draws a line");
    assert!(!g.contains("marker-end"), "open edge has no arrowhead");

    // bidirectional: both ends
    let svg = render_svg("flowchart LR\n  a[\"A\"] <--> b[\"B\"]\n", false).unwrap();
    let g = first_edge_group(&svg);
    assert!(
        g.contains(r#"marker-end="url(#arrow)""#) && g.contains(r#"marker-start="url(#arrow)""#),
        "bidirectional edge has arrows at both ends, got: {g}"
    );
}

#[test]
fn animated_hint_adds_flowing_dash() {
    // `{animate}` directive in the pipe label (mirrors `{icon:...}`).
    let svg = render_svg(
        "flowchart LR\n  a[\"A\"] -->|{animate} sync| b[\"B\"]\n",
        false,
    )
    .unwrap();
    let g = first_edge_group(&svg);
    assert!(
        g.contains("<animate") && g.contains("stroke-dashoffset"),
        "animated edge emits an <animate> on stroke-dashoffset, got: {g}"
    );
    // animation forces a dash so the flow is visible even on a solid edge.
    assert!(
        g.contains("stroke-dasharray"),
        "animated edge has a dash pattern to flow"
    );
    // the directive is stripped from the visible label.
    assert!(
        svg.contains(">sync<"),
        "label keeps text minus the directive"
    );
    assert!(!svg.contains("{animate}"), "directive token is consumed");
}

#[test]
fn animated_preserves_underlying_style() {
    // animate a dashed edge: keeps its own 7 5 dash, still animates.
    let svg = render_svg("flowchart LR\n  a[\"A\"] -.->|{animate}| b[\"B\"]\n", false).unwrap();
    let g = first_edge_group(&svg);
    assert!(
        g.contains(r#"stroke-dasharray="7 5""#),
        "keeps dashed pattern"
    );
    assert!(g.contains("<animate"), "still animated");
}
