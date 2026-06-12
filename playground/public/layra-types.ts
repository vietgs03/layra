/**
 * TypeScript definitions for Layra's structured layout output
 * (`layout_json`). The engine (Rust/WASM) does parse → measure → layout →
 * route; these types describe the geometry it hands back so TS consumers
 * can render with Canvas/WebGL/React/D3 with full type safety.
 */

export interface Point {
  x: number;
  y: number;
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Size {
  width: number;
  height: number;
}

export type NodeShape =
  | "rect"
  | "rounded-rect"
  | "cylinder"
  | "diamond"
  | "hexagon"
  | "circle"
  | "queue"
  | "stadium"
  | "actor"
  | "cloud";

export type ComponentRole =
  | "generic"
  | "service"
  | "database"
  | "cache"
  | "queue"
  | "gateway"
  | "client"
  | "external"
  | "storage"
  | "compute"
  | "highlight";

export type EdgeStyle = "solid" | "dashed" | "thick" | "dotted";
export type EdgeKind = "arrow" | "open" | "bidirectional";

export interface LayoutNode {
  name: string;
  label: string;
  shape: NodeShape;
  role: ComponentRole;
  /** Iconify key like `mdi:laptop`, when the node declared one. */
  icon: string | null;
  size: Size;
  /** Final position from the layout stage. */
  rect: Rect;
  /** Owning subgraph index, if any. */
  parent: number | null;
}

export interface LayoutEdge {
  /** Index into `nodes`. */
  source: number;
  target: number;
  label: string | null;
  style: EdgeStyle;
  kind: EdgeKind;
  /** Routed polyline through layout waypoints, clipped to node borders. */
  points: Point[];
  label_pos: Point | null;
}

export interface LayoutSubgraph {
  name: string;
  label: string;
  nodes: number[];
  parent: number | null;
  rect: Rect;
}

export type LayoutDirection = "TopBottom" | "LeftRight" | "BottomTop" | "RightLeft";

export interface LayoutGraph {
  direction: LayoutDirection;
  nodes: LayoutNode[];
  edges: LayoutEdge[];
  subgraphs: LayoutSubgraph[];
}

/* ---- sequence diagrams ---- */

export type SeqArrow =
  | "solid"
  | "solid-open"
  | "dashed"
  | "dashed-open"
  | "solid-cross"
  | "dashed-cross";

export interface SeqParticipant {
  name: string;
  label: string;
  is_actor: boolean;
  x: number;
  rect: Rect;
}

export interface SeqMessage {
  from: number;
  to: number;
  arrow: SeqArrow;
  text: string;
  activate: boolean;
  deactivate: boolean;
  number: number | null;
}

export type NotePosition = "left-of" | "right-of" | "over";

export interface SeqNote {
  position: NotePosition;
  anchors: [number, number | null];
  text: string;
}

export type FrameKind =
  | "loop"
  | "alt"
  | "opt"
  | "par"
  | { rect: { fill: string } };

export type SeqItem =
  | { Message: SeqMessage }
  | { Note: SeqNote }
  | { FrameStart: { kind: FrameKind; label: string } }
  | { FrameElse: { label: string } }
  | "FrameEnd"
  | { Activate: number }
  | { Deactivate: number };

export interface SequenceDiagram {
  participants: SeqParticipant[];
  items: SeqItem[];
  autonumber: boolean;
}

/* ---- top-level union returned by layout_json ---- */

export type LayoutDocument =
  | { kind: "graph"; bounds: Rect; graph: LayoutGraph }
  | { kind: "sequence"; sequence: SequenceDiagram };

/** Parse the JSON string returned by the WASM `layout_json` export. */
export function parseLayout(json: string): LayoutDocument {
  return JSON.parse(json) as LayoutDocument;
}
