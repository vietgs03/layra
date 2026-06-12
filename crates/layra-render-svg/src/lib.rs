//! # layra-render-svg
//!
//! Renders a laid-out [`Graph`] to a standalone SVG string.
//!
//! Style follows the diagram-toolkit editorial signature: flat white nodes,
//! colored role borders (the component taxonomy), dashed cluster pills with
//! colored title pills, thin neutral edges.

mod fmt;
mod gantt;
mod linear;
mod pie;
mod sequence;
mod shapes;
mod theme;

use layra_core::{EdgeKind, EdgeStyle, Graph};
use layra_icons::IconRegistry;
use std::fmt::Write;

pub use gantt::render_gantt;
pub use linear::{render_git, render_journey, render_timeline};
pub use pie::render_pie;
pub use sequence::render_sequence;
pub use theme::Theme;

pub(crate) const FONT_STACK: &str = "Inter, 'Helvetica Neue', Arial, sans-serif";

/// Render `graph` to SVG without icons.
pub fn render(graph: &Graph, theme: &Theme) -> String {
    render_with_icons(graph, theme, None)
}

/// Render `graph` to SVG, resolving node icons from `icons` when given.
/// The graph must already be measured, laid out, and routed.
pub fn render_with_icons(graph: &Graph, theme: &Theme, icons: Option<&IconRegistry>) -> String {
    let bounds = graph.bounds().inflate(16.0);
    let w = bounds.width.ceil();
    let h = bounds.height.ceil();

    // ~220 bytes/node + ~180 bytes/edge empirically; pre-size to avoid
    // repeated buffer growth on large graphs.
    let estimate = 1024 + graph.nodes.len() * 260 + graph.edges.len() * 200;
    let mut svg = String::with_capacity(estimate);
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
    for (i, edge) in graph.edges.iter().enumerate() {
        let _ = write!(svg, r#"<g data-edge="{i}">"#);
        write_edge(&mut svg, edge, theme);
        svg.push_str("</g>");
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        let _ = write!(svg, r#"<g data-node="{i}">"#);
        write_node(&mut svg, node, theme, icons);
        svg.push_str("</g>");
    }

    svg.push_str("</svg>");
    svg
}

fn write_defs(svg: &mut String, theme: &Theme) {
    let e = theme.edge;
    let bg = theme.node_fill;
    svg.push_str("<defs>");
    let _ = write!(
        svg,
        r#"<marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="{e}"/></marker>"#
    );
    let _ = write!(
        svg,
        r#"<marker id="triangle" viewBox="0 0 12 12" refX="11" refY="6" markerWidth="11" markerHeight="11" orient="auto-start-reverse"><path d="M1 1L11 6L1 11z" fill="{bg}" stroke="{e}" stroke-width="1.2"/></marker>"#
    );
    let _ = write!(
        svg,
        r#"<marker id="diamond-filled" viewBox="0 0 14 10" refX="1" refY="5" markerWidth="13" markerHeight="9" orient="auto"><path d="M1 5L7 1L13 5L7 9z" fill="{e}"/></marker>"#
    );
    let _ = write!(
        svg,
        r#"<marker id="diamond-open" viewBox="0 0 14 10" refX="1" refY="5" markerWidth="13" markerHeight="9" orient="auto"><path d="M1 5L7 1L13 5L7 9z" fill="{bg}" stroke="{e}" stroke-width="1.2"/></marker>"#
    );
    svg.push_str("</defs>");
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

    // Path data is the hottest string in the renderer (one coordinate pair
    // per waypoint per edge) — build it with the fast formatter.
    let mut d = String::with_capacity(16 + edge.points.len() * 16);
    d.push('M');
    fmt::push_f1(&mut d, edge.points[0].x);
    d.push(' ');
    fmt::push_f1(&mut d, edge.points[0].y);
    if edge.points.len() == 2 {
        d.push('L');
        fmt::push_f1(&mut d, edge.points[1].x);
        d.push(' ');
        fmt::push_f1(&mut d, edge.points[1].y);
    } else {
        // Smooth through waypoints with quadratic joins.
        for i in 1..edge.points.len() - 1 {
            let p = edge.points[i];
            let next = edge.points[i + 1];
            d.push('Q');
            fmt::push_f1(&mut d, p.x);
            d.push(' ');
            fmt::push_f1(&mut d, p.y);
            d.push(' ');
            fmt::push_f1(&mut d, (p.x + next.x) / 2.0);
            d.push(' ');
            fmt::push_f1(&mut d, (p.y + next.y) / 2.0);
        }
        let last = edge.points[edge.points.len() - 1];
        d.push('L');
        fmt::push_f1(&mut d, last.x);
        d.push(' ');
        fmt::push_f1(&mut d, last.y);
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
        EdgeKind::Triangle => r#" marker-end="url(#triangle)""#.to_string(),
        EdgeKind::DiamondFilled => r#" marker-start="url(#diamond-filled)""#.to_string(),
        EdgeKind::DiamondOpen => r#" marker-start="url(#diamond-open)""#.to_string(),
    };
    let _ = write!(
        svg,
        r#"<path d="{d}" fill="none" stroke="{}" stroke-width="{width}"{dash}{markers}/>"#,
        theme.edge
    );

    // Endpoint labels (ER cardinality / UML multiplicities): small text
    // offset a fixed distance along the first/last segment.
    if let Some((src_label, dst_label)) = &edge.end_labels {
        let place = |out: &mut String, a: layra_core::Point, b: layra_core::Point, text: &str| {
            if text.is_empty() {
                return;
            }
            let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt().max(1.0);
            let t = (18.0 / len).min(0.4);
            let x = a.x + (b.x - a.x) * t;
            let y = a.y + (b.y - a.y) * t - 7.0;
            let _ = write!(
                out,
                r#"<text x="{x:.1}" y="{y:.1}" font-size="11" fill="{}" text-anchor="middle">{}</text>"#,
                theme.edge_label,
                escape(text)
            );
        };
        let pts = &edge.points;
        place(svg, pts[0], pts[1], src_label);
        place(svg, pts[pts.len() - 1], pts[pts.len() - 2], dst_label);
    }

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

fn write_node(
    svg: &mut String,
    node: &layra_core::Node,
    theme: &Theme,
    icons: Option<&IconRegistry>,
) {
    let role_color = theme.role_color(node.role);
    shapes::write_shape(svg, node, role_color, theme);

    // Compartmented node (UML class / ER entity): title strip + sections.
    if !node.sections.is_empty() {
        write_compartments(svg, node, theme);
        return;
    }

    let c = node.rect.center();
    let icon_key = node
        .icon
        .as_deref()
        .filter(|key| icons.is_some_and(|reg| reg.get(key).is_some()));

    let lines: Vec<&str> = node.label.split('\n').collect();
    let line_h = 18.0;
    let icon_h = if icon_key.is_some() {
        layra_text::ICON_SIZE + layra_text::ICON_GAP
    } else {
        0.0
    };
    let content_h = icon_h + lines.len() as f32 * line_h;
    let top = c.y - content_h / 2.0;

    if let (Some(key), Some(reg)) = (icon_key, icons) {
        if let Some(icon) = reg.emit_svg(
            key,
            c.x - layra_text::ICON_SIZE / 2.0,
            top,
            layra_text::ICON_SIZE,
            theme.text,
        ) {
            svg.push_str(&icon);
        }
    }

    let text_start = top + icon_h + line_h / 2.0;
    for (i, line) in lines.iter().enumerate() {
        // First line is the title; subsequent lines render smaller+dimmer
        // (the blog's `<span class='sub'>` convention).
        let (size, fill) = if i == 0 {
            (14.0, theme.text)
        } else {
            (11.5, theme.edge_label)
        };
        let _ = write!(
            svg,
            r#"<text x="{:.1}" y="{:.1}" font-size="{size}" fill="{fill}" text-anchor="middle" dominant-baseline="central">{}</text>"#,
            c.x,
            text_start + i as f32 * line_h,
            escape(line)
        );
    }
}

/// UML-class-style node: bold title strip, horizontal separators, and
/// left-aligned monospaced compartment lines. The text-measure stage
/// already sized `node.rect` to fit (see measure_graph's sections branch).
fn write_compartments(svg: &mut String, node: &layra_core::Node, theme: &Theme) {
    let r = node.rect;
    const TITLE_H: f32 = 30.0;
    const LINE_H: f32 = 17.0;
    const PAD_X: f32 = 10.0;

    // Title strip.
    let _ = write!(
        svg,
        r#"<text x="{:.1}" y="{:.1}" font-size="13.5" font-weight="700" fill="{}" text-anchor="middle" dominant-baseline="central">{}</text>"#,
        r.center().x,
        r.y + TITLE_H / 2.0,
        theme.text,
        escape(&node.label)
    );

    let mut y = r.y + TITLE_H;
    for section in &node.sections {
        // Separator above each compartment.
        let _ = write!(
            svg,
            r#"<line x1="{:.1}" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke="{}" stroke-width="1"/>"#,
            r.x,
            r.right(),
            theme.cluster_stroke
        );
        for line in section.split('\n') {
            y += LINE_H;
            let _ = write!(
                svg,
                r#"<text x="{:.1}" y="{:.1}" font-size="12" font-family="ui-monospace, 'SF Mono', Menlo, monospace" fill="{}" dominant-baseline="central">{}</text>"#,
                r.x + PAD_X,
                y - LINE_H / 2.0 + 2.0,
                theme.edge_label,
                escape(line)
            );
        }
        y += 6.0;
    }
}

/// Escape text for SVG. Borrows when no escaping is needed (the common
/// case), avoiding four chained replace() allocations per label.
pub(crate) fn escape(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains(['&', '<', '>', '"']) {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    std::borrow::Cow::Owned(out)
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
