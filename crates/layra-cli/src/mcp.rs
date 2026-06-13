//! `layra mcp` — Model Context Protocol server over stdio.
//!
//! Exposes the renderer as agent tools so AI coding assistants (Claude
//! Code, Cursor, Zed, Cline, ...) can generate a diagram, validate it,
//! fix their own syntax errors, and emit the final SVG — all without a
//! human in the loop.
//!
//! Tools:
//! - `validate_diagram { source }` → ok / per-line errors. The agent's
//!   inner loop: generate → validate → fix → repeat.
//! - `render_diagram { source, path?, dark? }` → SVG written to `path`
//!   (or returned inline when no path given).
//! - `list_shapes {}` → the node shapes, role classes, edge styles, and
//!   icon syntax the engine understands, so an agent knows what it can use.
//!
//! Implements the JSON-RPC subset MCP needs (initialize, tools/list,
//! tools/call) by hand — the protocol surface is 3 methods; a framework
//! would be heavier than the implementation.

use std::io::{BufRead, Write};

pub(crate) fn serve() -> ! {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let result = match method {
            "initialize" => Some(initialize_result()),
            "notifications/initialized" | "initialized" => None, // notification, no reply
            "tools/list" => Some(tools_list()),
            // Optional capabilities we don't provide: respond with empty
            // lists instead of -32601 — several clients log method-not-found
            // as a hard error and disable the server.
            "resources/list" => Some(serde_json::json!({ "resources": [] })),
            "resources/templates/list" => Some(serde_json::json!({ "resourceTemplates": [] })),
            "prompts/list" => Some(serde_json::json!({ "prompts": [] })),
            "tools/call" => Some(tools_call(req.get("params"))),
            "ping" => Some(serde_json::json!({})),
            _ => {
                if let Some(id) = id {
                    respond_err(&stdout, id, -32601, &format!("method not found: {method}"));
                }
                continue;
            }
        };

        if let (Some(result), Some(id)) = (result, id) {
            respond(&stdout, id, result);
        }
    }
    std::process::exit(0);
}

fn initialize_result() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "layra",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn tools_list() -> serde_json::Value {
    serde_json::json!({
        "tools": [
            {
                "name": "validate_diagram",
                "description": "Validate Mermaid-compatible diagram source (flowchart, sequenceDiagram, stateDiagram-v2, classDiagram, erDiagram, gantt, pie, mindmap, timeline, journey, gitGraph). Returns ok or per-line syntax errors. Call this after generating diagram source and fix any reported lines before presenting it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string", "description": "Diagram source text" }
                    },
                    "required": ["source"]
                }
            },
            {
                "name": "render_diagram",
                "description": "Render Mermaid-compatible diagram source to SVG. Provide `path` to write the file (recommended) or omit it to get the SVG text inline. Use validate_diagram first if unsure the source parses.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string", "description": "Diagram source text" },
                        "path":   { "type": "string", "description": "Output .svg file path (optional)" },
                        "dark":   { "type": "boolean", "description": "Dark theme (default false)" }
                    },
                    "required": ["source"]
                }
            },
            {
                "name": "list_shapes",
                "description": "List the node shapes, role classes, and edge styles the Layra engine supports, with the exact flowchart syntax for each. Call this before authoring a flowchart so you only use shapes/icons that actually render. Takes no arguments.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        ]
    })
}

fn tools_call(params: Option<&serde_json::Value>) -> serde_json::Value {
    let name = params
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let args = params
        .and_then(|p| p.get("arguments"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let source = args.get("source").and_then(|s| s.as_str()).unwrap_or("");

    let outcome = match name {
        "validate_diagram" => validate(source),
        "render_diagram" => {
            let dark = args.get("dark").and_then(|d| d.as_bool()).unwrap_or(false);
            let path = args.get("path").and_then(|p| p.as_str());
            render(source, dark, path)
        }
        "list_shapes" => Ok(list_shapes()),
        other => Err(format!("unknown tool: {other}")),
    };

    match outcome {
        Ok(text) => serde_json::json!({
            "content": [{ "type": "text", "text": text }]
        }),
        Err(message) => serde_json::json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true
        }),
    }
}

/// The full visual vocabulary the engine understands, as a JSON document
/// agents can read once and then author against. Kept in lockstep with the
/// `NodeShape`/`ComponentRole`/`EdgeStyle` enums in `layra-core` and the
/// bracket-syntax table in `layra-parser`.
fn list_shapes() -> String {
    let doc = serde_json::json!({
        "node_shapes": [
            { "shape": "rect",         "syntax": "id[\"Label\"]",      "note": "process / default rectangle" },
            { "shape": "rounded-rect", "syntax": "id(\"Label\")",      "note": "rounded box" },
            { "shape": "stadium",      "syntax": "id([\"Label\"])",    "note": "pill / terminal (start-end)" },
            { "shape": "cylinder",     "syntax": "id[(\"Label\")]",    "note": "database / storage; infers the `database` role" },
            { "shape": "circle",       "syntax": "id((\"Label\"))",    "note": "circle" },
            { "shape": "diamond",      "syntax": "id{\"Label\"}",      "note": "decision" },
            { "shape": "hexagon",      "syntax": "id{{\"Label\"}}",    "note": "queue / pipe; infers the `queue` role" }
        ],
        "role_classes": {
            "syntax": "id[\"Label\"]:::role",
            "note": "Semantic role drives color independent of shape.",
            "roles": [
                "service", "database", "cache", "queue", "gateway",
                "client", "external", "storage", "compute", "highlight"
            ]
        },
        "edge_styles": [
            { "style": "solid",     "syntax": "a --> b",        "meaning": "request / sync flow" },
            { "style": "dashed",    "syntax": "a -.-> b",       "meaning": "async / event" },
            { "style": "thick",     "syntax": "a ==> b",        "meaning": "hot path" },
            { "style": "invisible", "syntax": "a ~~~ b",        "meaning": "layout constraint, renders nothing" },
            { "style": "labeled",   "syntax": "a -->|label| b", "meaning": "any edge can carry a |label|" }
        ],
        "icons": {
            "syntax": "id[\"{icon:pack:name} Label\"]",
            "note": "Iconify-style refs inside a label; stripped from text and drawn on the node. Example packs: mdi, logos, simple-icons.",
            "examples": ["{icon:mdi:database}", "{icon:logos:postgresql}", "{icon:mdi:router-wireless}"]
        },
        "containers": {
            "subgraph": { "syntax": "subgraph id[\"Title\"]\n  a --> b\nend", "note": "groups nodes; nesting supported" }
        },
        "diagram_types": [
            "flowchart", "sequenceDiagram", "stateDiagram-v2", "classDiagram",
            "erDiagram", "gantt", "pie", "mindmap", "timeline", "journey", "gitGraph"
        ]
    });
    // Pretty-print so the agent (and humans reading transcripts) can scan it.
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| doc.to_string())
}

fn validate(source: &str) -> Result<String, String> {
    if source.trim().is_empty() {
        return Err("source is empty".into());
    }
    match layra_wasm::render_svg_lenient(source, false) {
        Ok((_, warnings)) if warnings.is_empty() => {
            Ok("ok: diagram parses and renders cleanly".into())
        }
        Ok((_, warnings)) => Err(format!(
            "{} problem(s) — fix these lines:\n{}",
            warnings.len(),
            warnings.join("\n")
        )),
        Err(e) => Err(format!("does not parse: {e}")),
    }
}

fn render(source: &str, dark: bool, path: Option<&str>) -> Result<String, String> {
    let (svg, warnings) = layra_wasm::render_svg_lenient(source, dark)?;
    let warning_note = if warnings.is_empty() {
        String::new()
    } else {
        format!(
            "\nwarnings ({} skipped lines):\n{}",
            warnings.len(),
            warnings.join("\n")
        )
    };

    match path {
        Some(p) => {
            // Agents routinely target docs/diagrams/x.svg in fresh repos;
            // create intermediate directories instead of failing.
            if let Some(parent) = std::path::Path::new(p).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
                }
            }
            std::fs::write(p, &svg).map_err(|e| format!("cannot write {p}: {e}"))?;
            Ok(format!("wrote {p} ({} bytes){warning_note}", svg.len()))
        }
        None => Ok(format!("{svg}{warning_note}")),
    }
}

fn respond(stdout: &std::io::Stdout, id: serde_json::Value, result: serde_json::Value) {
    let msg = serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result });
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{msg}");
    let _ = lock.flush();
}

fn respond_err(stdout: &std::io::Stdout, id: serde_json::Value, code: i64, message: &str) {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    });
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{msg}");
    let _ = lock.flush();
}
