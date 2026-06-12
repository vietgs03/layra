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

/// Full pipeline: diagram source in, SVG string out. Dispatches on diagram
/// type — flowcharts and state diagrams run the graph pipeline; sequence
/// diagrams use the dedicated deterministic layout. Strict: the first
/// unparseable line is an error.
pub fn render_svg(source: &str, dark: bool) -> Result<String, String> {
    let (svg, warnings) = render_svg_lenient(source, dark)?;
    if let Some(w) = warnings.into_iter().next() {
        return Err(w);
    }
    Ok(svg)
}

/// Lenient pipeline: skips unparseable lines (mangled copy-paste, partial
/// edits) and renders everything that did parse, returning per-line
/// warnings. Hard-fails only when nothing usable was found.
pub fn render_svg_lenient(source: &str, dark: bool) -> Result<(String, Vec<String>), String> {
    let (doc, parse_warnings) = layra_parser::parse_document_lenient(source);
    let warnings: Vec<String> = parse_warnings.iter().map(|e| e.to_string()).collect();

    let theme = if dark {
        layra_render_svg::Theme::dark()
    } else {
        layra_render_svg::Theme::light()
    };

    let svg = match doc {
        layra_core::Document::Graph(mut graph) => {
            if graph.nodes.is_empty() && !warnings.is_empty() {
                return Err(warnings.into_iter().next().unwrap());
            }
            layra_text::measure_graph(&mut graph, &layra_text::TextOptions::default());
            layra_layout::layout(&mut graph, &layra_layout::LayoutOptions::default());
            layra_router::route(&mut graph);
            let reg = registry().lock().map_err(|e| e.to_string())?;
            let icons = (!reg.is_empty()).then_some(&*reg);
            layra_render_svg::render_with_icons(&graph, &theme, icons)
        }
        layra_core::Document::Sequence(seq) => {
            if seq.participants.is_empty() && !warnings.is_empty() {
                return Err(warnings.into_iter().next().unwrap());
            }
            layra_render_svg::render_sequence(&seq, &theme)
        }
    };
    Ok((svg, warnings))
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

/// JS-facing lenient entry point. Returns JSON:
/// `{"svg": "...", "warnings": ["line N: ...", ...]}`.
/// Throws only when nothing in the source could be parsed.
#[wasm_bindgen]
pub fn render_lenient(source: &str, dark: bool) -> Result<String, JsError> {
    let (svg, warnings) = render_svg_lenient(source, dark).map_err(|e| JsError::new(&e))?;
    serde_json::to_string(&serde_json::json!({ "svg": svg, "warnings": warnings }))
        .map_err(|e| JsError::new(&e.to_string()))
}

/// JS-facing icon pack loader.
#[wasm_bindgen]
pub fn load_icons(json: &str) -> Result<usize, JsError> {
    load_icon_pack(json).map_err(|e| JsError::new(&e))
}

/// Structured pipeline output: parse + measure + layout + route, then
/// return the **laid-out document as JSON** instead of SVG. For consumers
/// that render themselves (Canvas/WebGL/React/D3) or need hit-testing,
/// animation, custom theming — Rust does the expensive math, JS/TS owns
/// the pixels.
///
/// Graph JSON shape: `{ "kind": "graph", "nodes": [{name,label,shape,role,
/// icon,rect:{x,y,width,height}}], "edges": [{source,target,label,style,
/// kind,points:[{x,y}],label_pos}], "subgraphs": [...], "bounds": {...} }`
#[wasm_bindgen]
pub fn layout_json(source: &str) -> Result<String, JsError> {
    let (doc, _) = layra_parser::parse_document_lenient(source);
    let value = match doc {
        layra_core::Document::Graph(mut graph) => {
            layra_text::measure_graph(&mut graph, &layra_text::TextOptions::default());
            layra_layout::layout(&mut graph, &layra_layout::LayoutOptions::default());
            layra_router::route(&mut graph);
            let bounds = graph.bounds();
            serde_json::json!({ "kind": "graph", "bounds": bounds, "graph": graph })
        }
        layra_core::Document::Sequence(seq) => {
            serde_json::json!({ "kind": "sequence", "sequence": seq })
        }
    };
    serde_json::to_string(&value).map_err(|e| JsError::new(&e.to_string()))
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
