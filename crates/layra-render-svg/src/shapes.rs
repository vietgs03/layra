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
        NodeShape::Actor | NodeShape::Cloud => {
            // Fallback to rounded rect until dedicated paths land.
            let _ = write!(
                svg,
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="10"{common}/>"#,
                r.x, r.y, r.width, r.height
            );
        }
    }
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
