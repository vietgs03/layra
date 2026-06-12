//! # layra-render-svg
//!
//! Renders a laid-out [`Graph`] to a standalone SVG string.
//!
//! Style follows the diagram-toolkit editorial signature: flat white nodes,
//! colored role borders (the component taxonomy), dashed cluster pills with
//! colored title pills, thin neutral edges.

mod shapes;
mod theme;

use layra_core::{EdgeKind, EdgeStyle, Graph};
use std::fmt::Write;

pub use theme::Theme;

const FONT_STACK: &str = "Inter, 'Helvetica Neue', Arial, sans-serif";

/// Render `graph` to SVG. The graph must already be measured, laid out, and
/// routed.
pub fn render(graph: &Graph, theme: &Theme) -> String {
    let bounds = graph.bounds().inflate(16.0);
    let w = bounds.width.ceil();
    let h = bounds.height.ceil();

    let mut svg = String::with_capacity(4096);
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{:.0} {:.0} {w:.0} {h:.0}" width="{w:.0}" height="{h:.0}" font-family="{FONT_STACK}">"#,
        bounds.x, bounds.y
    );

    write_defs(&mut svg, theme);

    let _ = write!(
        svg,
        r#"<rect x="{:.0}" y="{:.0}" width="{w:.0}" height="{h:.0}" fill="{}"/>"#,
        bounds.x, bounds.y, theme.background
    );

    // Paint order: clusters under nodes under edges-labels.
    for sg in &graph.subgraphs {
        write_subgraph(&mut svg, sg, theme);
    }
    for edge in &graph.edges {
        write_edge(&mut svg, edge, theme);
    }
    for node in &graph.nodes {
        write_node(&mut svg, node, theme);
    }

    svg.push_str("</svg>");
    svg
}

fn write_defs(svg: &mut String, theme: &Theme) {
    let _ = write!(
        svg,
        r#"<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="{}"/></marker></defs>"#,
        theme.edge
    );
}

fn write_subgraph(svg: &mut String, sg: &layra_core::Subgraph, theme: &Theme) {
    let r = sg.rect;
    let _ = write!(
        svg,
        r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="12" fill="{}" stroke="{}" stroke-width="1.2" stroke-dasharray="6 4"/>"#,
        r.x, r.y, r.width, r.height, theme.cluster_fill, theme.cluster_stroke
    );
    // Title pill.
    let label_w = sg.label.len() as f32 * 7.5 + 20.0;
    let _ = write!(
        svg,
        r##"<rect x="{:.1}" y="{:.1}" width="{label_w:.1}" height="22" rx="11" fill="{}"/><text x="{:.1}" y="{:.1}" font-size="12" font-weight="600" fill="#fff" text-anchor="middle" dominant-baseline="central">{}</text>"##,
        r.x + 12.0,
        r.y - 11.0,
        theme.cluster_title,
        r.x + 12.0 + label_w / 2.0,
        r.y,
        escape(&sg.label)
    );
}

fn write_edge(svg: &mut String, edge: &layra_core::Edge, theme: &Theme) {
    if edge.points.len() < 2 {
        return;
    }

    let mut d = String::new();
    let _ = write!(d, "M{:.1} {:.1}", edge.points[0].x, edge.points[0].y);
    if edge.points.len() == 2 {
        let _ = write!(d, " L{:.1} {:.1}", edge.points[1].x, edge.points[1].y);
    } else {
        // Smooth through waypoints with quadratic joins.
        for i in 1..edge.points.len() - 1 {
            let p = edge.points[i];
            let next = edge.points[i + 1];
            let mx = (p.x + next.x) / 2.0;
            let my = (p.y + next.y) / 2.0;
            let _ = write!(d, " Q{:.1} {:.1} {:.1} {:.1}", p.x, p.y, mx, my);
        }
        let last = edge.points[edge.points.len() - 1];
        let _ = write!(d, " L{:.1} {:.1}", last.x, last.y);
    }

    let (width, dash) = match edge.style {
        EdgeStyle::Solid => (1.6, ""),
        EdgeStyle::Thick => (3.0, ""),
        EdgeStyle::Dashed => (1.6, r#" stroke-dasharray="7 5""#),
        EdgeStyle::Dotted => (1.6, r#" stroke-dasharray="2 4""#),
    };
    let markers = match edge.kind {
        EdgeKind::Arrow => r#" marker-end="url(#arrow)""#.to_string(),
        EdgeKind::Bidirectional => {
            r#" marker-end="url(#arrow)" marker-start="url(#arrow)""#.to_string()
        }
        EdgeKind::Open => String::new(),
    };
    let _ = write!(
        svg,
        r#"<path d="{d}" fill="none" stroke="{}" stroke-width="{width}"{dash}{markers}/>"#,
        theme.edge
    );

    if let (Some(label), Some(pos)) = (&edge.label, edge.label_pos) {
        let w = label.len() as f32 * 7.0 + 12.0;
        let _ = write!(
            svg,
            r#"<rect x="{:.1}" y="{:.1}" width="{w:.1}" height="20" rx="4" fill="{}" opacity="0.92"/><text x="{:.1}" y="{:.1}" font-size="12" fill="{}" text-anchor="middle" dominant-baseline="central">{}</text>"#,
            pos.x - w / 2.0,
            pos.y - 10.0,
            theme.background,
            pos.x,
            pos.y,
            theme.edge_label,
            escape(label)
        );
    }
}

fn write_node(svg: &mut String, node: &layra_core::Node, theme: &Theme) {
    let role_color = theme.role_color(node.role);
    shapes::write_shape(svg, node, role_color, theme);

    let c = node.rect.center();
    let lines: Vec<&str> = node.label.split('\n').collect();
    let line_h = 18.0;
    let start_y = c.y - (lines.len() as f32 - 1.0) * line_h / 2.0;
    for (i, line) in lines.iter().enumerate() {
        let _ = write!(
            svg,
            r#"<text x="{:.1}" y="{:.1}" font-size="14" fill="{}" text-anchor="middle" dominant-baseline="central">{}</text>"#,
            c.x,
            start_y + i as f32 * line_h,
            theme.text,
            escape(line)
        );
    }
}

pub(crate) fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use layra_core::{Direction, Node, Rect};

    #[test]
    fn renders_valid_svg_skeleton() {
        let mut g = Graph::new(Direction::TopBottom);
        let mut n = Node::new("a", "Hello & <World>");
        n.rect = Rect::new(10.0, 10.0, 120.0, 40.0);
        g.add_node(n);

        let svg = render(&g, &Theme::light());
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("&amp;"));
        assert!(svg.contains("&lt;World&gt;"));
        assert!(!svg.contains("<World>"));
    }
}
