//! Sequence diagram parser: Mermaid `sequenceDiagram` dialect.
//!
//! Covers the constructs found in real-world usage (validated against the
//! blog corpus): `participant X as Label`, `actor`, `autonumber`, all six
//! arrow types with `+`/`-` activation suffixes, `Note left of/right
//! of/over A,B`, `loop`/`alt`/`else`/`opt`/`par`/`rect rgb(...)`/`end`,
//! `activate`/`deactivate`, `<br/>` in any text.

use crate::ParseError;
use layra_core::{
    FrameKind, NotePosition, SeqArrow, SeqItem, SeqMessage, SeqNote, SequenceDiagram,
};

#[cfg(test)]
pub(crate) fn parse(lines: &[(usize, &str)]) -> Result<SequenceDiagram, ParseError> {
    let (d, warnings) = parse_lenient(lines);
    match warnings.into_iter().next() {
        Some(w) => Err(w),
        None => Ok(d),
    }
}

pub(crate) fn parse_lenient(lines: &[(usize, &str)]) -> (SequenceDiagram, Vec<ParseError>) {
    let mut d = SequenceDiagram::default();
    let mut counter = 0u32;
    let mut warnings = Vec::new();

    for &(ln, line) in lines {
        if let Err(e) = parse_line(&mut d, &mut counter, ln, line) {
            warnings.push(e);
        }
    }

    // Unclosed frames would silently vanish in the renderer (its frame
    // stack only draws on FrameEnd). Auto-close and tell the author.
    let opens = d
        .items
        .iter()
        .filter(|i| matches!(i, SeqItem::FrameStart { .. }))
        .count();
    let closes = d
        .items
        .iter()
        .filter(|i| matches!(i, SeqItem::FrameEnd))
        .count();
    for _ in closes..opens {
        d.items.push(SeqItem::FrameEnd);
        warnings.push(ParseError::Syntax {
            line: lines.last().map_or(0, |&(ln, _)| ln),
            message: "unclosed frame (loop/alt/opt/par/rect) — auto-closed at end".into(),
        });
    }
    (d, warnings)
}

fn parse_line(
    d: &mut SequenceDiagram,
    counter: &mut u32,
    ln: usize,
    line: &str,
) -> Result<(), ParseError> {
    if line == "autonumber" {
        d.autonumber = true;
        return Ok(());
    }

    if let Some(rest) = line
        .strip_prefix("participant ")
        .or_else(|| line.strip_prefix("actor "))
    {
        let is_actor = line.starts_with("actor ");
        let (name, label) = match rest.split_once(" as ") {
            Some((n, l)) => (n.trim(), l.trim()),
            None => (rest.trim(), rest.trim()),
        };
        let id = d.intern_participant(name);
        let p = &mut d.participants[id.index()];
        p.label = clean_text(label);
        p.is_actor = is_actor;
        return Ok(());
    }

    if let Some(rest) = line
        .strip_prefix("Note ")
        .or_else(|| line.strip_prefix("note "))
    {
        return parse_note(d, ln, rest);
    }

    if let Some(rest) = line.strip_prefix("loop") {
        d.items.push(SeqItem::FrameStart {
            kind: FrameKind::Loop,
            label: clean_text(rest.trim()),
        });
        return Ok(());
    }
    if let Some(rest) = line.strip_prefix("alt") {
        d.items.push(SeqItem::FrameStart {
            kind: FrameKind::Alt,
            label: clean_text(rest.trim()),
        });
        return Ok(());
    }
    if let Some(rest) = line.strip_prefix("opt") {
        d.items.push(SeqItem::FrameStart {
            kind: FrameKind::Opt,
            label: clean_text(rest.trim()),
        });
        return Ok(());
    }
    if let Some(rest) = line.strip_prefix("par") {
        d.items.push(SeqItem::FrameStart {
            kind: FrameKind::Par,
            label: clean_text(rest.trim()),
        });
        return Ok(());
    }
    if let Some(rest) = line.strip_prefix("else") {
        d.items.push(SeqItem::FrameElse {
            label: clean_text(rest.trim()),
        });
        return Ok(());
    }
    if let Some(rest) = line.strip_prefix("rect") {
        d.items.push(SeqItem::FrameStart {
            kind: FrameKind::Rect {
                fill: rest.trim().to_string(),
            },
            label: String::new(),
        });
        return Ok(());
    }
    if line == "end" {
        d.items.push(SeqItem::FrameEnd);
        return Ok(());
    }

    if let Some(rest) = line.strip_prefix("activate ") {
        let id = d.intern_participant(rest.trim());
        d.items.push(SeqItem::Activate(id));
        return Ok(());
    }
    if let Some(rest) = line.strip_prefix("deactivate ") {
        let id = d.intern_participant(rest.trim());
        d.items.push(SeqItem::Deactivate(id));
        return Ok(());
    }

    // Message: `A->>+B: text`
    if let Some(msg) = parse_message(d, counter, line) {
        d.items.push(SeqItem::Message(msg));
        return Ok(());
    }

    Err(ParseError::Syntax {
        line: ln,
        message: format!("cannot parse sequence statement '{line}'"),
    })
}

fn parse_note(d: &mut SequenceDiagram, ln: usize, rest: &str) -> Result<(), ParseError> {
    let (position, after) = if let Some(a) = rest.strip_prefix("left of ") {
        (NotePosition::LeftOf, a)
    } else if let Some(a) = rest.strip_prefix("right of ") {
        (NotePosition::RightOf, a)
    } else if let Some(a) = rest.strip_prefix("over ") {
        (NotePosition::Over, a)
    } else {
        return Err(ParseError::Syntax {
            line: ln,
            message: format!("expected 'left of'/'right of'/'over' after Note, got '{rest}'"),
        });
    };

    let (anchors_text, text) = after.split_once(':').ok_or_else(|| ParseError::Syntax {
        line: ln,
        message: "Note is missing ': text'".into(),
    })?;

    let mut anchor_iter = anchors_text.split(',').map(str::trim);
    let first = d.intern_participant(anchor_iter.next().unwrap_or(""));
    let second = anchor_iter.next().map(|n| d.intern_participant(n));

    d.items.push(SeqItem::Note(SeqNote {
        position,
        anchors: (first, second),
        text: clean_text(text.trim()),
    }));
    Ok(())
}

/// `A->>+B: text` — find the arrow operator outside of participant names.
fn parse_message(d: &mut SequenceDiagram, counter: &mut u32, line: &str) -> Option<SeqMessage> {
    // Arrow tokens, longest first so `-->>` wins over `->>`.
    const ARROWS: &[(&str, SeqArrow)] = &[
        ("-->>", SeqArrow::Dashed),
        ("->>", SeqArrow::Solid),
        ("--x", SeqArrow::DashedCross),
        ("-x", SeqArrow::SolidCross),
        ("-->", SeqArrow::DashedOpen),
        ("->", SeqArrow::SolidOpen),
    ];

    let (pos, token, arrow) = ARROWS
        .iter()
        .filter_map(|&(tok, ar)| line.find(tok).map(|p| (p, tok, ar)))
        .min_by_key(|&(p, tok, _)| (p, std::cmp::Reverse(tok.len())))?;

    let from_name = line[..pos].trim();
    if from_name.is_empty() {
        return None;
    }

    let mut rest = &line[pos + token.len()..];
    let mut activate = false;
    let mut deactivate = false;
    if let Some(r) = rest.strip_prefix('+') {
        activate = true;
        rest = r;
    } else if let Some(r) = rest.strip_prefix('-') {
        deactivate = true;
        rest = r;
    }

    let (to_name, text) = match rest.split_once(':') {
        Some((t, x)) => (t.trim(), x.trim()),
        None => (rest.trim(), ""),
    };
    if to_name.is_empty() {
        return None;
    }

    let from = d.intern_participant(from_name);
    let to = d.intern_participant(to_name);

    let number = if d.autonumber {
        *counter += 1;
        Some(*counter)
    } else {
        None
    };

    Some(SeqMessage {
        from,
        to,
        arrow,
        text: clean_text(text),
        activate,
        deactivate,
        number,
    })
}

/// `<br/>` → newline, other tags stripped, entities kept as-is.
fn clean_text(s: &str) -> String {
    let (text, _) = crate::sanitize_html_label(s.to_string());
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(src: &str) -> Vec<(usize, String)> {
        src.lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim().to_string()))
            .filter(|(_, l)| !l.is_empty())
            .collect()
    }

    fn parse_src(src: &str) -> SequenceDiagram {
        let owned = lines(src);
        let borrowed: Vec<(usize, &str)> = owned.iter().map(|(n, l)| (*n, l.as_str())).collect();
        parse(&borrowed).unwrap()
    }

    #[test]
    fn parses_blog_style_handshake() {
        let d = parse_src(
            "autonumber\n\
             participant C as Client\n\
             participant S as Server :443\n\
             rect rgb(219, 234, 254)\n\
             Note over C,S: TLS 1.3 — fresh handshake (1 RTT)\n\
             C->>+S: ClientHello<br/>(supported ciphers)\n\
             S-->>-C: ServerHello\n\
             end",
        );
        assert!(d.autonumber);
        assert_eq!(d.participants.len(), 2);
        assert_eq!(d.participants[1].label, "Server :443");

        // rect + note + 2 messages + end
        assert_eq!(d.items.len(), 5);
        let SeqItem::Message(m) = &d.items[2] else {
            panic!("expected message")
        };
        assert!(m.activate);
        assert_eq!(m.number, Some(1));
        assert_eq!(m.text, "ClientHello\n(supported ciphers)");

        let SeqItem::Message(m2) = &d.items[3] else {
            panic!("expected message")
        };
        assert_eq!(m2.arrow, SeqArrow::Dashed);
        assert!(m2.deactivate);
    }

    #[test]
    fn lost_message_cross() {
        let d = parse_src("C->>S: GET /resource\nS--xC: 503 Service Unavailable");
        let SeqItem::Message(m) = &d.items[1] else {
            panic!()
        };
        assert_eq!(m.arrow, SeqArrow::DashedCross);
    }

    #[test]
    fn auto_declares_participants() {
        let d = parse_src("A->>B: hi\nB-->>A: yo");
        assert_eq!(d.participants.len(), 2);
        assert_eq!(d.participants[0].name, "A");
    }
}
