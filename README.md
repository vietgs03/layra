# Layra

**Rust-powered diagram engine. Layout in milliseconds, not seconds.**

Layra parses a Mermaid-compatible flowchart dialect, lays it out with a
Sugiyama-framework engine, and renders editorial-quality SVG — all in pure
Rust, compiled to WASM for the browser.

```text
parse → measure text → layout → route edges → render SVG
```

## Why

Mermaid renders labels into the DOM to measure them and runs its layout in
JS. Layra measures text with font metrics (no DOM) and runs layout in native
code. Current numbers on the full pipeline (parse → SVG string):

| Graph | Time (full pipeline) |
|---|---|
| 12 nodes, 11 edges, 1 cluster | **~21 µs** |
| 500-node tree | **~1 ms** |
| 5,000-node dense graph | **~16 ms** |

Per-stage on the 5k graph: parse ~5ms, layout ~3.6ms, SVG ~5.4ms —
profiled with `cargo run --release --example bench`.

Fast enough to re-render on every keystroke.

## Crates

| Crate | Role |
|---|---|
| `layra-core` | IR: graph, geometry, style taxonomy. The contract between all stages |
| `layra-parser` | Mermaid-compatible flowchart text → IR |
| `layra-text` | DOM-free label measurement (metrics table; shaped text planned) |
| `layra-layout` | Sugiyama: cycle breaking, longest-path layering, barycenter crossing minimization, coordinate relaxation |
| `layra-router` | Edge routing: border clipping, label placement (orthogonal A* planned) |
| `layra-render-svg` | SVG with the editorial theme: role-colored borders, cluster pills, shape library |
| `layra-wasm` | `render(source, dark) -> svg` for the browser |

## Diagram types

| Type | Status | Notes |
|---|---|---|
| `flowchart` / `graph` | ✅ | shapes, roles, icons, subgraphs, A* edge routing |
| `sequenceDiagram` | ✅ | activations, autonumber, notes, frames, rect blocks |
| `stateDiagram-v2` | ✅ | pseudo-states, composite states, transitions |
| `classDiagram` | ✅ | compartments, UML markers (inheritance/composition/aggregation), cardinalities |
| `erDiagram` | ✅ | attribute blocks, crow's-foot cardinality labels |
| `pie` | ✅ | title, showData, percentage labels, legend |
| `gantt` | ✅ | sections, after-dependencies, done/active/crit, milestones, date axis |
| `mindmap` | ✅ | indentation hierarchy, Mermaid shapes, layered tree layout |
| `timeline` | ✅ | sections (colored), periods, event stacks, continuations |
| `journey` | ✅ | score curve (1-5), colored faces, actors, section bands |
| `gitGraph` | ✅ | branch lanes, fork/merge curves, commit ids + tags |

## Syntax

```text
flowchart LR
  client(["Browser"]):::client --> api["API Gateway"]:::gateway
  api --> orders["Order Service"]:::service
  orders -->|persist| db[("Postgres")]:::database
  orders -.->|events| mq{{Kafka}}
  subgraph data["Data Plane"]
    db
    mq
  end
```

- Shapes: `[rect]` `(rounded)` `([stadium])` `[(cylinder)]` `((circle))` `{diamond}` `{{hexagon}}`
- Edges: `-->` solid, `-.->` dashed (async/events), `==>` thick (hot path), `<-->` bidirectional, with `|label|` or `-- label -->`
- Roles via `:::service`, `:::database`, `:::cache`, ... — each role gets a consistent border color. Cylinder ⇒ `database` and hexagon ⇒ `queue` are inferred.
- Icons: `{icon:logos:postgresql}` inside a label (rendering WIP)

## Try it

**Playground** (zero install): https://vietgs03.github.io/layra/

**CLI**:

```bash
cargo install --path crates/layra-cli   # or grab a binary from Releases

layra render diagram.mmd                # → diagram.svg
layra render diagram.mmd -o out.svg --dark
cat diagram.mmd | layra render - -o -   # stdin → stdout
layra render docs/*.mmd --check         # CI gate: exit 1 on any parse issue
layra render d.mmd --icons mdi.json     # extra Iconify packs
```

**npm** (Node ≥18 / bundlers / browser):

```bash
npm install @vietgs03/layra
```

```js
import { render } from "@vietgs03/layra";
const svg = await render("flowchart LR\n  a --> b");
```

**VS Code**: `packages/vscode` — live preview for `.mmd` files and markdown
mermaid fences (`vsce package` to build the vsix).

**Library**:

```bash
cargo run --release --example demo   # writes demo.svg + demo-dark.svg, prints timings
cargo test                           # full pipeline test suite
```

## Roadmap

- [x] Orthogonal edge routing (localized A* with bend penalties, spatial-grid collision pruning)
- [x] Brandes-Köpf coordinate assignment (4-pass alignment, type-1 conflict marking, block compaction)
- [x] Subgraph-aware layout (cluster contiguity constraint within layers)
- [x] Iconify icon rendering in nodes (inline SVG, ID namespacing, currentColor theming)
- [ ] Shaped text measurement (`cosmic-text`) for CJK precision
- [x] WASM playground (editor + live preview + PNG/SVG export + share links) — https://vietgs03.github.io/layra/
- [ ] `vello`/WebGPU renderer for 10k-node interactive canvases

## License

MIT

## AI agents (MCP)

Layra is an MCP server — Claude Code, Cursor, Zed, and friends can
validate and render the diagrams they generate, fixing their own syntax
errors before you ever see them:

```bash
claude mcp add layra -- layra mcp
```

Tools: `validate_diagram` (per-line errors for the agent's fix loop) and
`render_diagram` (SVG to disk). Plus `layra watch docs/` for editors
without MCP and `layra render --check` as a CI gate.
See [docs/AGENTS.md](docs/AGENTS.md).

## Consuming from JS / TypeScript

Two integration levels, both from the same WASM bundle:

**1. `render(source, dark)` — SVG out (simplest).** Rust does everything:
parse → measure → layout → route → SVG string. Drop it into `innerHTML`.
`render_lenient` additionally returns `{svg, warnings}` JSON and skips
unparseable lines instead of failing.

**2. `layout_json(source)` — geometry out (most flexible).** Rust does the
expensive math (parsing, text measurement, Sugiyama layout, edge routing)
and returns the laid-out document as JSON: node rects, edge polylines,
label anchor points, cluster bounds. You render however you like — Canvas,
WebGL, React components, D3 — with `playground/public/layra-types.ts`
giving you full type safety:

```ts
import { parseLayout } from "./layra-types";
import init, { layout_json } from "./pkg/layra_wasm.js";

await init();
const doc = parseLayout(layout_json("flowchart LR\n a --> b"));
if (doc.kind === "graph") {
  for (const node of doc.graph.nodes) ctx.strokeRect(node.rect.x, node.rect.y, node.rect.width, node.rect.height);
}
```

The split is deliberate: layout dominates diagram cost (graph algorithms,
text measurement) and stays in Rust; painting is cheap and belongs to
whoever owns the UI. TypeScript definitions for the wasm exports ship via
wasm-pack (`pkg/layra_wasm.d.ts`).
