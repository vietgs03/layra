//! Sequence diagram layout + SVG rendering.
//!
//! Layout is two passes, both O(items):
//! 1. **Columns** — participant x positions: each column must fit its
//!    header label, the widest message label between adjacent columns, and
//!    self-message loops.
//! 2. **Rows** — single sweep down the item list; each message/note advances
//!    the y cursor by its measured height. Frames (loop/alt/rect) track
//!    their start y on a stack and close at `end`.

use crate::theme::Theme;
use crate::{escape, FONT_STACK};
use layra_core::{
    FrameKind, NotePosition, ParticipantId, SeqArrow, SeqItem, SeqMessage, SeqNote, SequenceDiagram,
};
use std::fmt::Write;

const HEADER_H: f32 = 40.0;
const ROW_GAP: f32 = 10.0;
const MSG_LINE_H: f32 = 16.0;
const NOTE_LINE_H: f32 = 15.0;
const MIN_COL_GAP: f32 = 70.0;
const FRAME_PAD: f32 = 10.0;
const ACTIVATION_W: f32 = 7.0;
/// Horizontal stagger applied to each nested activation level so concurrent
/// bars on the same lifeline read as a staircase instead of overlapping.
const ACT_STAGGER: f32 = ACTIVATION_W / 2.0;
const MARGIN: f32 = 16.0;
const SELF_LOOP_W: f32 = 46.0;

/// Attachment x for a message endpoint: the lifeline center when the
/// participant is idle, otherwise the edge of its (possibly nested)
/// activation bar facing `toward`. Level is the 0-based nesting depth of the
/// relevant bar; `None` means no active bar (attach at the lifeline).
fn bar_edge(center: f32, level: Option<usize>, toward: f32) -> f32 {
    match level {
        None => center,
        Some(l) => {
            let bx = center + l as f32 * ACT_STAGGER;
            if toward >= bx {
                bx + ACTIVATION_W / 2.0
            } else {
                bx - ACTIVATION_W / 2.0
            }
        }
    }
}

pub fn render_sequence(d: &SequenceDiagram, theme: &Theme) -> String {
    if d.participants.is_empty() {
        return r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 60" width="200" height="60"><text x="100" y="30" text-anchor="middle" font-size="13">empty sequence diagram</text></svg>"#.to_string();
    }

    let n = d.participants.len();

    // ---- Pass 1: column positions ----
    let mut col_w = vec![0.0f32; n]; // header box widths
    for (i, p) in d.participants.iter().enumerate() {
        let w = p
            .label
            .split('\n')
            .map(|l| measure(l, 13.5))
            .fold(0.0f32, f32::max)
            + 24.0;
        col_w[i] = w.max(70.0);
    }

    // Required gap between adjacent lifelines from message/note labels.
    let mut gap = vec![MIN_COL_GAP; n.saturating_sub(1)];
    for item in &d.items {
        match item {
            SeqItem::Message(m) => {
                let (a, b) = (
                    m.from.index().min(m.to.index()),
                    m.from.index().max(m.to.index()),
                );
                let label_w = m
                    .text
                    .split('\n')
                    .map(|l| measure(l, 12.5))
                    .fold(0.0f32, f32::max)
                    + 24.0
                    + if m.number.is_some() { 22.0 } else { 0.0 };
                if a == b {
                    // self message needs lateral room
                    if a < gap.len() {
                        gap[a] = gap[a].max(SELF_LOOP_W + label_w);
                    }
                    continue;
                }
                // Spread requirement across the spanned gaps.
                let span = (b - a) as f32;
                let per_gap = (label_w / span).max(MIN_COL_GAP);
                for g in gap.iter_mut().take(b).skip(a) {
                    *g = g.max(per_gap);
                }
            }
            SeqItem::Note(note) => {
                let w = note_width(note);
                let i = note.anchors.0.index();
                match (note.position, note.anchors.1) {
                    (NotePosition::Over, Some(second)) => {
                        let (a, b) = (i.min(second.index()), i.max(second.index()));
                        let span = (b - a).max(1) as f32;
                        let per_gap = ((w - 40.0) / span).max(MIN_COL_GAP);
                        for g in gap.iter_mut().take(b).skip(a) {
                            *g = g.max(per_gap);
                        }
                    }
                    (NotePosition::RightOf, _) if i < gap.len() => {
                        gap[i] = gap[i].max(w + 20.0);
                    }
                    (NotePosition::LeftOf, _) if i > 0 => {
                        gap[i - 1] = gap[i - 1].max(w + 20.0);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    let mut xs = vec![0.0f32; n];
    let mut cursor = MARGIN + col_w[0] / 2.0;
    xs[0] = cursor;
    for i in 1..n {
        cursor += (col_w[i - 1] / 2.0 + col_w[i] / 2.0).max(gap[i - 1]);
        xs[i] = cursor;
    }
    let total_w = cursor + col_w[n - 1] / 2.0 + MARGIN;

    // ---- Pass 2: rows ----
    // Pre-computed per-item y positions; frames resolved with a stack.
    let mut y = MARGIN + HEADER_H + 14.0;
    let mut body = String::with_capacity(8192);
    let mut frames: Vec<(FrameKind, String, f32)> = Vec::new();
    let mut frame_boxes: Vec<(FrameKind, String, f32, f32)> = Vec::new(); // kind, label, y0, y1
                                                                          // Active bars: (participant, y0, nesting level). Level = how many bars
                                                                          // were already open on that participant when this one started, so nested
                                                                          // activations stagger right instead of overlapping.
    let mut active: Vec<(ParticipantId, f32, usize)> = Vec::new();
    let mut activations: Vec<(usize, f32, f32, usize)> = Vec::new(); // participant, y0, y1, level

    // Deepest currently-open activation level for `pid` (None = idle).
    let level_of = |active: &[(ParticipantId, f32, usize)], pid: ParticipantId| {
        active
            .iter()
            .filter(|(p, _, _)| *p == pid)
            .map(|(_, _, l)| *l)
            .max()
    };
    // Level the next bar opened on `pid` would occupy.
    let next_level = |active: &[(ParticipantId, f32, usize)], pid: ParticipantId| {
        active.iter().filter(|(p, _, _)| *p == pid).count()
    };

    for item in &d.items {
        match item {
            SeqItem::Message(m) => {
                y += ROW_GAP;
                let lines: Vec<&str> = m.text.split('\n').collect();
                let text_h = if m.text.is_empty() {
                    0.0
                } else {
                    lines.len() as f32 * MSG_LINE_H
                };

                // Endpoint attachment x: from the edge of the sender's open
                // bar (if any) to the edge of the receiver's bar. When this
                // message activates the receiver, the arrowhead lands on the
                // NEW bar it is about to open.
                let from_c = xs[m.from.index()];
                let to_c = xs[m.to.index()];
                let from_level = level_of(&active, m.from);
                let to_level = if m.activate {
                    Some(next_level(&active, m.to))
                } else {
                    level_of(&active, m.to)
                };
                let from_x = bar_edge(from_c, from_level, to_c);
                let to_x = bar_edge(to_c, to_level, from_c);

                if m.activate {
                    let lvl = next_level(&active, m.to);
                    active.push((m.to, y + text_h + 4.0, lvl));
                }

                write_message(&mut body, m, from_x, to_x, y, &lines, theme);
                y += text_h + 14.0;

                if m.deactivate {
                    if let Some(pos) = active.iter().rposition(|(p, _, _)| *p == m.from) {
                        let (p, y0, lvl) = active.remove(pos);
                        activations.push((p.index(), y0, y, lvl));
                    }
                }
            }
            SeqItem::Note(note) => {
                y += ROW_GAP;
                let h = note.text.split('\n').count() as f32 * NOTE_LINE_H + 12.0;
                write_note(&mut body, note, &xs, &col_w, total_w, y, h, theme);
                y += h + 6.0;
            }
            SeqItem::FrameStart { kind, label } => {
                y += ROW_GAP + 4.0;
                frames.push((kind.clone(), label.clone(), y));
                // Reserve label strip height for non-rect frames.
                if !matches!(kind, FrameKind::Rect { .. }) {
                    y += 22.0;
                }
            }
            SeqItem::FrameElse { label } => {
                y += ROW_GAP;
                // Divider line across the current frame.
                if let Some((_, _, _y0)) = frames.last() {
                    let x0 = xs[0] - col_w[0] / 2.0 - FRAME_PAD;
                    let x1 = total_w - MARGIN + FRAME_PAD;
                    let _ = write!(
                        body,
                        r#"<line x1="{x0:.1}" y1="{y:.1}" x2="{x1:.1}" y2="{y:.1}" stroke="{}" stroke-dasharray="4 3" stroke-width="1"/><text x="{:.1}" y="{:.1}" font-size="11" font-style="italic" fill="{}">[{}]</text>"#,
                        theme.cluster_stroke,
                        x0 + 6.0,
                        y + 14.0,
                        theme.edge_label,
                        escape(label)
                    );
                    y += 20.0;
                }
            }
            SeqItem::FrameEnd => {
                y += 8.0;
                if let Some((kind, label, y0)) = frames.pop() {
                    frame_boxes.push((kind, label, y0, y));
                }
                y += 6.0;
            }
            SeqItem::Activate(p) => {
                let lvl = next_level(&active, *p);
                active.push((*p, y, lvl));
            }
            SeqItem::Deactivate(p) => {
                if let Some(pos) = active.iter().rposition(|(q, _, _)| q == p) {
                    let (q, y0, lvl) = active.remove(pos);
                    activations.push((q.index(), y0, y, lvl));
                }
            }
        }
    }
    // Close any dangling activations at the bottom.
    for (p, y0, lvl) in active.drain(..) {
        activations.push((p.index(), y0, y + 10.0, lvl));
    }

    let lifeline_bottom = y + 18.0;
    let total_h = lifeline_bottom + MARGIN;

    // ---- Assemble: background → frames → lifelines → activations → body → headers ----
    let mut svg = String::with_capacity(body.len() + 4096);
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total_w:.0} {total_h:.0}" width="{total_w:.0}" height="{total_h:.0}" font-family="{FONT_STACK}">"#
    );
    let _ = write!(
        svg,
        r#"<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="{}"/></marker><marker id="arrow-open" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse"><path d="M1 1L9 5L1 9" fill="none" stroke="{}" stroke-width="1.4"/></marker></defs>"#,
        theme.edge, theme.edge
    );
    let _ = write!(
        svg,
        r#"<rect x="0" y="0" width="{total_w:.0}" height="{total_h:.0}" fill="{}"/>"#,
        theme.background
    );

    // Frames (rect fills under everything else, framed boxes with labels).
    for (kind, label, y0, y1) in &frame_boxes {
        let x0 = MARGIN / 2.0;
        let w = total_w - MARGIN;
        match kind {
            FrameKind::Rect { fill } => {
                let _ = write!(
                    svg,
                    r#"<rect x="{x0:.1}" y="{y0:.1}" width="{w:.1}" height="{:.1}" rx="6" fill="{}" opacity="0.5"/>"#,
                    y1 - y0,
                    css_color(fill, theme)
                );
            }
            other => {
                let tag = match other {
                    FrameKind::Loop => "loop",
                    FrameKind::Alt => "alt",
                    FrameKind::Opt => "opt",
                    FrameKind::Par => "par",
                    FrameKind::Rect { .. } => unreachable!(),
                };
                let _ = write!(
                    svg,
                    r##"<rect x="{x0:.1}" y="{y0:.1}" width="{w:.1}" height="{:.1}" fill="none" stroke="{}" stroke-width="1.2"/><path d="M{x0:.1} {y0:.1}h54v14l-8 8H{x0:.1}z" fill="{}"/><text x="{:.1}" y="{:.1}" font-size="11" font-weight="600" fill="#fff">{tag}</text><text x="{:.1}" y="{:.1}" font-size="11" font-style="italic" fill="{}">{}</text>"##,
                    y1 - y0,
                    theme.cluster_stroke,
                    theme.cluster_title,
                    x0 + 8.0,
                    y0 + 15.0,
                    x0 + 62.0,
                    y0 + 15.0,
                    theme.edge_label,
                    escape(label)
                );
            }
        }
    }

    // Lifelines.
    let header_top = MARGIN;
    for (i, _p) in d.participants.iter().enumerate() {
        let x = xs[i];
        let _ = write!(
            svg,
            r#"<line x1="{x:.1}" y1="{:.1}" x2="{x:.1}" y2="{lifeline_bottom:.1}" stroke="{}" stroke-width="1" stroke-dasharray="4 4"/>"#,
            header_top + HEADER_H,
            theme.cluster_stroke
        );
    }

    // Activation bars. Nested bars stagger right by ACT_STAGGER per level so
    // concurrent activations on one lifeline read as a staircase.
    for (i, y0, y1, level) in &activations {
        let x = xs[*i] + *level as f32 * ACT_STAGGER;
        let _ = write!(
            svg,
            r#"<rect x="{:.1}" y="{y0:.1}" width="{ACTIVATION_W}" height="{:.1}" fill="{}" stroke="{}" stroke-width="1"/>"#,
            x - ACTIVATION_W / 2.0,
            (y1 - y0).max(6.0),
            theme.cluster_fill,
            theme.edge
        );
    }

    svg.push_str(&body);

    // Participant headers (top), drawn last so they sit above frame fills.
    for (i, p) in d.participants.iter().enumerate() {
        let w = col_w[i];
        let x = xs[i] - w / 2.0;
        let _ = write!(
            svg,
            r#"<rect x="{x:.1}" y="{header_top:.1}" width="{w:.1}" height="{HEADER_H:.1}" rx="6" fill="{}" stroke="{}" stroke-width="1.5"/>"#,
            theme.node_fill,
            theme.role_color(layra_core::ComponentRole::Service),
        );
        let lines: Vec<&str> = p.label.split('\n').collect();
        let lh = 16.0;
        let ty = header_top + HEADER_H / 2.0 - (lines.len() as f32 - 1.0) * lh / 2.0;
        for (k, line) in lines.iter().enumerate() {
            let _ = write!(
                svg,
                r#"<text x="{:.1}" y="{:.1}" font-size="13.5" font-weight="600" fill="{}" text-anchor="middle" dominant-baseline="central">{}</text>"#,
                xs[i],
                ty + k as f32 * lh,
                theme.text,
                escape(line)
            );
        }
    }

    svg.push_str("</svg>");
    svg
}

fn write_message(
    out: &mut String,
    m: &SeqMessage,
    x0: f32,
    x1: f32,
    y: f32,
    lines: &[&str],
    theme: &Theme,
) {
    let text_h = if m.text.is_empty() {
        0.0
    } else {
        lines.len() as f32 * MSG_LINE_H
    };
    let arrow_y = y + text_h + 6.0;

    let dash = match m.arrow {
        SeqArrow::Dashed | SeqArrow::DashedOpen | SeqArrow::DashedCross => {
            r#" stroke-dasharray="5 4""#
        }
        _ => "",
    };
    let marker = match m.arrow {
        SeqArrow::Solid | SeqArrow::Dashed => r#" marker-end="url(#arrow)""#,
        SeqArrow::SolidOpen | SeqArrow::DashedOpen => r#" marker-end="url(#arrow-open)""#,
        SeqArrow::SolidCross | SeqArrow::DashedCross => "",
    };

    if m.from == m.to {
        // Self message: small lasso to the right.
        let _ = write!(
            out,
            r#"<path d="M{x0:.1} {arrow_y:.1}h{SELF_LOOP_W}v14h-{SELF_LOOP_W}" fill="none" stroke="{}" stroke-width="1.4"{dash}{marker}/>"#,
            theme.edge
        );
    } else {
        let _ = write!(
            out,
            r#"<line x1="{x0:.1}" y1="{arrow_y:.1}" x2="{x1:.1}" y2="{arrow_y:.1}" stroke="{}" stroke-width="1.4"{dash}{marker}/>"#,
            theme.edge
        );
        // Lost-message cross at the target end.
        if matches!(m.arrow, SeqArrow::SolidCross | SeqArrow::DashedCross) {
            let dir = if x1 >= x0 { -1.0 } else { 1.0 };
            let cx = x1 + dir * 6.0;
            let _ = write!(
                out,
                r#"<path d="M{:.1} {:.1}l8 8M{:.1} {:.1}l-8 8" stroke="{}" stroke-width="1.6"/>"#,
                cx - 4.0,
                arrow_y - 4.0,
                cx + 4.0,
                arrow_y - 4.0,
                theme.role_color(layra_core::ComponentRole::Cache), // red-ish
            );
        }
    }

    // Label above the arrow, centered.
    let mid = if m.from == m.to {
        x0 + SELF_LOOP_W + 8.0
    } else {
        (x0 + x1) / 2.0
    };
    let anchor = if m.from == m.to { "start" } else { "middle" };
    for (i, line) in lines.iter().enumerate() {
        let mut content = String::new();
        if i == 0 {
            if let Some(num) = m.number {
                let _ = write!(
                    content,
                    r#"<tspan font-weight="700" fill="{}">{num}</tspan> "#,
                    theme.role_color(layra_core::ComponentRole::Service)
                );
            }
        }
        content.push_str(&escape(line));
        let _ = write!(
            out,
            r#"<text x="{mid:.1}" y="{:.1}" font-size="12.5" fill="{}" text-anchor="{anchor}">{content}</text>"#,
            y + (i as f32 + 0.75) * MSG_LINE_H,
            theme.text
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn write_note(
    out: &mut String,
    note: &SeqNote,
    xs: &[f32],
    col_w: &[f32],
    total_w: f32,
    y: f32,
    h: f32,
    theme: &Theme,
) {
    let w = note_width(note);
    let i = note.anchors.0.index();
    let (x0, x1) = match (note.position, note.anchors.1) {
        (NotePosition::Over, Some(b)) => {
            let j = b.index();
            let (lo, hi) = (xs[i].min(xs[j]), xs[i].max(xs[j]));
            (lo - 30.0, hi + 30.0)
        }
        (NotePosition::Over, None) => (xs[i] - w / 2.0, xs[i] + w / 2.0),
        (NotePosition::RightOf, _) => (xs[i] + 12.0, xs[i] + 12.0 + w),
        (NotePosition::LeftOf, _) => (xs[i] - 12.0 - w, xs[i] - 12.0),
    };
    let x0 = x0.max(4.0);
    let x1 = x1.min(total_w - 4.0);
    let _ = col_w;

    let _ = write!(
        out,
        r#"<rect x="{x0:.1}" y="{y:.1}" width="{:.1}" height="{h:.1}" rx="3" fill="{}" stroke="{}" stroke-width="1"/>"#,
        x1 - x0,
        theme.note_fill,
        theme.note_stroke,
    );
    for (k, line) in note.text.split('\n').enumerate() {
        let _ = write!(
            out,
            r#"<text x="{:.1}" y="{:.1}" font-size="11.5" fill="{}" text-anchor="middle">{}</text>"#,
            (x0 + x1) / 2.0,
            y + (k as f32 + 1.0) * NOTE_LINE_H - 2.0,
            theme.note_text,
            escape(line)
        );
    }
}

fn note_width(note: &SeqNote) -> f32 {
    note.text
        .split('\n')
        .map(|l| measure(l, 11.5))
        .fold(0.0f32, f32::max)
        + 24.0
}

/// Same metrics-table approach as layra-text (kept local: sequence layout
/// needs only rough widths and layra-text would be a circular dep).
fn measure(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|c| match c {
            'i' | 'j' | 'l' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' => 0.28,
            'f' | 't' | 'r' | ' ' | '(' | ')' | '/' | '\\' => 0.35,
            'm' | 'M' | 'W' | 'w' | '@' => 0.88,
            'A'..='Z' | '0'..='9' => 0.62,
            _ if (c as u32) > 0x2E80 => 1.0,
            _ => 0.52,
        })
        .sum::<f32>()
        * font_size
}

/// Pass through `rgb(...)` / hex colors; harmonize with dark theme by
/// dimming light pastel fills.
fn css_color(raw: &str, theme: &Theme) -> String {
    if theme.is_dark {
        // Blog rect fills are light pastels; swap for a translucent panel.
        return theme.cluster_fill.to_string();
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use layra_core::Document;
    use layra_parser::parse_document_lenient;

    fn seq(src: &str) -> SequenceDiagram {
        let (doc, warnings) = parse_document_lenient(src);
        assert!(warnings.is_empty(), "parse warnings: {warnings:?}");
        match doc {
            Document::Sequence(s) => s,
            other => panic!("expected sequence, got {other:?}"),
        }
    }

    /// Count `<rect width="7" ...>` activation bars and return their x lefts.
    fn activation_xs(svg: &str) -> Vec<f32> {
        svg.split("<rect")
            .filter(|s| s.contains(r#"width="7""#))
            .filter_map(|s| s.split("x=\"").nth(1)?.split('"').next()?.parse().ok())
            .collect()
    }

    #[test]
    fn activation_bars_render_from_plus_minus() {
        let d = seq("sequenceDiagram\n  A->>+B: call\n  B-->>-A: ret");
        let svg = render_sequence(&d, &Theme::light());
        // One activation bar (B), 7px wide.
        assert_eq!(activation_xs(&svg).len(), 1, "one activation bar expected");
    }

    #[test]
    fn nested_activation_bars_stagger_not_overlap() {
        // Two concurrent activations on B must not draw at the same x.
        let d = seq("sequenceDiagram\n  participant A\n  participant B\n  \
             A->>B: a\n  activate B\n  A->>B: b\n  activate B\n  \
             B-->>A: r1\n  deactivate B\n  B-->>A: r2\n  deactivate B");
        let svg = render_sequence(&d, &Theme::light());
        let xs = activation_xs(&svg);
        assert_eq!(xs.len(), 2, "two activation bars expected");
        assert!(
            (xs[0] - xs[1]).abs() >= ACT_STAGGER - 0.01,
            "nested bars must stagger by at least {ACT_STAGGER}, got {xs:?}"
        );
    }

    #[test]
    fn explicit_activate_deactivate_render() {
        let d = seq("sequenceDiagram\n  A->>B: go\n  activate B\n  B-->>A: ok\n  deactivate B");
        let svg = render_sequence(&d, &Theme::light());
        assert_eq!(activation_xs(&svg).len(), 1);
    }

    #[test]
    fn loop_alt_opt_par_frames_render_labels() {
        let d = seq("sequenceDiagram\n  A->>B: x\n  \
             loop retry\n    A->>B: y\n  end\n  \
             alt ok\n    B-->>A: 200\n  else fail\n    B-->>A: 500\n  end\n  \
             opt maybe\n    A->>B: z\n  end\n  \
             par work\n    A->>B: p\n  end");
        let svg = render_sequence(&d, &Theme::light());
        assert!(svg.contains(">loop<"), "loop frame tag");
        assert!(svg.contains(">alt<"), "alt frame tag");
        assert!(svg.contains(">opt<"), "opt frame tag");
        assert!(svg.contains(">par<"), "par frame tag");
        // alt else divider present.
        assert!(
            svg.contains(r#"stroke-dasharray="4 3""#),
            "alt else divider line"
        );
        assert!(svg.contains("[fail]"), "else label rendered");
    }

    #[test]
    fn autonumber_prefixes_messages() {
        let d = seq("sequenceDiagram\n  autonumber\n  A->>B: first\n  B-->>A: second");
        assert!(d.autonumber);
        let svg = render_sequence(&d, &Theme::light());
        // Numbers render as bold tspans, one per message.
        assert_eq!(svg.matches("<tspan").count(), 2, "two numbered messages");
        assert!(svg.contains(">1</tspan>"));
        assert!(svg.contains(">2</tspan>"));
    }

    #[test]
    fn without_autonumber_no_numbers() {
        let d = seq("sequenceDiagram\n  A->>B: hi");
        let svg = render_sequence(&d, &Theme::light());
        assert!(!svg.contains("<tspan"), "no numbering without autonumber");
    }

    #[test]
    fn message_attaches_to_activation_bar_edge() {
        // When B is active, an arrow from A to B must land on the bar's left
        // edge (center - ACTIVATION_W/2), not at the lifeline center.
        let d = seq("sequenceDiagram\n  participant A\n  participant B\n  A->>+B: call");
        let svg = render_sequence(&d, &Theme::light());
        let bar_x = activation_xs(&svg)[0]; // left edge of the 7px bar
        let bar_edge_x = bar_x; // x= of rect == left edge
                                // The message <line> x2 should equal the bar's near edge, well left
                                // of the lifeline center (bar_left + 3.5).
        let center = bar_edge_x + ACTIVATION_W / 2.0;
        let x2: f32 = svg
            .split("<line")
            .find(|s| s.contains("marker-end"))
            .and_then(|s| s.split("x2=\"").nth(1)?.split('"').next()?.parse().ok())
            .unwrap();
        assert!(
            (x2 - bar_edge_x).abs() < 0.6,
            "arrow x2 {x2} should hit bar edge {bar_edge_x} (center {center})"
        );
    }

    #[test]
    fn bar_edge_helper_picks_facing_side() {
        // Idle: attach at center.
        assert_eq!(bar_edge(100.0, None, 200.0), 100.0);
        // Active, partner to the right: attach at right edge.
        assert_eq!(bar_edge(100.0, Some(0), 200.0), 100.0 + ACTIVATION_W / 2.0);
        // Active, partner to the left: attach at left edge.
        assert_eq!(bar_edge(100.0, Some(0), 10.0), 100.0 - ACTIVATION_W / 2.0);
    }
}
