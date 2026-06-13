//! Class diagram parser: Mermaid `classDiagram` dialect → graph IR.
//!
//! Classes become compartmented nodes (label + fields section + methods
//! section); relationships map to edges with UML markers:
//!
//! - `A <|-- B` inheritance (hollow triangle at A)
//! - `A *-- B` composition (filled diamond at A)
//! - `A o-- B` aggregation (hollow diamond at A)
//! - `A --> B` association, `A ..> B` dependency (dashed)
//! - `A -- B` plain link, `A ..|> B` realization
//! - labels: `A --> B : uses`, cardinalities: `A "1" --> "many" B`
//!
//! Members come from either block form (`class A { +int x; +run() }`) or
//! colon form (`A : +int x`). A trailing `()` classifies methods.

use crate::ParseError;
use layra_core::{
    ComponentRole, Direction, Edge, EdgeKind, EdgeStyle, Graph, Node, NodeId, NodeShape,
};
use std::collections::HashMap;

pub(crate) fn parse_lenient(lines: &[(usize, &str)]) -> (Graph, Vec<ParseError>) {
    let mut p = ClassParser {
        graph: Graph::new(Direction::TopBottom),
        by_name: HashMap::new(),
        members: HashMap::new(),
        open_class: None,
    };
    let mut warnings = Vec::new();

    for &(ln, line) in lines {
        if let Err(e) = p.line(ln, line) {
            warnings.push(e);
        }
    }
    p.flush_members();
    (p.graph, warnings)
}

struct ClassParser {
    graph: Graph,
    by_name: HashMap<String, NodeId>,
    /// name -> (fields, methods); folded into node.sections at the end.
    members: HashMap<String, (Vec<String>, Vec<String>)>,
    /// Currently open `class X {` block.
    open_class: Option<String>,
}

impl ClassParser {
    fn line(&mut self, ln: usize, line: &str) -> Result<(), ParseError> {
        if line.starts_with("direction ") {
            self.graph.direction = match line.trim_start_matches("direction ").trim() {
                "LR" => Direction::LeftRight,
                "RL" => Direction::RightLeft,
                "BT" => Direction::BottomTop,
                _ => Direction::TopBottom,
            };
            return Ok(());
        }

        // Inside a `class X { ... }` block: every line is a member.
        if let Some(name) = self.open_class.clone() {
            if line == "}" {
                self.open_class = None;
            } else {
                self.add_member(&name, line.trim_end_matches(';').trim());
            }
            return Ok(());
        }

        if let Some(rest) = line.strip_prefix("class ") {
            let rest = rest.trim();
            if let Some(name) = rest.strip_suffix('{') {
                let name = name.trim();
                self.intern(name);
                self.open_class = Some(name.to_string());
            } else if let Some((name, annot)) = rest.split_once(':') {
                // `class A : +field` shorthand never standard; treat as member.
                let name = name.trim().to_string();
                self.intern(&name);
                self.add_member(&name, annot.trim());
            } else {
                self.intern(rest);
            }
            return Ok(());
        }

        // `A : +int field` member form.
        if let Some((lhs, rhs)) = line.split_once(" : ") {
            if !lhs.contains("--") && !lhs.contains("..") && is_class_ident(lhs.trim()) {
                let name = lhs.trim().to_string();
                self.intern(&name);
                self.add_member(&name, rhs.trim());
                return Ok(());
            }
        }

        // Relationship line.
        if let Some(()) = self.try_relationship(line) {
            return Ok(());
        }

        // Annotations we accept silently.
        if line.starts_with("<<") || line.starts_with("note ") || line.starts_with("%%") {
            return Ok(());
        }

        Err(ParseError::Syntax {
            line: ln,
            message: format!("cannot parse class statement '{line}'"),
        })
    }

    /// `A "1" <|-- "many" B : label`
    fn try_relationship(&mut self, line: &str) -> Option<()> {
        // Operators, longest first. (kind, style, reverse) — reverse=true
        // means the marker belongs at the LHS (Mermaid points base at LHS).
        const OPS: &[(&str, EdgeKind, EdgeStyle, bool)] = &[
            ("<|--", EdgeKind::Triangle, EdgeStyle::Solid, true),
            ("--|>", EdgeKind::Triangle, EdgeStyle::Solid, false),
            ("<|..", EdgeKind::Triangle, EdgeStyle::Dashed, true),
            ("..|>", EdgeKind::Triangle, EdgeStyle::Dashed, false),
            ("*--", EdgeKind::DiamondFilled, EdgeStyle::Solid, false),
            ("--*", EdgeKind::DiamondFilled, EdgeStyle::Solid, true),
            ("o--", EdgeKind::DiamondOpen, EdgeStyle::Solid, false),
            ("--o", EdgeKind::DiamondOpen, EdgeStyle::Solid, true),
            ("-->", EdgeKind::Arrow, EdgeStyle::Solid, false),
            ("<--", EdgeKind::Arrow, EdgeStyle::Solid, true),
            ("..>", EdgeKind::Arrow, EdgeStyle::Dashed, false),
            ("<..", EdgeKind::Arrow, EdgeStyle::Dashed, true),
            ("--", EdgeKind::Open, EdgeStyle::Solid, false),
            ("..", EdgeKind::Open, EdgeStyle::Dashed, false),
        ];

        let (pos, op, kind, style, reverse) = OPS
            .iter()
            .filter_map(|&(op, k, s, rev)| line.find(op).map(|p| (p, op, k, s, rev)))
            .min_by_key(|&(p, op, ..)| (p, std::cmp::Reverse(op.len())))?;

        let (lhs_raw, rest) = (line[..pos].trim(), line[pos + op.len()..].trim());
        // Optional trailing `: label`.
        let (rhs_raw, label) = match rest.split_once(':') {
            Some((r, l)) => (r.trim(), Some(l.trim().to_string())),
            None => (rest, None),
        };

        let (lhs, lcard) = strip_cardinality(lhs_raw);
        let (rhs, rcard) = strip_cardinality(rhs_raw);
        if lhs.is_empty() || rhs.is_empty() || !is_class_ident(&lhs) || !is_class_ident(&rhs) {
            return None;
        }

        let a = self.intern(&lhs);
        let b = self.intern(&rhs);
        // Marker semantics: triangle/diamond sit at the *base* class side.
        let (source, target, end_labels) = if reverse {
            (b, a, rcard.zip_or(lcard))
        } else {
            (a, b, lcard.zip_or(rcard))
        };

        self.graph.add_edge(Edge {
            source,
            target,
            label,
            style,
            kind,
            points: vec![],
            label_pos: None,
            end_labels,
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
        node.role = ComponentRole::Service;
        let id = self.graph.add_node(node);
        self.by_name.insert(name.to_string(), id);
        id
    }

    fn add_member(&mut self, class: &str, member: &str) {
        if member.is_empty() || member.starts_with("<<") {
            return;
        }
        let entry = self.members.entry(class.to_string()).or_default();
        if member.contains('(') {
            entry.1.push(member.to_string());
        } else {
            entry.0.push(member.to_string());
        }
    }

    /// Fold collected members into `node.sections`: [fields, methods].
    fn flush_members(&mut self) {
        for (name, (fields, methods)) in &self.members {
            if let Some(&id) = self.by_name.get(name) {
                let node = self.graph.node_mut(id);
                node.sections.clear();
                if !fields.is_empty() {
                    node.sections.push(fields.join("\n"));
                }
                if !methods.is_empty() {
                    node.sections.push(methods.join("\n"));
                }
            }
        }
    }
}

trait ZipOr {
    fn zip_or(self, other: Self) -> Option<(String, String)>;
}
impl ZipOr for Option<String> {
    /// Combine endpoint cardinalities; missing side becomes "".
    fn zip_or(self, other: Self) -> Option<(String, String)> {
        match (self, other) {
            (None, None) => None,
            (a, b) => Some((a.unwrap_or_default(), b.unwrap_or_default())),
        }
    }
}

/// `"1" Customer` / `Customer "many"` → (Customer, Some(card)).
fn strip_cardinality(s: &str) -> (String, Option<String>) {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('"') {
        if let Some((card, name)) = rest.split_once('"') {
            return (name.trim().to_string(), Some(card.to_string()));
        }
    }
    if let Some((name, rest)) = s.split_once('"') {
        let card = rest.trim_end_matches('"');
        return (name.trim().to_string(), Some(card.to_string()));
    }
    (s.to_string(), None)
}

fn is_class_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '~' || c == '<' || c == '>')
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
    fn parses_inheritance_and_members() {
        let g = parse_src(
            "class Animal {\n\
               +String name\n\
               +eat()\n\
             }\n\
             class Dog\n\
             Animal <|-- Dog\n\
             Dog : +bark()",
        );
        assert_eq!(g.nodes.len(), 2);
        let animal = g.node(g.node_by_name("Animal").unwrap());
        assert_eq!(animal.sections.len(), 2); // fields + methods
        assert_eq!(animal.sections[0], "+String name");
        assert_eq!(animal.sections[1], "+eat()");

        // Inheritance: triangle at Animal (the base) => edge Dog -> Animal.
        let e = &g.edges[0];
        assert_eq!(e.kind, EdgeKind::Triangle);
        assert_eq!(g.node(e.target).name, "Animal");
    }

    #[test]
    fn parses_cardinality_and_label() {
        let g = parse_src("Customer \"1\" --> \"*\" Order : places");
        let e = &g.edges[0];
        assert_eq!(e.label.as_deref(), Some("places"));
        assert_eq!(e.end_labels, Some(("1".to_string(), "*".to_string())));
    }

    #[test]
    fn composition_and_aggregation_markers() {
        let g = parse_src("Engine *-- Car\nWheel o-- Car");
        assert_eq!(g.edges[0].kind, EdgeKind::DiamondFilled);
        assert_eq!(g.edges[1].kind, EdgeKind::DiamondOpen);
    }
}
