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

/// Per-character advance widths (em units, i.e. fraction of font size) for
/// the common sans UI stack (Inter / Helvetica / Arial class).
///
/// The table is a deliberate **upper bound**: values are taken from the
/// widest mainstream member of the stack (Helvetica/Arial AFM advances,
/// em = units/1000) and never under-shoot it. Inter is narrower, so a box
/// sized from this table fits Inter too. Over-measuring slightly is the
/// safe direction — a label never overflows its node — while still being
/// within a few percent of the real advance for Latin text.
fn char_em(c: char) -> f32 {
    match c {
        ' ' => 0.30,
        '!' => 0.30,
        '"' => 0.36,
        '#' => 0.56,
        '$' => 0.56,
        '%' => 0.90,
        '&' => 0.68,
        '\'' => 0.20,
        '(' | ')' => 0.34,
        '*' => 0.39,
        '+' | '<' | '=' | '>' | '~' => 0.59,
        ',' => 0.28,
        '-' => 0.34,
        '.' => 0.28,
        '/' | '\\' => 0.30,
        '0'..='9' => 0.56,
        ':' | ';' => 0.28,
        '?' => 0.56,
        '@' => 1.02,
        // Capitals: real per-letter advances (the old 0.68 bucket
        // under-measured C/G/O/Q/D/H/U/V/X/Y and especially M/W).
        'A' | 'B' | 'E' | 'K' | 'P' | 'S' | 'V' | 'X' | 'Y' => 0.68,
        'C' | 'D' | 'H' | 'N' | 'R' | 'U' => 0.73,
        'F' | 'T' | 'Z' => 0.62,
        'G' | 'O' | 'Q' => 0.78,
        'I' => 0.30,
        'J' => 0.50,
        'L' => 0.56,
        'M' => 0.84,
        'W' => 0.95,
        '[' | ']' => 0.30,
        '^' => 0.47,
        '_' => 0.56,
        '`' => 0.34,
        // Lowercase: per-letter advances.
        'a' | 'b' | 'd' | 'e' | 'g' | 'h' | 'k' | 'n' | 'o' | 'p' | 'q' | 'u' | 'x' | 'y' | 'z' => {
            0.56
        }
        'c' | 's' | 'v' => 0.50,
        'f' | 't' => 0.30,
        'i' | 'j' | 'l' => 0.24,
        'm' => 0.84,
        'r' => 0.34,
        'w' => 0.73,
        '{' | '}' => 0.34,
        '|' => 0.27,
        // Emoji render wider than a nominal em advance.
        c if is_emoji(c) => 1.2,
        // CJK / full-width ideographs and kana.
        _ if (c as u32) >= 0x2E80 => 1.0,
        // Other Latin-1 / accented letters: ~base letter width.
        _ => 0.56,
    }
}

/// Rough emoji-range test (pictographs, symbols, supplementary planes) so
/// emoji glyphs reserve their wider advance.
fn is_emoji(c: char) -> bool {
    let u = c as u32;
    (0x2600..=0x27BF).contains(&u)        // misc symbols + dingbats
        || (0x1F000..=0x1FAFF).contains(&u) // pictographs / supplemental
        || (0x2190..=0x21FF).contains(&u)   // arrows (often emoji-presented)
        || u == 0x200D                      // ZWJ (sequence joiner)
        || (0x1F1E6..=0x1F1FF).contains(&u) // regional indicators (flags)
}

/// Measure a single line of text at `font_size`.
pub fn measure_line(text: &str, font_size: f32) -> f32 {
    text.chars().map(char_em).sum::<f32>() * font_size
}

/// Monospace advance (em) used for compartment member rows. The renderer
/// draws those rows with `font-family: ui-monospace, 'SF Mono', Menlo,
/// monospace`; every glyph in such a face advances a fixed width. We take a
/// safe upper bound over the common system monospaces (SF Mono / Menlo are
/// ~0.602em) so a measured box never under-shoots the drawn text.
pub const MONO_EM: f32 = 0.62;

/// Measure a single monospace line at `font_size` (fixed advance per char).
/// Used for UML/ER compartment rows so wide member signatures like
/// `+eat(food: String) boolean` don't overflow their box.
pub fn measure_line_mono(text: &str, font_size: f32) -> f32 {
    text.chars().count() as f32 * MONO_EM * font_size
}

/// Geometry of a compartmented node (UML class / ER entity), shared by the
/// text-measure stage and the SVG renderer so a box is always sized to the
/// text it will actually draw. Keeping these in one place is what guarantees
/// `width == text_width + 2*PAD_X` for every member row.
pub mod compartment {
    /// Horizontal padding inside each compartment, per side.
    pub const PAD_X: f32 = 10.0;
    /// Height of the bold title strip.
    pub const TITLE_H: f32 = 30.0;
    /// Height of each member row.
    pub const LINE_H: f32 = 17.0;
    /// Vertical gap after each section block.
    pub const SECTION_GAP: f32 = 6.0;
    /// Title font size (drawn bold and centered).
    pub const TITLE_FONT: f32 = 13.5;
    /// Member-row font size (drawn monospace, left-aligned).
    pub const MEMBER_FONT: f32 = 12.0;
    /// Bold weighting: a bold title advances a few percent wider than the
    /// regular metrics table predicts, so pad the measured title slightly.
    pub const TITLE_BOLD_FACTOR: f32 = 1.06;
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
        // Side bars eat horizontal room; cloud blob needs slack all round.
        NodeShape::Subroutine => (1.18, 1.0),
        NodeShape::Cloud => (1.35, 1.4),
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
        //
        // Member rows render in a MONOSPACE face, so they must be measured
        // with the fixed monospace advance (not the proportional table) or
        // wide signatures like `+eat(food: String) boolean` overflow. The
        // box width is exactly `longest_line + 2*PAD_X` for every row, using
        // the geometry constants shared with the renderer.
        if !node.sections.is_empty() {
            use compartment::*;
            let mut w =
                measure_line(&node.label, TITLE_FONT) * TITLE_BOLD_FACTOR + 2.0 * PAD_X + 8.0;
            let mut h = TITLE_H;
            for section in &node.sections {
                for line in section.split('\n') {
                    w = w.max(measure_line_mono(line, MEMBER_FONT) + 2.0 * PAD_X);
                    h += LINE_H;
                }
                h += SECTION_GAP;
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

    /// L14: a compartmented (UML class) node must be wide enough for its
    /// widest MONOSPACE member row. The classic regression was measuring
    /// member rows with the proportional table while the renderer drew them
    /// monospaced, so wide signatures overflowed.
    #[test]
    fn class_node_fits_widest_monospace_row() {
        use layra_core::{Direction, Node};
        let mut g = Graph::new(Direction::TopBottom);
        let mut n = Node::new("Animal", "Animal");
        n.sections = vec!["+String name\n+makeSound() void\n+eat(food: String) boolean".into()];
        g.add_node(n);
        measure_graph(&mut g, &TextOptions::default());

        let r_w = g.nodes[0].size.width;
        // The widest row, measured in the SAME monospace metric the renderer
        // uses, plus padding on both sides, must fit inside the box.
        let widest = "+eat(food: String) boolean";
        let needed = measure_line_mono(widest, compartment::MEMBER_FONT) + 2.0 * compartment::PAD_X;
        assert!(
            r_w >= needed,
            "class box {r_w} too narrow for monospace row needing {needed}"
        );
    }

    #[test]
    fn monospace_measures_wider_than_proportional_for_narrow_chars() {
        // Narrow glyphs (i, l, |) advance their true narrow width in the
        // proportional table but a full monospace cell in a mono face, so the
        // monospace measure must be the larger of the two for such strings.
        let s = "illillillill";
        assert!(measure_line_mono(s, 12.0) > measure_line(s, 12.0));
    }
}
