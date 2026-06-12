//! # layra-text
//!
//! DOM-free text measurement. This is one of Layra's biggest wins over
//! Mermaid: Mermaid renders every label into the DOM and calls `getBBox()`,
//! causing layout thrashing. We measure with font metrics instead.
//!
//! Two tiers:
//! - **Metrics table** (this module, default): per-character advance widths
//!   for the UI font stack at a reference size. Accurate to ~2% for Latin
//!   text, costs nothing, works everywhere including WASM.
//! - **Shaped** (planned, behind a feature): `cosmic-text`/`rustybuzz` for
//!   exact shaping incl. CJK, ligatures, emoji.

use layra_core::{Graph, NodeShape, Size};

/// Measurement parameters.
#[derive(Debug, Clone, Copy)]
pub struct TextOptions {
    pub font_size: f32,
    pub line_height: f32,
    /// Padding added around the label inside the node, per side.
    pub padding_x: f32,
    pub padding_y: f32,
    pub min_node_width: f32,
    pub min_node_height: f32,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            line_height: 1.4,
            padding_x: 14.0,
            padding_y: 10.0,
            min_node_width: 56.0,
            min_node_height: 36.0,
        }
    }
}

/// Average advance widths (em units, i.e. fraction of font size) for the
/// common sans UI stack (Inter/Helvetica/Arial class). Buckets keep the
/// table tiny while staying within a few percent of real measurements.
fn char_em(c: char) -> f32 {
    match c {
        'i' | 'j' | 'l' | '!' | '|' | '\'' | '.' | ',' | ':' | ';' => 0.28,
        'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '{' | '}' | '/' | '\\' | ' ' => 0.35,
        'I' | '"' | '`' => 0.30,
        'm' | 'M' | 'W' | 'w' => 0.85,
        '@' => 0.95,
        'A'..='Z' => 0.68,
        '0'..='9' => 0.56,
        'a'..='z' => 0.52,
        '-' | '_' | '=' | '+' | '<' | '>' | '~' => 0.55,
        _ if (c as u32) > 0x2E80 => 1.0, // CJK and friends: full-width
        _ => 0.55,
    }
}

/// Measure a single line of text at `font_size`.
pub fn measure_line(text: &str, font_size: f32) -> f32 {
    text.chars().map(char_em).sum::<f32>() * font_size
}

/// Measure a (possibly multi-line) label. Lines split on `\n`.
pub fn measure_label(label: &str, opts: &TextOptions) -> Size {
    let mut width = 0.0f32;
    let mut lines = 0usize;
    for line in label.split('\n') {
        width = width.max(measure_line(line, opts.font_size));
        lines += 1;
    }
    let height = lines.max(1) as f32 * opts.font_size * opts.line_height;
    Size::new(width, height)
}

/// Extra padding factor certain shapes need so the label fits inside the
/// geometry (e.g. text inside a diamond only has ~50% usable width).
fn shape_factor(shape: NodeShape) -> (f32, f32) {
    match shape {
        NodeShape::Diamond => (1.9, 1.9),
        NodeShape::Circle => (1.35, 1.35),
        NodeShape::Hexagon => (1.25, 1.0),
        NodeShape::Cylinder => (1.0, 1.45),
        NodeShape::Stadium => (1.2, 1.0),
        _ => (1.0, 1.0),
    }
}

/// Icon block size used by both measurement and rendering.
pub const ICON_SIZE: f32 = 22.0;
/// Gap between the icon block and the first label line.
pub const ICON_GAP: f32 = 5.0;

/// Fill `node.size` for every node in the graph from its label and shape.
///
/// Icons render as a block above the label (the blog's editorial style),
/// so they add height, not width.
pub fn measure_graph(graph: &mut Graph, opts: &TextOptions) {
    for node in &mut graph.nodes {
        // Compartmented node (UML class / ER entity): title strip + one
        // block per section; width fits the longest line anywhere.
        if !node.sections.is_empty() {
            const TITLE_H: f32 = 30.0;
            const LINE_H: f32 = 17.0;
            let mut w = measure_line(&node.label, opts.font_size) + 28.0;
            let mut h = TITLE_H;
            for section in &node.sections {
                for line in section.split('\n') {
                    w = w.max(measure_line(line, 12.0) + 24.0);
                    h += LINE_H;
                }
                h += 6.0;
            }
            node.size = Size::new(w.max(110.0).ceil(), h.ceil());
            continue;
        }

        let text = measure_label(&node.label, opts);
        let icon_h = if node.icon.is_some() {
            ICON_SIZE + ICON_GAP
        } else {
            0.0
        };
        let (fx, fy) = shape_factor(node.shape);
        let mut w = (text.width.max(icon_h.min(ICON_SIZE)) * fx + opts.padding_x * 2.0)
            .max(opts.min_node_width);
        let mut h = ((text.height + icon_h) * fy + opts.padding_y * 2.0).max(opts.min_node_height);
        // Circles render with radius = max(w, h)/2, so layout must reserve a
        // square box or the drawn circle bleeds into rank spacing.
        if node.shape == NodeShape::Circle {
            let side = w.max(h);
            w = side;
            h = side;
        }
        node.size = Size::new(w.ceil(), h.ceil());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wider_text_measures_wider() {
        let opts = TextOptions::default();
        let a = measure_label("DB", &opts);
        let b = measure_label("PostgreSQL primary", &opts);
        assert!(b.width > a.width * 3.0);
    }

    #[test]
    fn multiline_grows_height_not_width() {
        let opts = TextOptions::default();
        let one = measure_label("hello world", &opts);
        let two = measure_label("hello\nworld", &opts);
        assert!(two.height > one.height);
        assert!(two.width < one.width);
    }
}
