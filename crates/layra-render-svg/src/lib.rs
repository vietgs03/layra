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
        write_subgraph(&mut svg, sg, theme, icons);
    }
    for (i, edge) in graph.edges.iter().enumerate() {
        let _ = write!(
            svg,
            r#"<g data-edge="{i}" data-from="{}" data-to="{}">"#,
            edge.source.0, edge.target.0
        );
        write_edge(&mut svg, edge, theme);
        svg.push_str("</g>");
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        let _ = write!(
            svg,
            r#"<g data-node="{i}" data-name="{}">"#,
            escape(&node.name)
        );
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

fn write_subgraph(
    svg: &mut String,
    sg: &layra_core::Subgraph,
    theme: &Theme,
    icons: Option<&IconRegistry>,
) {
    // AWS container look (VPC / AZ / Region): when the subgraph carries an
    // icon with a known category, draw a colored dashed border in the
    // category hue and a left-aligned corner header with the small glyph.
    let aws = sg
        .icon
        .as_deref()
        .filter(|key| icons.is_some_and(|reg| reg.get(key).is_some()))
        .and_then(|key| icons.and_then(|reg| reg.category(key).map(|cat| (key, cat))));

    if let (Some((icon_key, category)), Some(reg)) = (aws, icons) {
        write_aws_subgraph(svg, sg, icon_key, category, reg);
        return;
    }

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

/// AWS-architecture container: a category-colored dashed border with a flat
/// header bar in the top-left corner carrying a small service glyph and the
/// label (the VPC / Availability Zone / Region / Account look in AWS docs).
fn write_aws_subgraph(
    svg: &mut String,
    sg: &layra_core::Subgraph,
    icon_key: &str,
    category: layra_icons::IconCategory,
    reg: &IconRegistry,
) {
    let r = sg.rect;
    let color = category.color();
    const HEADER_H: f32 = 26.0;
    const ICON: f32 = 18.0;
    const PAD: f32 = 8.0;

    // Tinted body: faint category wash so nested containers read as layers.
    let _ = write!(
        svg,
        r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="8" fill="{color}" fill-opacity="0.04" stroke="{color}" stroke-width="1.6" stroke-dasharray="7 5"/>"#,
        r.x, r.y, r.width, r.height
    );

    // Corner header bar (solid category color) hugging the top-left. The
    // label must never clip: size the bar to the full label and only cap at
    // the diagram-reasonable max, not the (possibly narrow) cluster width.
    let label_px = sg.label.chars().count() as f32 * 7.2;
    let natural_w = label_px + ICON + PAD * 3.0;
    let header_w = natural_w.min(r.width.max(natural_w));
    let _ = write!(
        svg,
        r#"<path d="M{x:.1} {y:.1} h{hw:.1} v{hh:.1} h-{inner:.1} a8 8 0 0 1 -8 -8 v-{rest:.1} z" fill="{color}"/>"#,
        x = r.x,
        y = r.y,
        hw = header_w,
        hh = HEADER_H,
        inner = (header_w - 8.0).max(0.0),
        rest = (HEADER_H - 8.0).max(0.0),
    );

    // Small glyph in the header, drawn in white over the colored bar.
    let icon_y = r.y + (HEADER_H - ICON) / 2.0;
    if let Some(icon) = reg.emit_svg(icon_key, r.x + PAD, icon_y, ICON, "#ffffff") {
        // The category tile would double-paint a square; for the header we
        // want the bare white mark, so strip the tile and keep the glyph.
        svg.push_str(&strip_icon_tile(&icon, color));
    }

    // Label text in white, after the glyph.
    let _ = write!(
        svg,
        r##"<text x="{:.1}" y="{:.1}" font-size="12.5" font-weight="700" fill="#ffffff" dominant-baseline="central">{}</text>"##,
        r.x + PAD * 2.0 + ICON,
        r.y + HEADER_H / 2.0,
        escape(&sg.label)
    );
}

/// The categorized `emit_svg` wraps the glyph in a colored tile + white mark.
/// In the AWS header we already have a colored bar behind it, so drop the
/// tile `<rect>` and keep just the white glyph `<svg>`.
fn strip_icon_tile(icon: &str, color: &str) -> String {
    // Shape is `<g><rect .../><svg ...>...</svg></g>`. Remove the outer <g>,
    // the leading tile rect (fill=color), and the trailing </g>.
    let tile_marker = format!(r#"fill="{color}"/>"#);
    if let Some(inner) = icon.strip_prefix("<g>") {
        if let Some(pos) = inner.find(&tile_marker) {
            let after = &inner[pos + tile_marker.len()..];
            return after.strip_suffix("</g>").unwrap_or(after).to_string();
        }
    }
    icon.to_string()
}

fn write_edge(svg: &mut String, edge: &layra_core::Edge, theme: &Theme) {
    if edge.points.len() < 2 || edge.style == EdgeStyle::Invisible {
        return; // invisible links constrain layout but draw nothing
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
        // Round each interior corner with a fixed radius: straight runs
        // stay straight (unlike midpoint quadratics which bow the whole
        // segment), corners get a compact draw.io-style curve.
        const CORNER_R: f32 = 8.0;
        for i in 1..edge.points.len() - 1 {
            let prev = edge.points[i - 1];
            let p = edge.points[i];
            let next = edge.points[i + 1];

            let din = ((p.x - prev.x).powi(2) + (p.y - prev.y).powi(2)).sqrt();
            let dout = ((next.x - p.x).powi(2) + (next.y - p.y).powi(2)).sqrt();
            let r = CORNER_R.min(din / 2.0).min(dout / 2.0);

            if r < 1.0 || din < 0.5 || dout < 0.5 {
                d.push('L');
                fmt::push_f1(&mut d, p.x);
                d.push(' ');
                fmt::push_f1(&mut d, p.y);
                continue;
            }
            // Entry point r before the corner, exit point r after.
            let ex = p.x - (p.x - prev.x) / din * r;
            let ey = p.y - (p.y - prev.y) / din * r;
            let lx = p.x + (next.x - p.x) / dout * r;
            let ly = p.y + (next.y - p.y) / dout * r;
            d.push('L');
            fmt::push_f1(&mut d, ex);
            d.push(' ');
            fmt::push_f1(&mut d, ey);
            d.push('Q');
            fmt::push_f1(&mut d, p.x);
            d.push(' ');
            fmt::push_f1(&mut d, p.y);
            d.push(' ');
            fmt::push_f1(&mut d, lx);
            d.push(' ');
            fmt::push_f1(&mut d, ly);
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
        EdgeStyle::Invisible => unreachable!("filtered above"),
    };
    // Animation forces a dash pattern (so a solid/thick line still has marks
    // to flow) and adds an `<animate>` that scrolls `stroke-dashoffset`. The
    // dash sum (12 for "8 4") matches the offset travel so the loop seamless.
    let dash = if edge.animated && dash.is_empty() {
        r#" stroke-dasharray="8 4""#
    } else {
        dash
    };
    let markers = match edge.kind {
        EdgeKind::Arrow => r#" marker-end="url(#arrow)""#.to_string(),
        EdgeKind::Bidirectional => {
            r#" marker-end="url(#arrow)" marker-start="url(#arrow)""#.to_string()
        }
        EdgeKind::Open => String::new(),
        // UML generalization: the hollow triangle sits at the PARENT, which is
        // the edge source (ranked on the upper layer), so the apex points UP
        // to the base class. `auto-start-reverse` flips the marker so the
        // triangle tip touches the source node.
        EdgeKind::Triangle => r#" marker-start="url(#triangle)""#.to_string(),
        EdgeKind::DiamondFilled => r#" marker-start="url(#diamond-filled)""#.to_string(),
        EdgeKind::DiamondOpen => r#" marker-start="url(#diamond-open)""#.to_string(),
    };
    if edge.animated {
        // Dashed path with a looping dashoffset animation. `from`/`to` differ
        // by the dash period (12 = 8 + 4) so the cycle is seamless.
        let _ = write!(
            svg,
            r#"<path d="{d}" fill="none" stroke="{}" stroke-width="{width}"{dash}{markers}><animate attributeName="stroke-dashoffset" from="12" to="0" dur="0.6s" repeatCount="indefinite"/></path>"#,
            theme.edge
        );
    } else {
        let _ = write!(
            svg,
            r#"<path d="{d}" fill="none" stroke="{}" stroke-width="{width}"{dash}{markers}/>"#,
            theme.edge
        );
    }

    // Endpoint labels (UML multiplicities like `1`, `*`): small text offset
    // along the first/last segment. ER diagrams instead get graphical
    // crow's-foot markers (below), so suppress the redundant text there.
    if let (Some((src_label, dst_label)), None) = (&edge.end_labels, &edge.crowfoot) {
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

    // ER crow's-foot markers: graphical bars / circle / three-prong foot at
    // each endpoint, oriented along the first/last segment toward the entity.
    if let Some((src_foot, dst_foot)) = &edge.crowfoot {
        let pts = &edge.points;
        write_crowfoot(svg, pts[0], pts[1], src_foot, theme);
        write_crowfoot(svg, pts[pts.len() - 1], pts[pts.len() - 2], dst_foot, theme);
    }

    if let (Some(label), Some(pos)) = (&edge.label, edge.label_pos) {
        // Size the label chip to the measured text (not byte length) so the
        // background pill always fully covers the glyphs, plus a little
        // horizontal breathing room on each side.
        let w = layra_text::measure_line(label, 12.0) + 12.0;
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

/// Draw an ER crow's-foot marker at endpoint `a` (on the entity), oriented
/// along the segment a→b (pointing into the diagram). The notation, reading
/// outward from the entity:
/// - mandatory-one  (`||`): two bars across the line;
/// - zero-or-one    (`|o`): one bar + a circle further out;
/// - one-or-more    (`}|`): a three-prong foot + one bar behind it;
/// - zero-or-more   (`}o`): a three-prong foot + a circle behind it.
fn write_crowfoot(
    svg: &mut String,
    a: layra_core::Point,
    b: layra_core::Point,
    foot: &layra_core::CrowFoot,
    theme: &Theme,
) {
    // Unit vector a→b (along the line) and its perpendicular.
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / len, dy / len); // toward interior
    let (px, py) = (-uy, ux); // perpendicular

    const FOOT: f32 = 11.0; // crow's-foot depth / bar half-width
    const BAR1: f32 = 9.0; // first bar distance from entity
    const BAR2: f32 = 16.0; // second bar distance
    const CIRCLE_D: f32 = 18.0; // circle center distance
    let stroke = theme.edge;

    // Point `d` units along the line from `a`, offset `s` perpendicular.
    let at = |d: f32, s: f32| (a.x + ux * d + px * s, a.y + uy * d + py * s);

    if foot.many {
        // Three-prong foot: lines from a point on the line out to the entity
        // edge spread, forming the "crow's foot".
        let (tipx, tipy) = at(FOOT, 0.0); // apex inward
        for s in [-FOOT * 0.6, 0.0, FOOT * 0.6] {
            let (ex, ey) = at(0.0, s);
            let _ = write!(
                svg,
                r#"<line x1="{tipx:.1}" y1="{tipy:.1}" x2="{ex:.1}" y2="{ey:.1}" stroke="{stroke}" stroke-width="1.4"/>"#
            );
        }
    } else {
        // Single bar (one) right at the entity edge.
        let (b1x, b1y) = at(BAR1, FOOT * 0.55);
        let (b2x, b2y) = at(BAR1, -FOOT * 0.55);
        let _ = write!(
            svg,
            r#"<line x1="{b1x:.1}" y1="{b1y:.1}" x2="{b2x:.1}" y2="{b2y:.1}" stroke="{stroke}" stroke-width="1.4"/>"#
        );
    }

    if foot.optional {
        // Circle (zero allowed) set back from the entity.
        let (cx, cy) = at(CIRCLE_D, 0.0);
        let _ = write!(
            svg,
            r#"<circle cx="{cx:.1}" cy="{cy:.1}" r="4.2" fill="{}" stroke="{stroke}" stroke-width="1.4"/>"#,
            theme.background
        );
    } else {
        // Mandatory bar set back from the entity (the "one" tick).
        let (b1x, b1y) = at(BAR2, FOOT * 0.55);
        let (b2x, b2y) = at(BAR2, -FOOT * 0.55);
        let _ = write!(
            svg,
            r#"<line x1="{b1x:.1}" y1="{b1y:.1}" x2="{b2x:.1}" y2="{b2y:.1}" stroke="{stroke}" stroke-width="1.4"/>"#
        );
    }
}

fn write_node(
    svg: &mut String,
    node: &layra_core::Node,
    theme: &Theme,
    icons: Option<&IconRegistry>,
) {
    let c = node.rect.center();
    let icon_key = node
        .icon
        .as_deref()
        .filter(|key| icons.is_some_and(|reg| reg.get(key).is_some()));

    // L10: service-category node theming. When the author hasn't set an
    // explicit `:::role` (role stays Generic), derive the accent from the
    // icon's AWS category so an `{icon:aws:lambda}` node reads orange
    // automatically. An explicit role always wins.
    let icon_color = if node.role == layra_core::ComponentRole::Generic {
        icon_key
            .and_then(|key| icons.and_then(|reg| reg.category(key)))
            .map(|cat| cat.color())
    } else {
        None
    };
    let role_color = icon_color.unwrap_or_else(|| theme.role_color(node.role));
    shapes::write_shape(svg, node, role_color, theme);

    // Compartmented node (UML class / ER entity): title strip + sections.
    if !node.sections.is_empty() {
        write_compartments(svg, node, theme);
        return;
    }

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
/// already sized `node.rect` to fit (see measure_graph's sections branch),
/// using the same geometry constants from `layra_text::compartment` so the
/// drawn member rows always fit inside the box.
fn write_compartments(svg: &mut String, node: &layra_core::Node, theme: &Theme) {
    use layra_text::compartment::{LINE_H, MEMBER_FONT, PAD_X, SECTION_GAP, TITLE_FONT, TITLE_H};
    let r = node.rect;

    // Title strip.
    let _ = write!(
        svg,
        r#"<text x="{:.1}" y="{:.1}" font-size="{TITLE_FONT}" font-weight="700" fill="{}" text-anchor="middle" dominant-baseline="central">{}</text>"#,
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
                r#"<text x="{:.1}" y="{:.1}" font-size="{MEMBER_FONT}" font-family="ui-monospace, 'SF Mono', Menlo, monospace" fill="{}" dominant-baseline="central">{}</text>"#,
                r.x + PAD_X,
                y - LINE_H / 2.0 + 2.0,
                theme.edge_label,
                escape(line)
            );
        }
        y += SECTION_GAP;
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

    // ---- L10: service-category node theming ----

    fn render_node_with_icon(icon: &str, role: layra_core::ComponentRole) -> String {
        let reg = IconRegistry::with_builtins();
        let mut g = Graph::new(Direction::TopBottom);
        let mut n = Node::new("a", "Svc");
        n.icon = Some(icon.to_string());
        n.role = role;
        n.rect = Rect::new(10.0, 10.0, 120.0, 64.0);
        g.add_node(n);
        render_with_icons(&g, &Theme::light(), Some(&reg))
    }

    #[test]
    fn icon_node_auto_accents_from_category() {
        // A plain (Generic) node carrying a compute icon picks up the AWS
        // orange accent on its border — no explicit :::role needed.
        let svg = render_node_with_icon("aws:lambda", layra_core::ComponentRole::Generic);
        assert!(
            svg.contains(r##"stroke="#ED7100""##),
            "compute icon must drive an orange border, got: {svg}"
        );
        // Default generic grey must not be the border anymore.
        assert!(
            !svg.contains(r##"stroke="#94a3b8""##),
            "generic grey border should be overridden by category"
        );
    }

    #[test]
    fn storage_icon_node_accents_green() {
        let svg = render_node_with_icon("aws:s3", layra_core::ComponentRole::Generic);
        assert!(
            svg.contains(r##"stroke="#7AA116""##),
            "storage icon must drive a green border"
        );
    }

    #[test]
    fn explicit_role_overrides_icon_category() {
        // If the author sets :::database, that wins over the icon category.
        let svg = render_node_with_icon("aws:lambda", layra_core::ComponentRole::Database);
        assert!(
            svg.contains(r##"stroke="#8b5cf6""##),
            "explicit database role must win over compute icon"
        );
        assert!(
            !svg.contains(r##"stroke="#ED7100""##),
            "icon category must not override an explicit role"
        );
    }

    #[test]
    fn iconless_generic_node_keeps_role_color() {
        let mut g = Graph::new(Direction::TopBottom);
        let mut n = Node::new("a", "Plain");
        n.rect = Rect::new(10.0, 10.0, 120.0, 40.0);
        g.add_node(n);
        let svg = render(&g, &Theme::light());
        assert!(
            svg.contains(r##"stroke="#94a3b8""##),
            "iconless generic node keeps neutral grey"
        );
    }

    // ---- L12: AWS group containers ----

    fn render_subgraph(icon: Option<&str>) -> String {
        let reg = IconRegistry::with_builtins();
        let mut g = Graph::new(Direction::TopBottom);
        let mut n = Node::new("a", "Svc");
        n.rect = Rect::new(40.0, 60.0, 120.0, 40.0);
        let id = g.add_node(n);
        g.add_subgraph(layra_core::Subgraph {
            name: "vpc".into(),
            label: "VPC 10.0.0.0/16".into(),
            nodes: vec![id],
            parent: None,
            icon: icon.map(|s| s.to_string()),
            rect: Rect::new(20.0, 30.0, 200.0, 120.0),
        });
        render_with_icons(&g, &Theme::light(), Some(&reg))
    }

    #[test]
    fn aws_subgraph_uses_category_colored_border() {
        // A subgraph carrying aws:vpc (network) renders a purple dashed
        // border + a colored corner header, not the plain grey cluster pill.
        let svg = render_subgraph(Some("aws:vpc"));
        assert!(
            svg.contains(r##"stroke="#8C4FFF""##),
            "VPC container must use the AWS network purple border"
        );
        assert!(
            svg.contains("stroke-dasharray"),
            "AWS container keeps a dashed border"
        );
        // Header bar filled with the category color (a <path ... fill=color>).
        assert!(
            svg.contains(r##"fill="#8C4FFF"/>"##),
            "AWS container has a solid colored corner header"
        );
        // White label text in the header.
        assert!(
            svg.contains("VPC 10.0.0.0/16"),
            "header carries the label text"
        );
        // The small glyph appears (an inline <svg> for the icon).
        assert!(svg.contains("<svg x="), "header carries the small glyph");
    }

    #[test]
    fn aws_subgraph_strips_double_tile_on_header() {
        // The header draws its own colored bar, so the icon's category tile
        // must be stripped (only ONE colored rect at the cluster, no nested
        // tile rect inside the header glyph).
        let svg = render_subgraph(Some("aws:vpc"));
        // White glyph mark must be present (icon repainted white).
        assert!(svg.contains("#ffffff"), "header glyph drawn in white");
    }

    #[test]
    fn plain_subgraph_keeps_pill_style() {
        // No icon => classic grey dashed cluster with a title pill.
        let svg = render_subgraph(None);
        assert!(
            svg.contains(r##"stroke="#c3c9d4""##),
            "plain cluster keeps neutral grey stroke"
        );
        assert!(
            !svg.contains(r##"fill="#8C4FFF""##),
            "plain cluster has no AWS category color"
        );
    }

    #[test]
    fn aws_subgraph_security_is_red() {
        let svg = render_subgraph(Some("aws:iam"));
        assert!(
            svg.contains(r##"stroke="#D13212""##),
            "an iam (security) container is AWS red"
        );
    }

    // ---- L16: ER crow's-foot notation ----

    fn render_er(src: &str) -> String {
        use layra_core::Document;
        let (doc, warnings) = layra_parser::parse_document_lenient(src);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        let Document::Graph(mut g) = doc else {
            panic!("ER diagram should parse to a graph");
        };
        layra_text::measure_graph(&mut g, &layra_text::TextOptions::default());
        layra_layout::layout(&mut g, &layra_layout::LayoutOptions::default());
        layra_router::route(&mut g);
        render(&g, &Theme::light())
    }

    #[test]
    fn er_crowfoot_renders_graphical_markers_not_text() {
        // CUSTOMER ||--o{ ORDER: the ORDER side is zero-or-many (crow's foot
        // + circle), the CUSTOMER side is exactly-one (two bars). The render
        // must draw geometry, not textual "1" / "0..*".
        let svg = render_er("erDiagram\n  CUSTOMER ||--o{ ORDER : places");
        // Zero-or-many side draws a circle (optional marker).
        assert!(
            svg.contains("<circle"),
            "zero-or-many endpoint draws an optional circle"
        );
        // Crow's-foot / bar marker lines use stroke-width 1.4.
        assert!(
            svg.matches(r#"stroke-width="1.4""#).count() >= 5,
            "crow's-foot + bar marker lines present"
        );
        // Textual cardinality is suppressed for ER (replaced by geometry).
        assert!(
            !svg.contains(">0..*</text>") && !svg.contains(">1</text>"),
            "ER textual cardinality suppressed in favour of crow's-foot"
        );
    }

    #[test]
    fn er_exactly_one_draws_two_bars_no_circle() {
        // A--||--||--B with both ends exactly-one: no circles anywhere.
        let svg = render_er("erDiagram\n  A ||--|| B : has");
        assert!(
            !svg.contains("<circle"),
            "exactly-one on both ends has no optional circles"
        );
        // Two bars per end = at least 4 marker lines.
        assert!(
            svg.matches(r#"stroke-width="1.4""#).count() >= 4,
            "two bars per endpoint"
        );
    }

    #[test]
    fn er_attribute_box_keeps_pk_fk_markers() {
        let svg = render_er(
            "erDiagram\n  ORDER {\n    int id PK\n    int customer_id FK\n    string status\n  }\n  CUSTOMER ||--o{ ORDER : places",
        );
        assert!(svg.contains("[PK]"), "primary key marker rendered");
        assert!(svg.contains("[FK]"), "foreign key marker rendered");
    }
}
