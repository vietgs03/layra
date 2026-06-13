//! Shape library: outline path generation per [`NodeShape`].

use crate::theme::Theme;
use layra_core::{Node, NodeShape, Rect};
use std::fmt::Write;

const STROKE_W: f32 = 1.8;

pub(crate) fn write_shape(svg: &mut String, node: &Node, stroke: &str, theme: &Theme) {
    let r = node.rect;
    let common = format!(
        r#" fill="{}" stroke="{stroke}" stroke-width="{STROKE_W}""#,
        theme.node_fill
    );

    match node.shape {
        NodeShape::Rect => {
            let _ = write!(
                svg,
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="3"{common}/>"#,
                r.x, r.y, r.width, r.height
            );
        }
        NodeShape::RoundedRect => {
            let _ = write!(
                svg,
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="10"{common}/>"#,
                r.x, r.y, r.width, r.height
            );
        }
        NodeShape::Stadium => {
            let _ = write!(
                svg,
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.1}"{common}/>"#,
                r.x,
                r.y,
                r.width,
                r.height,
                r.height / 2.0
            );
        }
        NodeShape::Circle => {
            let c = r.center();
            let radius = r.width.max(r.height) / 2.0;
            let _ = write!(
                svg,
                r#"<circle cx="{:.1}" cy="{:.1}" r="{radius:.1}"{common}/>"#,
                c.x, c.y
            );
        }
        NodeShape::Diamond => {
            let c = r.center();
            let _ = write!(
                svg,
                r#"<polygon points="{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}"{common}/>"#,
                c.x,
                r.y,
                r.right(),
                c.y,
                c.x,
                r.bottom(),
                r.x,
                c.y
            );
        }
        NodeShape::Hexagon => {
            let inset = (r.height / 2.0).min(r.width / 4.0);
            let _ = write!(
                svg,
                r#"<polygon points="{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}"{common}/>"#,
                r.x + inset,
                r.y,
                r.right() - inset,
                r.y,
                r.right(),
                r.center().y,
                r.right() - inset,
                r.bottom(),
                r.x + inset,
                r.bottom(),
                r.x,
                r.center().y
            );
        }
        NodeShape::Cylinder => write_cylinder(svg, r, &common),
        NodeShape::Subroutine => {
            // Process box: rect with vertical bars set in from each side.
            let inset = (r.width * 0.08).clamp(4.0, 12.0);
            let _ = write!(
                svg,
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="3"{common}/><line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{stroke}" stroke-width="{STROKE_W}"/><line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{stroke}" stroke-width="{STROKE_W}"/>"#,
                r.x,
                r.y,
                r.width,
                r.height,
                r.x + inset,
                r.y,
                r.x + inset,
                r.bottom(),
                r.right() - inset,
                r.y,
                r.right() - inset,
                r.bottom()
            );
        }
        NodeShape::Queue => {
            // Horizontal pipe: rect with elliptical right cap hint.
            let _ = write!(
                svg,
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.1}" ry="{:.1}"{common}/>"#,
                r.x,
                r.y,
                r.width,
                r.height,
                r.height / 4.0,
                r.height / 2.0
            );
        }
        NodeShape::Cloud => write_cloud(svg, r, &common),
        NodeShape::Actor => {
            // Fallback to rounded rect until a dedicated actor glyph lands.
            let _ = write!(
                svg,
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="10"{common}/>"#,
                r.x, r.y, r.width, r.height
            );
        }
    }
}

/// Cloud: a rounded blob built from arcs along the top and a flat-ish base,
/// scaled to fit the node rect. The classic VPC / internet boundary shape.
fn write_cloud(svg: &mut String, r: Rect, common: &str) {
    // Bump radii relative to size; clamp so tiny nodes still read as clouds.
    let bx = (r.width * 0.18).clamp(8.0, 40.0);
    let by = (r.height * 0.32).clamp(8.0, 40.0);
    let left = r.x + bx * 0.4;
    let right = r.right() - bx * 0.4;
    let top = r.y + by * 0.6;
    let bot = r.bottom() - by * 0.2;
    let midx = r.center().x;
    let _ = write!(
        svg,
        r#"<path d="M{left:.1} {bot:.1} C{l2:.1} {bot:.1} {l3:.1} {top:.1} {ql:.1} {top:.1} C{qm:.1} {ty:.1} {qr:.1} {ty:.1} {qr2:.1} {top:.1} C{r3:.1} {top:.1} {r2:.1} {bot:.1} {right:.1} {bot:.1} Z"{common}/>"#,
        l2 = left - bx * 0.3,
        l3 = left,
        ql = r.x + r.width * 0.32,
        qm = midx - r.width * 0.05,
        ty = r.y + by * 0.1,
        qr = midx + r.width * 0.05,
        qr2 = r.x + r.width * 0.68,
        r3 = right,
        r2 = right + bx * 0.3,
    );
}

/// Database cylinder: body path + top ellipse.
fn write_cylinder(svg: &mut String, r: Rect, common: &str) {
    let ry = (r.height * 0.14).min(12.0);
    let _ = write!(
        svg,
        r#"<path d="M{x:.1} {top:.1} A{rx:.1} {ry:.1} 0 0 1 {right:.1} {top:.1} V{bot:.1} A{rx:.1} {ry:.1} 0 0 1 {x:.1} {bot:.1} Z"{common}/><ellipse cx="{cx:.1}" cy="{top:.1}" rx="{rx:.1}" ry="{ry:.1}"{common}/>"#,
        x = r.x,
        right = r.right(),
        top = r.y + ry,
        bot = r.bottom() - ry,
        rx = r.width / 2.0,
        cx = r.center().x,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use layra_core::{ComponentRole, Node, NodeShape, Rect};

    fn render(shape: NodeShape) -> String {
        let mut node = Node::new("n", "Label");
        node.shape = shape;
        node.role = ComponentRole::Generic;
        node.rect = Rect::new(10.0, 10.0, 120.0, 48.0);
        let mut svg = String::new();
        write_shape(&mut svg, &node, "#333", &Theme::light());
        svg
    }

    #[test]
    fn subroutine_has_side_bars() {
        let svg = render(NodeShape::Subroutine);
        assert!(svg.contains("<rect"), "subroutine keeps a body rect");
        // Two inset vertical bars distinguish it from a plain rect.
        assert_eq!(svg.matches("<line").count(), 2, "subroutine needs 2 bars");
    }

    #[test]
    fn cloud_is_a_curved_path() {
        let svg = render(NodeShape::Cloud);
        assert!(svg.contains("<path"), "cloud must be a path");
        assert!(svg.contains("C"), "cloud path must use cubic curves");
        assert!(!svg.contains("<rect"), "cloud must not fall back to a rect");
    }
}
