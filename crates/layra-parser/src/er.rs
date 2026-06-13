//! ER diagram parser: Mermaid `erDiagram` dialect → graph IR.
//!
//! Entities are compartmented nodes (attributes in one section);
//! relationships carry crow's-foot cardinality as edge end labels:
//!
//! ```text
//! CUSTOMER ||--o{ ORDER : places
//! ORDER {
//!     int id PK
//!     string status
//! }
//! ```
//!
//! Cardinality tokens (each side): `||` exactly one, `|o` zero-or-one,
//! `}|` one-or-more, `}o` zero-or-more (and mirrored forms). They render
//! as compact end labels (`1`, `0..1`, `1..*`, `0..*`) — readable without
//! dedicated crow's-foot marker art, which can come later.

use crate::ParseError;
use layra_core::{
    ComponentRole, Direction, Edge, EdgeKind, EdgeStyle, Graph, Node, NodeId, NodeShape,
};
use std::collections::HashMap;

pub(crate) fn parse_lenient(lines: &[(usize, &str)]) -> (Graph, Vec<ParseError>) {
    let mut p = ErParser {
        graph: Graph::new(Direction::TopBottom),
        by_name: HashMap::new(),
        open_entity: None,
    };
    let mut warnings = Vec::new();
    for &(ln, line) in lines {
        if let Err(e) = p.line(ln, line) {
            warnings.push(e);
        }
    }
    (p.graph, warnings)
}

struct ErParser {
    graph: Graph,
    by_name: HashMap<String, NodeId>,
    open_entity: Option<NodeId>,
}

impl ErParser {
    fn line(&mut self, ln: usize, line: &str) -> Result<(), ParseError> {
        // Attribute block body.
        if let Some(id) = self.open_entity {
            if line == "}" {
                self.open_entity = None;
            } else {
                let node = self.graph.node_mut(id);
                if node.sections.is_empty() {
                    node.sections.push(String::new());
                }
                let section = &mut node.sections[0];
                if !section.is_empty() {
                    section.push('\n');
                }
                section.push_str(&normalize_attribute(line));
            }
            return Ok(());
        }

        // `ENTITY {` opens an attribute block.
        if let Some(name) = line.strip_suffix('{') {
            let name = name.trim();
            if is_entity_ident(name) {
                let id = self.intern(name);
                self.open_entity = Some(id);
                return Ok(());
            }
        }

        // Relationship: `A <card>--<card> B : label` (or `..` for dashed).
        if let Some(()) = self.try_relationship(line) {
            return Ok(());
        }

        // Bare entity declaration.
        if is_entity_ident(line) {
            self.intern(line);
            return Ok(());
        }

        Err(ParseError::Syntax {
            line: ln,
            message: format!("cannot parse ER statement '{line}'"),
        })
    }

    fn try_relationship(&mut self, line: &str) -> Option<()> {
        // Body is `--` (identifying, solid) or `..` (non-identifying, dashed).
        let (body_pos, style) = match (line.find("--"), line.find("..")) {
            (Some(p), None) => (p, EdgeStyle::Solid),
            (None, Some(p)) => (p, EdgeStyle::Dashed),
            (Some(a), Some(b)) if a < b => (a, EdgeStyle::Solid),
            (Some(_), Some(b)) => (b, EdgeStyle::Dashed),
            (None, None) => return None,
        };

        // Left cardinality: 2 chars before body; right: 2 chars after.
        let left_part = line[..body_pos].trim_end();
        let after_body = &line[body_pos + 2..];
        if left_part.len() < 2 || after_body.len() < 2 {
            return None;
        }
        let (lhs_name, lcard_tok) = left_part.split_at(left_part.len().saturating_sub(2));
        let (rcard_tok, rest) = after_body.split_at(2.min(after_body.len()));

        let lcard = cardinality_label(lcard_tok, true)?;
        let rcard = cardinality_label(rcard_tok, false)?;

        let (rhs_name, label) = match rest.split_once(':') {
            Some((r, l)) => (r.trim(), Some(l.trim().trim_matches('"').to_string())),
            None => (rest.trim(), None),
        };
        let lhs_name = lhs_name.trim();
        if !is_entity_ident(lhs_name) || !is_entity_ident(rhs_name) {
            return None;
        }

        let a = self.intern(lhs_name);
        let b = self.intern(rhs_name);
        self.graph.add_edge(Edge {
            source: a,
            target: b,
            label,
            style,
            kind: EdgeKind::Open,
            points: vec![],
            label_pos: None,
            end_labels: Some((lcard, rcard)),
            animated: false,
        });
        Some(())
    }

    fn intern(&mut self, name: &str) -> NodeId {
        if let Some(&id) = self.by_name.get(name) {
            return id;
        }
        let mut node = Node::new(name, name);
        node.shape = NodeShape::Rect;
        node.role = ComponentRole::Database;
        let id = self.graph.add_node(node);
        self.by_name.insert(name.to_string(), id);
        id
    }
}

/// Map a crow's-foot token to a compact cardinality label.
/// `left` controls reading direction (`}o--` mirrors `--o{`); braces are
/// folded so both `{` and `}` mean "many".
fn cardinality_label(tok: &str, left: bool) -> Option<String> {
    let oriented: String = if left {
        tok.chars().rev().collect()
    } else {
        tok.to_string()
    };
    let normalized: String = oriented
        .chars()
        .map(|c| if c == '}' { '{' } else { c })
        .collect();
    Some(
        match normalized.as_str() {
            "||" => "1",
            "o|" | "|o" => "0..1",
            "|{" | "{|" => "1..*",
            "o{" | "{o" => "0..*",
            _ => return None,
        }
        .to_string(),
    )
}

/// `int id PK` → `id: int  [PK]` — type-second display, keys highlighted.
fn normalize_attribute(line: &str) -> String {
    let mut parts = line.split_whitespace();
    let (Some(ty), Some(name)) = (parts.next(), parts.next()) else {
        return line.to_string();
    };
    let keys: Vec<&str> = parts.collect();
    let mut out = format!("{name}: {ty}");
    if !keys.is_empty() {
        out.push_str(&format!("  [{}]", keys.join(",")));
    }
    out
}

fn is_entity_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_src(src: &str) -> Graph {
        let owned: Vec<(usize, String)> = src
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim().to_string()))
            .filter(|(_, l)| !l.is_empty())
            .collect();
        let borrowed: Vec<(usize, &str)> = owned.iter().map(|(n, l)| (*n, l.as_str())).collect();
        let (g, warnings) = parse_lenient(&borrowed);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        g
    }

    #[test]
    fn parses_relationship_with_cardinality() {
        let g = parse_src("CUSTOMER ||--o{ ORDER : places");
        assert_eq!(g.nodes.len(), 2);
        let e = &g.edges[0];
        assert_eq!(e.label.as_deref(), Some("places"));
        assert_eq!(e.end_labels, Some(("1".into(), "0..*".into())));
    }

    #[test]
    fn parses_attribute_block() {
        let g = parse_src(
            "ORDER {\n\
               int id PK\n\
               string status\n\
             }\n\
             CUSTOMER ||--o{ ORDER : places",
        );
        let order = g.node(g.node_by_name("ORDER").unwrap());
        assert_eq!(order.sections.len(), 1);
        assert!(order.sections[0].contains("id: int  [PK]"));
        assert!(order.sections[0].contains("status: string"));
    }

    #[test]
    fn non_identifying_is_dashed() {
        let g = parse_src("A ||..o{ B : has");
        assert_eq!(g.edges[0].style, EdgeStyle::Dashed);
    }
}
