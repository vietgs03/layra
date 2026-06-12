//! # layra-parser
//!
//! Parses a Mermaid-compatible flowchart dialect into the Layra IR.
//!
//! Supported today (the pragmatic 90% of real-world flowcharts):
//!
//! ```text
//! flowchart LR
//!   api["API Gateway"]:::gateway --> svc["Order Service"]:::service
//!   svc -->|persist| db[("Postgres")]:::database
//!   svc -.->|events| q{{Kafka}}:::queue
//!   subgraph cluster["Data Plane"]
//!     db
//!     q
//!   end
//! ```
//!
//! - Node shapes from bracket syntax: `[rect]`, `(rounded)`, `([stadium])`,
//!   `[(cylinder)]`, `((circle))`, `{diamond}`, `{{hexagon}}`
//! - Edges: `-->`, `---`, `-.->`, `==>`, `<-->`, with `|label|` or `-- label -->`
//! - `:::role` class bindings mapping to the component taxonomy
//! - `subgraph id["Label"] ... end` (nesting supported)
//! - Icon refs inside labels: `{icon:logos:postgresql}` (stripped from text,
//!   stored on the node)

use layra_core::{
    ComponentRole, Direction, Document, Edge, EdgeKind, EdgeStyle, Graph, Node, NodeId, NodeShape,
    Subgraph, SubgraphId,
};
use std::collections::HashMap;
use thiserror::Error;

mod class;
mod er;
mod gantt;
mod git;
mod linear;
mod mindmap;
mod pie;
mod sequence;
mod state;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("line {line}: {message}")]
    Syntax { line: usize, message: String },
}

/// Parse any supported diagram type, dispatching on the header line:
/// `flowchart`/`graph`, `sequenceDiagram`, `stateDiagram`/`stateDiagram-v2`.
pub fn parse_document(source: &str) -> Result<Document, ParseError> {
    let (doc, warnings) = parse_document_lenient(source);
    match warnings.into_iter().next() {
        // Strict mode: surface the first problem as a hard error.
        Some(w) => Err(w),
        None => Ok(doc),
    }
}

/// Lenient parse: unparseable lines are skipped and reported as warnings
/// instead of failing the whole document. Real-world sources are often
/// mangled by copy-paste (joined lines, truncated tails) — rendering the
/// 95% that parses beats a blank screen.
pub fn parse_document_lenient(source: &str) -> (Document, Vec<ParseError>) {
    let lines: Vec<(usize, &str)> = source
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with("%%"))
        .collect();

    let Some(&(_, header)) = lines.first() else {
        return (Document::Graph(Graph::default()), Vec::new());
    };

    if header == "sequenceDiagram" {
        let (seq, warnings) = sequence::parse_lenient(&lines[1..]);
        return (Document::Sequence(seq), warnings);
    }
    if header.starts_with("stateDiagram") {
        let (graph, warnings) = state::parse_lenient(&lines[1..]);
        return (Document::Graph(graph), warnings);
    }
    if header.starts_with("classDiagram") {
        let (graph, warnings) = class::parse_lenient(&lines[1..]);
        return (Document::Graph(graph), warnings);
    }
    if header.starts_with("erDiagram") {
        let (graph, warnings) = er::parse_lenient(&lines[1..]);
        return (Document::Graph(graph), warnings);
    }
    if header == "mindmap" {
        let (graph, warnings) = mindmap::parse_lenient(&lines[1..], source);
        return (Document::Graph(graph), warnings);
    }
    if header == "timeline" {
        let (tl, warnings) = linear::parse_timeline(&lines[1..]);
        return (Document::Timeline(tl), warnings);
    }
    if header == "journey" {
        let (j, warnings) = linear::parse_journey(&lines[1..]);
        return (Document::Journey(j), warnings);
    }
    if header.starts_with("gitGraph") {
        let (g, warnings) = git::parse_lenient(&lines[1..]);
        return (Document::Git(g), warnings);
    }
    if header == "gantt" {
        let (chart, warnings) = gantt::parse_lenient(&lines[1..]);
        return (Document::Gantt(chart), warnings);
    }
    if let Some(rest) = header.strip_prefix("pie") {
        let (chart, warnings) = pie::parse_lenient(rest, &lines[1..]);
        return (Document::Pie(chart), warnings);
    }
    let (graph, warnings) = Parser::new(source).run_lenient();
    (Document::Graph(graph), warnings)
}

/// Parse flowchart source only (legacy entry point; prefer
/// [`parse_document`]).
pub fn parse(source: &str) -> Result<Graph, ParseError> {
    Parser::new(source).run()
}

struct Parser<'a> {
    lines: Vec<(usize, &'a str)>,
    graph: Graph,
    by_name: HashMap<String, NodeId>,
    subgraph_stack: Vec<SubgraphId>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        let lines = source
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim()))
            .filter(|(_, l)| !l.is_empty() && !l.starts_with("%%"))
            .collect();
        Self {
            lines,
            graph: Graph::default(),
            by_name: HashMap::new(),
            subgraph_stack: Vec::new(),
        }
    }

    fn run(mut self) -> Result<Graph, ParseError> {
        let lines = std::mem::take(&mut self.lines);
        let mut iter = lines.into_iter();

        // Optional header line.
        let mut pending: Option<(usize, &str)> = None;
        if let Some((ln, line)) = iter.next() {
            if let Some(rest) = line
                .strip_prefix("flowchart")
                .or_else(|| line.strip_prefix("graph"))
            {
                self.graph.direction = parse_direction(rest.trim());
            } else {
                pending = Some((ln, line));
            }
        }

        for (ln, line) in pending.into_iter().chain(iter) {
            self.statement(ln, line)?;
        }
        Ok(self.graph)
    }

    /// Like [`run`], but collects per-line errors instead of bailing.
    fn run_lenient(mut self) -> (Graph, Vec<ParseError>) {
        let lines = std::mem::take(&mut self.lines);
        let mut iter = lines.into_iter();
        let mut warnings = Vec::new();

        let mut pending: Option<(usize, &str)> = None;
        if let Some((ln, line)) = iter.next() {
            if let Some(rest) = line
                .strip_prefix("flowchart")
                .or_else(|| line.strip_prefix("graph"))
            {
                self.graph.direction = parse_direction(rest.trim());
            } else {
                pending = Some((ln, line));
            }
        }

        for (ln, line) in pending.into_iter().chain(iter) {
            if let Err(e) = self.statement(ln, line) {
                warnings.push(e);
            }
        }
        (self.graph, warnings)
    }

    fn statement(&mut self, ln: usize, line: &str) -> Result<(), ParseError> {
        if let Some(rest) = line.strip_prefix("subgraph") {
            return self.open_subgraph(ln, rest.trim());
        }
        if line == "end" {
            if self.subgraph_stack.pop().is_none() {
                return Err(ParseError::Syntax {
                    line: ln,
                    message: "'end' without matching 'subgraph'".into(),
                });
            }
            return Ok(());
        }
        if line.starts_with("classDef") || line.starts_with("class ") || line.starts_with("style") {
            return Ok(()); // theming handled by Layra's taxonomy; ignore
        }
        if line.starts_with("direction ") {
            // Per-subgraph direction: accepted for compatibility; layout
            // currently applies the global direction (tracked for v1.1).
            return Ok(());
        }
        if line.starts_with("linkStyle") {
            return Ok(()); // per-link styling not yet mapped; ignore
        }
        if is_style_debris(line) {
            // Mangled copy-paste fragments of classDef/style lines (CSS
            // tokens like `stroke-width:1.75px;` glued to garbage). Layra
            // ignores style lines anyway, so their debris is skipped
            // silently rather than warned about.
            return Ok(());
        }
        self.node_or_edge_chain(ln, line)
    }

    fn open_subgraph(&mut self, _ln: usize, rest: &str) -> Result<(), ParseError> {
        // Forms: `subgraph name`, `subgraph name["Label"]`
        let (name, label) = if let Some(idx) = rest.find('[') {
            let name = rest[..idx].trim().to_string();
            let label = rest[idx + 1..]
                .trim_end_matches(']')
                .trim_matches('"')
                .to_string();
            (name, label)
        } else {
            let name = rest.trim().trim_matches('"').to_string();
            (name.clone(), name)
        };

        let parent = self.subgraph_stack.last().copied();
        let id = self.graph.add_subgraph(Subgraph {
            name,
            label,
            nodes: Vec::new(),
            parent,
            rect: Default::default(),
        });
        self.subgraph_stack.push(id);
        Ok(())
    }

    /// Parse `a --> b -.-> c` style chains, including standalone node decls.
    fn node_or_edge_chain(&mut self, ln: usize, line: &str) -> Result<(), ParseError> {
        let mut rest = line;
        // Mermaid `&` groups: `a & b --> c & d` creates the cross product
        // (a->c, a->d, b->c, b->d). Each chain segment is a *group*.
        let mut prev: Option<(Vec<NodeId>, EdgeOp)> = None;

        loop {
            let (group_text, after) = split_node_segment(rest);

            // Split the segment on `&` (outside brackets) into group members.
            let mut group: Vec<NodeId> = Vec::new();
            for part in split_amp(group_text) {
                group.push(self.intern_node(ln, part.trim())?);
            }

            if let Some((sources, op)) = prev.take() {
                // Invisible links (~~~) influence layout but draw nothing:
                // represented as an edge with no kind/markers and a flag the
                // renderer respects via EdgeStyle::Invisible.
                for &src in &sources {
                    for &dst in &group {
                        self.graph.add_edge(Edge {
                            source: src,
                            target: dst,
                            label: op.label.clone(),
                            style: op.style,
                            kind: op.kind,
                            points: vec![],
                            label_pos: None,
                            end_labels: None,
                        });
                    }
                }
            }

            let after = after.trim();
            if after.is_empty() {
                return Ok(());
            }
            let (op, tail) = parse_edge_op(after).ok_or_else(|| ParseError::Syntax {
                line: ln,
                message: format!("expected edge operator near '{after}'"),
            })?;
            prev = Some((group, op));
            rest = tail;
        }
    }

    /// Get-or-create a node from a declaration like `db[("Postgres")]:::database`.
    fn intern_node(&mut self, ln: usize, text: &str) -> Result<NodeId, ParseError> {
        let decl = parse_node_decl(text).ok_or_else(|| ParseError::Syntax {
            line: ln,
            message: format!("cannot parse node '{text}'"),
        })?;

        if let Some(&id) = self.by_name.get(&decl.name) {
            // Enrich an earlier bare reference with label/shape/role info.
            let node = self.graph.node_mut(id);
            if let Some(label) = decl.label {
                node.label = label;
            }
            if let Some(shape) = decl.shape {
                node.shape = shape;
            }
            if let Some(role) = decl.role {
                node.role = role;
            }
            if decl.icon.is_some() {
                node.icon = decl.icon;
            }
            self.claim_for_subgraph(id);
            return Ok(id);
        }

        let mut node = Node::new(decl.name.clone(), decl.label.unwrap_or(decl.name.clone()));
        node.shape = decl.shape.unwrap_or_default();
        node.role = decl.role.unwrap_or_else(|| infer_role(node.shape));
        node.icon = decl.icon;
        node.parent = self.subgraph_stack.last().copied();
        let id = self.graph.add_node(node);
        self.by_name.insert(decl.name, id);
        self.claim_for_subgraph(id);
        Ok(id)
    }

    fn claim_for_subgraph(&mut self, id: NodeId) {
        if let Some(&sg) = self.subgraph_stack.last() {
            let list = &mut self.graph.subgraphs[sg.index()].nodes;
            if !list.contains(&id) {
                list.push(id);
            }
        }
    }
}

/// Heuristic: does this unparseable line look like the debris of a styling
/// statement (classDef/style) mangled by copy-paste? CSS-ish tokens are a
/// strong signal: `stroke`, `fill:`, `px;`, `color:`, `dasharray`.
fn is_style_debris(line: &str) -> bool {
    const SIGNALS: &[&str] = &[
        "stroke",
        "fill:",
        "px;",
        "px,",
        "px ",
        "color:",
        "dasharray",
        "classdef",
        ":1.",
        ":2.",
    ];
    let lower = line.to_ascii_lowercase();
    let hits: usize = SIGNALS.iter().map(|s| lower.matches(s).count()).sum();
    hits >= 2
}

fn parse_direction(s: &str) -> Direction {
    match s {
        "LR" => Direction::LeftRight,
        "RL" => Direction::RightLeft,
        "BT" => Direction::BottomTop,
        _ => Direction::TopBottom,
    }
}

/// Default roles for shapes with strong conventions.
fn infer_role(shape: NodeShape) -> ComponentRole {
    match shape {
        NodeShape::Cylinder => ComponentRole::Database,
        NodeShape::Hexagon => ComponentRole::Queue,
        _ => ComponentRole::Generic,
    }
}

struct EdgeOp {
    style: EdgeStyle,
    kind: EdgeKind,
    label: Option<String>,
}

/// Split a chain segment on `&` outside any bracket/quote nesting:
/// `a & b["x & y"]` → ["a", "b[\"x & y\"]"].
fn split_amp(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_quote = false;
    let mut start = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        match b as char {
            '"' => in_quote = !in_quote,
            '[' | '(' | '{' if !in_quote => depth += 1,
            ']' | ')' | '}' if !in_quote => depth -= 1,
            '&' if depth == 0 && !in_quote => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Split off the leading node segment of a chain, stopping before an edge
/// operator. Respects bracket nesting so `a["x --> y"]` parses correctly.
fn split_node_segment(s: &str) -> (&str, &str) {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            '"' => {
                // skip quoted span
                i += 1;
                while i < bytes.len() && bytes[i] as char != '"' {
                    i += 1;
                }
            }
            '-' | '=' | '<' | '~' if depth == 0 => {
                // Potential edge operator start. Require at least `--`/`==`/`<-`/`~~~`.
                let rest = &s[i..];
                if rest.starts_with("--")
                    || rest.starts_with("-.")
                    || rest.starts_with("==")
                    || rest.starts_with("<--")
                    || rest.starts_with("<==")
                    || rest.starts_with("~~~")
                {
                    return (&s[..i], rest);
                }
            }
            _ => {}
        }
        i += 1;
    }
    (s, "")
}

/// Parse an edge operator (with optional label) at the start of `s`.
/// Returns the op and the remaining tail (the next node segment).
fn parse_edge_op(s: &str) -> Option<(EdgeOp, &str)> {
    // `-- label -->` inline-label form
    // `-->|label|` pipe-label form
    // styles: --> solid, -.-> dashed, ==> thick, --- open line, <--> bidi
    let (kind_prefix, s) = if let Some(rest) = s.strip_prefix('<') {
        (true, rest)
    } else {
        (false, s)
    };

    if let Some(rest) = s.strip_prefix("~~~") {
        // Invisible link: constrains layout, renders nothing.
        return Some((
            EdgeOp {
                style: EdgeStyle::Invisible,
                kind: EdgeKind::Open,
                label: None,
            },
            rest,
        ));
    }
    let (style, after) = if let Some(rest) = s.strip_prefix("-.") {
        // -.-> or -.text.->  (we only support -.->)
        let rest = rest.strip_prefix('-')?;
        (EdgeStyle::Dashed, rest)
    } else if let Some(rest) = s.strip_prefix("==") {
        (EdgeStyle::Thick, rest)
    } else if let Some(rest) = s.strip_prefix("--") {
        (EdgeStyle::Solid, rest)
    } else {
        return None;
    };

    // Inline label: `-- label -->`
    if style == EdgeStyle::Solid && !after.starts_with('>') && !after.starts_with('-') {
        if let Some(end) = after.find("-->") {
            let label = after[..end].trim();
            let tail = &after[end + 3..];
            return Some((
                EdgeOp {
                    style,
                    kind: if kind_prefix {
                        EdgeKind::Bidirectional
                    } else {
                        EdgeKind::Arrow
                    },
                    label: (!label.is_empty()).then(|| clean_edge_label(label)),
                },
                tail,
            ));
        }
    }

    let (kind, after) = if let Some(rest) = after.strip_prefix('>') {
        (
            if kind_prefix {
                EdgeKind::Bidirectional
            } else {
                EdgeKind::Arrow
            },
            rest,
        )
    } else if let Some(rest) = after.strip_prefix('-') {
        (EdgeKind::Open, rest)
    } else if let Some(rest) = after.strip_prefix('=') {
        (EdgeKind::Open, rest)
    } else {
        (EdgeKind::Arrow, after)
    };

    // Pipe label: `|label|`
    let (label, tail) = if let Some(rest) = after.trim_start().strip_prefix('|') {
        let end = rest.find('|')?;
        (Some(clean_edge_label(&rest[..end])), &rest[end + 1..])
    } else {
        (None, after)
    };

    Some((EdgeOp { style, kind, label }, tail))
}

struct NodeDecl {
    name: String,
    label: Option<String>,
    shape: Option<NodeShape>,
    role: Option<ComponentRole>,
    icon: Option<String>,
}

/// Parse `name<bracket label>:::role`.
fn parse_node_decl(text: &str) -> Option<NodeDecl> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // Split off `:::role`
    let (body, role) = match text.split_once(":::") {
        Some((b, r)) => (b.trim(), parse_role(r.trim())),
        None => (text, None),
    };

    // Find bracket start.
    let bracket_at = body.find(['[', '(', '{']);
    let Some(idx) = bracket_at else {
        // Bare reference.
        let name = body.trim();
        if name.is_empty() || !is_ident(name) {
            return None;
        }
        return Some(NodeDecl {
            name: name.to_string(),
            label: None,
            shape: None,
            role,
            icon: None,
        });
    };

    let name = body[..idx].trim();
    if name.is_empty() || !is_ident(name) {
        return None;
    }
    let bracket = body[idx..].trim();

    let (shape, raw_label) = parse_bracket(bracket)?;
    let (label, icon) = extract_icon(raw_label);
    let (label, icon_from_html) = sanitize_html_label(label);
    let label = unescape_newlines(label);

    Some(NodeDecl {
        name: name.to_string(),
        label: Some(label),
        shape: Some(shape),
        role,
        icon: icon.or(icon_from_html),
    })
}

fn is_ident(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// Map bracket syntax to shape, returning the inner label text.
fn parse_bracket(s: &str) -> Option<(NodeShape, String)> {
    let strip = |s: &str, open: &str, close: &str| -> Option<String> {
        s.strip_prefix(open)?
            .strip_suffix(close)
            .map(|inner| inner.trim().trim_matches('"').to_string())
    };

    // Order matters: longest delimiters first.
    if let Some(l) = strip(s, "([", "])") {
        return Some((NodeShape::Stadium, l));
    }
    if let Some(l) = strip(s, "[(", ")]") {
        return Some((NodeShape::Cylinder, l));
    }
    if let Some(l) = strip(s, "((", "))") {
        return Some((NodeShape::Circle, l));
    }
    if let Some(l) = strip(s, "{{", "}}") {
        return Some((NodeShape::Hexagon, l));
    }
    if let Some(l) = strip(s, "[", "]") {
        return Some((NodeShape::Rect, l));
    }
    if let Some(l) = strip(s, "(", ")") {
        return Some((NodeShape::RoundedRect, l));
    }
    if let Some(l) = strip(s, "{", "}") {
        return Some((NodeShape::Diamond, l));
    }
    None
}

fn parse_role(s: &str) -> Option<ComponentRole> {
    Some(match s {
        "service" => ComponentRole::Service,
        "database" | "db" => ComponentRole::Database,
        "cache" => ComponentRole::Cache,
        "queue" => ComponentRole::Queue,
        "gateway" => ComponentRole::Gateway,
        "client" => ComponentRole::Client,
        "external" => ComponentRole::External,
        "storage" => ComponentRole::Storage,
        "compute" => ComponentRole::Compute,
        "highlight" => ComponentRole::Highlight,
        _ => ComponentRole::Generic,
    })
}

/// Edge labels: strip quotes and reduce embedded HTML (`<br/>` → newline,
/// other tags dropped).
pub(crate) fn clean_edge_label(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"');
    let (text, _) = sanitize_html_label(trimmed.to_string());
    text
}

/// Mermaid-compat: blog diagrams embed HTML inside labels —
/// `<img src="/icons/mdi-laptop.svg" ...>`, `<br/>`, `<span class='sub'>…</span>`.
/// Layra labels are plain text + an icon slot, so:
/// - `<img src=".../icons/{pack}-{name}.svg">` → icon `pack:name`
/// - `<br>`/`<br/>` → newline
/// - any other tag → stripped, inner text kept
pub(crate) fn sanitize_html_label(label: String) -> (String, Option<String>) {
    if !label.contains('<') {
        return (label, None);
    }

    let mut icon = None;
    let mut text = String::with_capacity(label.len());
    let mut rest = label.as_str();

    while let Some(lt) = rest.find('<') {
        text.push_str(&rest[..lt]);
        let Some(gt_rel) = rest[lt..].find('>') else {
            text.push_str(&rest[lt..]);
            rest = "";
            break;
        };
        let tag = &rest[lt + 1..lt + gt_rel];
        let tag_name = tag
            .trim_start_matches('/')
            .split([' ', '/', '\t'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();

        match tag_name.as_str() {
            "br" => text.push('\n'),
            "img" if icon.is_none() => icon = img_src_to_icon(tag),
            _ => {} // drop the tag, keep surrounding text
        }
        rest = &rest[lt + gt_rel + 1..];
    }
    text.push_str(rest);

    // Collapse leftover whitespace per line.
    let cleaned = text
        .split('\n')
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    (cleaned, icon)
}

/// Convert literal `\n` escapes in labels to real newlines (Mermaid allows
/// both `<br/>` and `\n` in quoted labels).
fn unescape_newlines(label: String) -> String {
    if label.contains("\\n") {
        label.replace("\\n", "\n")
    } else {
        label
    }
}

/// `src=".../icons/mdi-laptop.svg"` → `Some("mdi:laptop")`.
fn img_src_to_icon(tag: &str) -> Option<String> {
    let src_at = tag.find("src=")?;
    let quote = tag.as_bytes().get(src_at + 4).copied()? as char;
    let after = &tag[src_at + 5..];
    let end = after.find(quote)?;
    let path = &after[..end];

    let file = path.rsplit('/').next()?.strip_suffix(".svg")?;
    let (pack, name) = file.split_once('-')?;
    Some(format!("{pack}:{name}"))
}

/// Pull `{icon:pack:name}` out of a label.
fn extract_icon(label: String) -> (String, Option<String>) {
    let Some(start) = label.find("{icon:") else {
        return (label, None);
    };
    let Some(end_rel) = label[start..].find('}') else {
        return (label, None);
    };
    let end = start + end_rel;
    let icon = label[start + 6..end].trim().to_string();
    let mut text = String::with_capacity(label.len());
    text.push_str(label[..start].trim_end());
    let after = label[end + 1..].trim_start();
    if !text.is_empty() && !after.is_empty() {
        text.push(' ');
    }
    text.push_str(after);
    (text.trim().to_string(), Some(icon))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_flowchart() {
        let g = parse(
            r#"flowchart LR
              api["API Gateway"]:::gateway --> svc["Order Service"]:::service
              svc -->|persist| db[("Postgres")]:::database
              svc -.->|events| q{{Kafka}}
            "#,
        )
        .unwrap();

        assert_eq!(g.direction, Direction::LeftRight);
        assert_eq!(g.nodes.len(), 4);
        assert_eq!(g.edges.len(), 3);

        let db = g.node(g.node_by_name("db").unwrap());
        assert_eq!(db.shape, NodeShape::Cylinder);
        assert_eq!(db.role, ComponentRole::Database);
        assert_eq!(db.label, "Postgres");

        let q = g.node(g.node_by_name("q").unwrap());
        assert_eq!(q.shape, NodeShape::Hexagon);
        assert_eq!(q.role, ComponentRole::Queue); // inferred from shape

        assert_eq!(g.edges[1].label.as_deref(), Some("persist"));
        assert_eq!(g.edges[2].style, EdgeStyle::Dashed);
    }

    #[test]
    fn parses_subgraphs() {
        let g = parse(
            r#"flowchart TB
              subgraph data["Data Plane"]
                db[("PG")]
                cache(("Redis"))
              end
              api --> db
            "#,
        )
        .unwrap();
        assert_eq!(g.subgraphs.len(), 1);
        assert_eq!(g.subgraphs[0].label, "Data Plane");
        assert_eq!(g.subgraphs[0].nodes.len(), 2);
    }

    #[test]
    fn chain_of_three() {
        let g = parse("flowchart LR\na --> b --> c").unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 2);
    }

    #[test]
    fn icon_extraction() {
        let g = parse(
            r#"flowchart LR
          db[("{icon:logos:postgresql} Postgres")]"#,
        )
        .unwrap();
        let db = g.node(g.node_by_name("db").unwrap());
        assert_eq!(db.icon.as_deref(), Some("logos:postgresql"));
        assert_eq!(db.label, "Postgres");
    }

    #[test]
    fn inline_label_form() {
        let g = parse("flowchart LR\na -- calls --> b").unwrap();
        assert_eq!(g.edges[0].label.as_deref(), Some("calls"));
    }

    #[test]
    fn error_has_line_number() {
        let err = parse("flowchart LR\nsubgraph x\nend\nend").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 4"), "got: {msg}");
    }
}
