//! Mindmap parser: Mermaid `mindmap` dialect → graph IR (tree layout).
//!
//! Indentation defines hierarchy. Node shapes follow Mermaid: `root((x))`
//! circle, `a[x]` rect, `b(x)` rounded, `c{{x}}` hexagon, bare = rounded.
//! Mermaid lays mindmaps out radially; Layra renders them as a layered
//! tree (TB), which reads just as well and reuses the whole pipeline.

use crate::ParseError;
use layra_core::{
    ComponentRole, Direction, Edge, EdgeKind, EdgeStyle, Graph, Node, NodeId, NodeShape,
};

pub(crate) fn parse_lenient(lines: &[(usize, &str)], raw: &str) -> (Graph, Vec<ParseError>) {
    let mut graph = Graph::new(Direction::TopBottom);
    let mut warnings = Vec::new();
    // Stack of (indent, node) from root to the current branch tip.
    let mut stack: Vec<(usize, NodeId)> = Vec::new();

    // We need original indentation, so re-scan raw lines but keep the
    // filtered line numbers for error reporting.
    let mut numbered = lines.iter();
    for raw_line in raw.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") || trimmed == "mindmap" {
            continue;
        }
        let Some(&(ln, _)) = numbered.next() else {
            break;
        };

        let indent = raw_line.len() - raw_line.trim_start().len();
        let Some((label, shape)) = parse_node_text(trimmed) else {
            warnings.push(ParseError::Syntax {
                line: ln,
                message: format!("cannot parse mindmap node '{trimmed}'"),
            });
            continue;
        };

        let mut node = Node::new(format!("mm{}", graph.nodes.len()), label);
        node.shape = shape;
        node.role = if stack.is_empty() {
            ComponentRole::Highlight
        } else {
            ComponentRole::Generic
        };
        let id = graph.add_node(node);

        // Pop to the nearest shallower ancestor.
        while stack.last().is_some_and(|&(i, _)| i >= indent) {
            stack.pop();
        }
        if let Some(&(_, parent)) = stack.last() {
            graph.add_edge(Edge {
                source: parent,
                target: id,
                label: None,
                style: EdgeStyle::Solid,
                kind: EdgeKind::Open,
                points: vec![],
                label_pos: None,
                end_labels: None,
            });
        }
        stack.push((indent, id));
    }
    (graph, warnings)
}

/// `root((Label))` / `a[Label]` / `b(Label)` / `c{{Label}}` / bare text.
fn parse_node_text(s: &str) -> Option<(String, NodeShape)> {
    // Optional leading identifier glued to a bracket.
    let bracket_start = s.find(['(', '[', '{']);
    let (head, tail) = match bracket_start {
        Some(i) => (&s[..i], &s[i..]),
        None => return Some((s.to_string(), NodeShape::RoundedRect)),
    };
    let _ = head; // id is irrelevant for rendering

    let strip = |open: &str, close: &str| -> Option<String> {
        tail.strip_prefix(open)?
            .strip_suffix(close)
            .map(|x| x.trim().to_string())
    };
    if let Some(l) = strip("((", "))") {
        return Some((l, NodeShape::Circle));
    }
    if let Some(l) = strip("{{", "}}") {
        return Some((l, NodeShape::Hexagon));
    }
    if let Some(l) = strip("[", "]") {
        return Some((l, NodeShape::Rect));
    }
    if let Some(l) = strip("(", ")") {
        return Some((l, NodeShape::RoundedRect));
    }
    Some((s.to_string(), NodeShape::RoundedRect))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indentation_builds_tree() {
        let raw =
            "mindmap\n  root((Layra))\n    Engine\n      Layout\n      Routing\n    Playground\n";
        let lines: Vec<(usize, &str)> = raw
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim()))
            .filter(|(_, l)| !l.is_empty() && *l != "mindmap")
            .collect();
        let (g, warnings) = parse_lenient(&lines, raw);
        assert!(warnings.is_empty());
        assert_eq!(g.nodes.len(), 5);
        assert_eq!(g.edges.len(), 4);
        assert_eq!(g.nodes[0].shape, NodeShape::Circle);
        assert_eq!(g.nodes[0].label, "Layra");
        // Layout + Routing hang off Engine (node 1).
        assert_eq!(g.edges[1].source.index(), 1);
        assert_eq!(g.edges[2].source.index(), 1);
        // Playground hangs off root.
        assert_eq!(g.edges[3].source.index(), 0);
    }
}
