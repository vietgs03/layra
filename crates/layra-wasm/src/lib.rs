//! # layra-wasm
//!
//! Thin WASM facade over the whole pipeline:
//! `parse → measure → layout → route → render SVG`.
//!
//! Also usable as a plain Rust library (`render_svg`) — the CLI and any
//! server-side rendering reuse the same entry point.

use std::sync::{Mutex, OnceLock};
use wasm_bindgen::prelude::*;

fn registry() -> &'static Mutex<layra_icons::IconRegistry> {
    static REG: OnceLock<Mutex<layra_icons::IconRegistry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(layra_icons::IconRegistry::new()))
}

/// Full pipeline: diagram source in, SVG string out.
pub fn render_svg(source: &str, dark: bool) -> Result<String, String> {
    let mut graph = layra_parser::parse(source).map_err(|e| e.to_string())?;
    layra_text::measure_graph(&mut graph, &layra_text::TextOptions::default());
    layra_layout::layout(&mut graph, &layra_layout::LayoutOptions::default());
    layra_router::route(&mut graph);
    let theme = if dark {
        layra_render_svg::Theme::dark()
    } else {
        layra_render_svg::Theme::light()
    };
    let reg = registry().lock().map_err(|e| e.to_string())?;
    let icons = (!reg.is_empty()).then_some(&*reg);
    Ok(layra_render_svg::render_with_icons(&graph, &theme, icons))
}

/// Load an Iconify-format icon pack JSON. Returns the number of icons
/// added. Call any number of times; packs merge.
pub fn load_icon_pack(json: &str) -> Result<usize, String> {
    registry()
        .lock()
        .map_err(|e| e.to_string())?
        .load_pack(json)
        .map_err(|e| e.to_string())
}

/// JS-facing entry point. Throws a JS error with the parse message on
/// failure (carrying the line number).
#[wasm_bindgen]
pub fn render(source: &str, dark: bool) -> Result<String, JsError> {
    render_svg(source, dark).map_err(|e| JsError::new(&e))
}

/// JS-facing icon pack loader.
#[wasm_bindgen]
pub fn load_icons(json: &str) -> Result<usize, JsError> {
    load_icon_pack(json).map_err(|e| JsError::new(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_to_end_pipeline() {
        let svg = render_svg(
            r#"flowchart LR
              client(["Browser"]):::client --> api["API Gateway"]:::gateway
              api --> orders["Order Service"]:::service
              api --> users["User Service"]:::service
              orders -->|persist| db[("Postgres")]:::database
              orders -.->|events| mq{{Kafka}}
              users --> cache(("Redis")):::cache
              subgraph data["Data Plane"]
                db
                mq
                cache
              end
            "#,
            false,
        )
        .unwrap();

        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("Postgres"));
        assert!(svg.contains("Data Plane"));
        assert!(svg.contains("marker-end")); // arrows rendered
        assert!(svg.contains("stroke-dasharray=\"7 5\"")); // dashed event edge
    }

    #[test]
    fn parse_error_carries_line() {
        let err = render_svg("flowchart LR\nend", false).unwrap_err();
        assert!(err.contains("line 2"), "got: {err}");
    }
}
