/**
 * @vietgs03/layra — Mermaid-compatible diagram renderer (Rust → WASM).
 */

export interface RenderOptions {
  /** Render with the dark theme. Default: false. */
  dark?: boolean;
}

export interface LenientResult {
  svg: string;
  /** One entry per skipped line, e.g. `line 31: cannot parse node '…'`. */
  warnings: string[];
}

/* ---- structured layout output (see layout()) ---- */

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

export interface LayoutNode {
  name: string;
  label: string;
  shape: NodeShape;
  role: ComponentRole;
  icon: string | null;
  sections: string[];
  rect: Rect;
  parent: number | null;
}

export interface LayoutEdge {
  source: number;
  target: number;
  label: string | null;
  style: "solid" | "dashed" | "thick" | "dotted";
  kind:
    | "arrow"
    | "open"
    | "bidirectional"
    | "triangle"
    | "diamond-filled"
    | "diamond-open";
  points: Point[];
  label_pos: Point | null;
  end_labels: [string, string] | null;
}

export interface LayoutGraph {
  direction: "TopBottom" | "LeftRight" | "BottomTop" | "RightLeft";
  nodes: LayoutNode[];
  edges: LayoutEdge[];
  subgraphs: { name: string; label: string; nodes: number[]; rect: Rect }[];
}

export type LayoutDocument =
  | { kind: "graph"; bounds: Rect; graph: LayoutGraph }
  | { kind: "sequence"; sequence: unknown }
  | { kind: "pie"; pie: unknown }
  | { kind: "gantt"; gantt: unknown }
  | { kind: "timeline"; timeline: unknown }
  | { kind: "journey"; journey: unknown }
  | { kind: "git"; git: unknown };

/** Render diagram source to a standalone SVG string. Throws on parse errors. */
export function render(source: string, options?: RenderOptions): Promise<string>;

/** Lenient render: skips bad lines, returns warnings alongside the SVG. */
export function renderLenient(
  source: string,
  options?: RenderOptions
): Promise<LenientResult>;

/** Parse + layout only; returns structured geometry for custom renderers. */
export function layout(source: string): Promise<LayoutDocument>;

/** Load an Iconify-format icon pack. Returns the number of icons added. */
export function loadIcons(pack: object | string): Promise<number>;
