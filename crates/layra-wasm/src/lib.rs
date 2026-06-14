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
    // Pre-load the bundled AWS-architecture-style infra set so diagrams can
    // use `{icon:aws:lambda}` with no external pack. User packs merge on top.
    REG.get_or_init(|| Mutex::new(layra_icons::IconRegistry::with_builtins()))
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
        layra_core::Document::Pie(chart) => {
            if chart.slices.is_empty() && !warnings.is_empty() {
                return Err(warnings.into_iter().next().unwrap());
            }
            layra_render_svg::render_pie(&chart, &theme)
        }
        layra_core::Document::Gantt(chart) => {
            if chart.tasks().next().is_none() && !warnings.is_empty() {
                return Err(warnings.into_iter().next().unwrap());
            }
            layra_render_svg::render_gantt(&chart, &theme)
        }
        layra_core::Document::Timeline(tl) => layra_render_svg::render_timeline(&tl, &theme),
        layra_core::Document::Journey(j) => layra_render_svg::render_journey(&j, &theme),
        layra_core::Document::Git(g) => layra_render_svg::render_git(&g, &theme),
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

/// Every bundled icon key with its service category and accent color, as
/// JSON: `{"count": N, "icons": [{"key": "aws:lambda", "ref":
/// "{icon:aws:lambda}", "category": "compute", "color": "#ED7100"}, ...]}`.
/// Backs the MCP `list_icons` tool so an agent can author `{icon:...}` refs
/// that actually render instead of guessing names.
pub fn list_icons_json() -> Result<String, String> {
    let reg = registry().lock().map_err(|e| e.to_string())?;
    let icons: Vec<serde_json::Value> = reg
        .entries()
        .into_iter()
        .map(|(key, category)| {
            serde_json::json!({
                "key": key,
                "ref": format!("{{icon:{key}}}"),
                "category": category.map(|c| c.name()),
                "color": category.map(|c| c.color()),
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "count": icons.len(),
        "note": "Use a ref inside a node label, e.g. id[\"{icon:aws:lambda} Worker\"]. The aws: and infra: prefixes alias the same set.",
        "icons": icons,
    }))
    .map_err(|e| e.to_string())
}

/// Lint a diagram beyond parse errors: structured quality warnings an agent
/// can act on. Returns JSON `{"ok": bool, "warnings": [{"severity","line?",
/// "kind","message"}]}`. Detects unparseable lines (from the lenient parse),
/// orphan nodes (a node with no edges in a multi-node graph), and labels
/// whose measured text would overflow their node box.
pub fn lint_diagram_json(source: &str) -> Result<String, String> {
    use serde_json::json;
    if source.trim().is_empty() {
        return Err("source is empty".into());
    }
    let (doc, parse_warnings) = layra_parser::parse_document_lenient(source);
    let mut warnings: Vec<serde_json::Value> = parse_warnings
        .iter()
        .map(|e| {
            json!({
                "severity": "error",
                "line": e.line(),
                "kind": "parse",
                "message": e.message(),
            })
        })
        .collect();

    if let layra_core::Document::Graph(mut graph) = doc {
        // Orphans: in a graph with several nodes, a node touched by no edge is
        // usually an accident (typo'd edge endpoint, forgotten arrow).
        if graph.nodes.len() > 1 {
            let mut connected = std::collections::HashSet::new();
            for edge in &graph.edges {
                connected.insert(edge.source);
                connected.insert(edge.target);
            }
            for (i, node) in graph.nodes.iter().enumerate() {
                let id = layra_core::NodeId(i as u32);
                if !connected.contains(&id) {
                    warnings.push(json!({
                        "severity": "warning",
                        "kind": "orphan_node",
                        "message": format!(
                            "node '{}' has no edges — connect it or remove it",
                            node.name
                        ),
                    }));
                }
            }
        }

        // Label overflow: measure + lay out, then flag any node whose label is
        // wider than its box (the engine should size to fit; a flag here means
        // a pathological label the author may want to shorten).
        layra_text::measure_graph(&mut graph, &layra_text::TextOptions::default());
        layra_layout::layout(&mut graph, &layra_layout::LayoutOptions::default());
        for node in &graph.nodes {
            let text_w = layra_text::measure_line(&node.label, 14.0);
            if text_w > node.rect.width + 0.5 {
                warnings.push(json!({
                    "severity": "warning",
                    "kind": "label_overflow",
                    "message": format!(
                        "label '{}' (~{:.0}px) is wider than node '{}' box ({:.0}px)",
                        node.label, text_w, node.name, node.rect.width
                    ),
                }));
            }
        }
    }

    serde_json::to_string_pretty(&json!({
        "ok": warnings.is_empty(),
        "count": warnings.len(),
        "warnings": warnings,
    }))
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
        layra_core::Document::Pie(chart) => {
            serde_json::json!({ "kind": "pie", "pie": chart })
        }
        layra_core::Document::Gantt(chart) => {
            serde_json::json!({ "kind": "gantt", "gantt": chart })
        }
        layra_core::Document::Timeline(tl) => {
            serde_json::json!({ "kind": "timeline", "timeline": tl })
        }
        layra_core::Document::Journey(j) => {
            serde_json::json!({ "kind": "journey", "journey": j })
        }
        layra_core::Document::Git(g) => {
            serde_json::json!({ "kind": "git", "git": g })
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

    /// L13: class-diagram inheritance must rank the PARENT above the CHILD.
    /// `Animal <|-- Dog` (Dog extends Animal) must lay Animal out on the
    /// upper layer and the hollow generalization triangle must point UP to
    /// the parent (rendered as a `marker-start` at the source = parent).
    #[test]
    fn class_inheritance_parent_above_child() {
        use layra_core::Document;
        let src = "classDiagram\n    Animal <|-- Dog\n    Animal <|-- Cat";
        let (doc, warnings) = layra_parser::parse_document_lenient(src);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        let Document::Graph(mut g) = doc else {
            panic!("class diagram should parse to a graph");
        };
        layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
        layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());

        let animal = g.node(g.node_by_name("Animal").unwrap()).rect;
        let dog = g.node(g.node_by_name("Dog").unwrap()).rect;
        let cat = g.node(g.node_by_name("Cat").unwrap()).rect;
        assert!(
            animal.y < dog.y,
            "parent Animal (y={}) must be above child Dog (y={})",
            animal.y,
            dog.y
        );
        assert!(
            animal.y < cat.y,
            "parent Animal (y={}) must be above child Cat (y={})",
            animal.y,
            cat.y
        );

        // And the rendered triangle is a marker-start so its apex points up
        // to the parent, never a downward marker-end.
        let svg = render_svg(src, false).unwrap();
        assert!(
            svg.contains(r##"marker-start="url(#triangle)""##),
            "generalization triangle must sit at the parent (marker-start)"
        );
        assert!(
            !svg.contains(r##"marker-end="url(#triangle)""##),
            "triangle must not point down via marker-end"
        );
    }

    /// L14: text must never overflow its box. Reproduce the reported case
    /// (`+makeSound() void` and a wider signature spilling out of a class
    /// member row) and assert every member row + the title fit inside the
    /// node rect, using the same monospace geometry the renderer draws with.
    #[test]
    fn class_member_rows_fit_inside_box() {
        use layra_core::Document;
        use layra_text::compartment::{MEMBER_FONT, PAD_X, TITLE_FONT};
        let src = "classDiagram\n    class Animal {\n      +String name\n      +makeSound() void\n      +eat(food: String) boolean\n      +describeYourselfInDetail() String\n    }";
        let (doc, warnings) = layra_parser::parse_document_lenient(src);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        let Document::Graph(mut g) = doc else {
            panic!("class diagram should parse to a graph");
        };
        layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
        layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());

        let n = g.node(g.node_by_name("Animal").unwrap());
        let r = n.rect;
        // Title is centered and bold: its half-width must fit within the box.
        let title_half = layra_text::measure_line(&n.label, TITLE_FONT) / 2.0;
        assert!(
            r.center().x - title_half >= r.x - 0.5 && r.center().x + title_half <= r.right() + 0.5,
            "title '{}' overflows box (half={title_half}, rect={r:?})",
            n.label
        );
        // Every monospace member row, drawn left-aligned at x + PAD_X, must
        // end before the right edge of the box.
        for section in &n.sections {
            for line in section.split('\n') {
                let extent = r.x + PAD_X + layra_text::measure_line_mono(line, MEMBER_FONT);
                assert!(
                    extent <= r.right() + 0.5,
                    "member row '{line}' overflows: extent={extent:.1} > right={:.1}",
                    r.right()
                );
            }
        }
    }

    /// L14: edge-label chips must fully cover their text. The old chip width
    /// used `byte_len * 7`, which under-measures wide glyphs (W/m/@), letting
    /// the text spill out of its background pill. The chip is now sized from
    /// the measured advance, so the rendered chip width always covers the
    /// rendered text width.
    #[test]
    fn edge_label_chip_covers_text() {
        use layra_core::Document;
        // A label of wide glyphs: byte_len*7 = 82px would under-measure the
        // ~114px of drawn text, clipping it under the old formula.
        let src = "flowchart LR\n  A -->|WWWWWWWWWW| B";
        let (doc, _) = layra_parser::parse_document_lenient(src);
        let Document::Graph(mut g) = doc else {
            panic!("flowchart should parse to a graph");
        };
        layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
        layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());
        layra_router::route(&mut g);
        let svg = render_svg(src, false).unwrap();

        let label = g.edges[0].label.as_ref().unwrap();
        let text_w = layra_text::measure_line(label, 12.0);
        // Find the chip rect width the renderer emitted (width="..." right
        // before the label text). It must be at least the measured text width.
        let chip_w = svg
            .split("width=\"")
            .filter_map(|s| s.split('"').next())
            .filter_map(|s| s.parse::<f32>().ok())
            .find(|&w| (w - (text_w + 12.0)).abs() < 1.0)
            .unwrap_or(0.0);
        assert!(
            chip_w >= text_w,
            "chip width {chip_w} must cover wide label text {text_w}"
        );
    }

    /// D3: `list_icons` exposes the full bundled set with category + color so
    /// an agent can reference glyphs that actually render.
    #[test]
    fn list_icons_reports_full_set_with_categories() {
        let json = list_icons_json().unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let count = v["count"].as_u64().unwrap();
        assert!(count >= 100, "expected the full bundled set, got {count}");

        let icons = v["icons"].as_array().unwrap();
        // Every entry has a usable {icon:...} ref.
        assert!(icons.iter().all(|i| i["ref"]
            .as_str()
            .is_some_and(|r| r.starts_with("{icon:") && r.ends_with('}'))));
        // A known compute glyph is categorized with the AWS orange accent.
        let lambda = icons
            .iter()
            .find(|i| i["key"] == "aws:lambda")
            .expect("aws:lambda must be bundled");
        assert_eq!(lambda["category"], "compute");
        assert_eq!(lambda["color"], "#ED7100");
    }

    /// D3: `lint_diagram` flags an orphan node (no edges) but passes a clean
    /// connected graph.
    #[test]
    fn lint_flags_orphan_node_but_passes_clean_graph() {
        let clean = lint_diagram_json("flowchart TD\n  A[Start] --> B[End]").unwrap();
        let cv: serde_json::Value = serde_json::from_str(&clean).unwrap();
        assert_eq!(cv["ok"], true, "clean graph should lint ok: {clean}");

        let dirty = lint_diagram_json("flowchart TD\n  A[Start] --> B[End]\n  C[Orphan]").unwrap();
        let dv: serde_json::Value = serde_json::from_str(&dirty).unwrap();
        assert_eq!(dv["ok"], false);
        let kinds: Vec<&str> = dv["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|w| w["kind"].as_str())
            .collect();
        assert!(kinds.contains(&"orphan_node"), "got {kinds:?}");
    }

    /// D3: `lint_diagram` surfaces an unparseable line with its line number.
    #[test]
    fn lint_reports_parse_error_with_line() {
        let json = lint_diagram_json("flowchart TD\n  A --> B\n  @@@ broken @@@").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let parse_w = v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["kind"] == "parse")
            .expect("a parse warning");
        assert_eq!(parse_w["severity"], "error");
        assert_eq!(parse_w["line"], 3);
    }
}
