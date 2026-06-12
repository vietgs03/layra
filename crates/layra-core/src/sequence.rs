//! Sequence diagram IR: participants (columns) + a linear list of items
//! (rows). Layout is deterministic — x from participant order and label
//! widths, y advances per item — so no solver is needed.

use crate::geometry::Rect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParticipantId(pub u32);

impl ParticipantId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    /// Short name used in message lines (`C` in `participant C as Client`).
    pub name: String,
    /// Display label (`Client`). Multi-line via `\n`.
    pub label: String,
    /// Actor (stick figure) vs boxed participant.
    pub is_actor: bool,
    /// Lifeline x center; filled by layout.
    pub x: f32,
    /// Header box; filled by layout.
    pub rect: Rect,
}

/// Arrow styles per Mermaid: `->>` solid arrow, `-->>` dashed arrow,
/// `-x`/`--x` cross (lost message), `->`/`-->` open line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SeqArrow {
    Solid,
    SolidOpen,
    Dashed,
    DashedOpen,
    SolidCross,
    DashedCross,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeqMessage {
    pub from: ParticipantId,
    pub to: ParticipantId,
    pub arrow: SeqArrow,
    pub text: String,
    /// `+` after the arrow: activate target.
    pub activate: bool,
    /// `-` after the arrow: deactivate source.
    pub deactivate: bool,
    /// Sequence number when `autonumber` is on; filled by the parser.
    pub number: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotePosition {
    LeftOf,
    RightOf,
    Over,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeqNote {
    pub position: NotePosition,
    /// One participant, or two for `Note over A,B`.
    pub anchors: (ParticipantId, Option<ParticipantId>),
    pub text: String,
}

/// Framed fragment kinds (`loop`, `alt`, `opt`, `par`, plain `rect`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameKind {
    Loop,
    Alt,
    Opt,
    Par,
    /// `rect rgb(...)` background block; carries its fill color.
    Rect {
        fill: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeqItem {
    Message(SeqMessage),
    Note(SeqNote),
    /// `loop label` / `alt label` / `rect rgb(..)` — opens a frame.
    FrameStart {
        kind: FrameKind,
        label: String,
    },
    /// `else label` divider inside an `alt`/`par` frame.
    FrameElse {
        label: String,
    },
    /// `end`.
    FrameEnd,
    /// Explicit `activate A` / `deactivate A`.
    Activate(ParticipantId),
    Deactivate(ParticipantId),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SequenceDiagram {
    pub participants: Vec<Participant>,
    pub items: Vec<SeqItem>,
    pub autonumber: bool,
}

impl SequenceDiagram {
    pub fn participant_by_name(&self, name: &str) -> Option<ParticipantId> {
        self.participants
            .iter()
            .position(|p| p.name == name)
            .map(|i| ParticipantId(i as u32))
    }

    /// Get-or-create: Mermaid auto-declares participants on first use.
    pub fn intern_participant(&mut self, name: &str) -> ParticipantId {
        if let Some(id) = self.participant_by_name(name) {
            return id;
        }
        let id = ParticipantId(self.participants.len() as u32);
        self.participants.push(Participant {
            name: name.to_string(),
            label: name.to_string(),
            is_actor: false,
            x: 0.0,
            rect: Rect::default(),
        });
        id
    }
}
