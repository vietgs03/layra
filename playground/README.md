# Layra Playground

The interactive playground for [Layra](https://github.com/vietgs03/layra) — a
Rust-powered, Mermaid-compatible diagram engine compiled to WebAssembly. Type a
diagram on the left, see editorial-quality SVG on the right, rendered in
milliseconds.

> Diagrams at the speed of thought.

## Features

- **Live editor** with a line-number gutter, a live diagram-type badge, and
  inline error/warning highlighting that points at the offending line.
- **Pan & zoom canvas** — scroll to zoom, drag (or hold <kbd>Space</kbd>) to pan,
  drag individual nodes to reposition them, and a bird's-eye **minimap**.
- **Shape & infra palette** — click or drag shapes, connectors, and 47 bundled
  AWS/infra icons straight onto the canvas.
- **New ▾ quick switcher** — start a fresh diagram of any type (flowchart,
  sequence, state, class, ER, gantt, pie, mindmap, git, or AWS architecture)
  from a clean starter template.
- **Examples gallery** — full, ready-made diagrams for every type.
- **Command palette** (<kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>K</kbd>) for every
  action, with fuzzy search and keyboard navigation.
- **Export** to SVG or PNG (1×/2×/4×), or copy a PNG to the clipboard.
- **Share links** — the whole diagram is compressed into the URL hash; no server.
- **Animated edges** toggle (marching-ants dashes).
- **Dark mode**, a **responsive** layout that stacks on narrow screens, and an
  accessibility pass (ARIA roles/labels, full keyboard navigation, a loading
  state while WASM initializes).

## Keyboard shortcuts

| Key | Action |
|---|---|
| <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>K</kbd> | Command palette |
| <kbd>?</kbd> | Help & shortcuts |
| <kbd>+</kbd> / <kbd>−</kbd> | Zoom in / out |
| <kbd>0</kbd> | Fit to view |
| <kbd>D</kbd> | Toggle dark mode |
| <kbd>A</kbd> | Animate edges |
| <kbd>Space</kbd> + drag | Pan the canvas |
| <kbd>Tab</kbd> / <kbd>Shift</kbd>+<kbd>Tab</kbd> (in editor) | Indent / outdent |
| <kbd>Esc</kbd> | Close any overlay |

## Syntax basics

```text
flowchart LR
  a["Process"] --> b{"Decision?"}
  b -->|yes| c(["Done"])
  b -.->|no| a
  db[("Database")]:::database
```

- Edges: `-->` arrow, `-.->` dashed, `==>` thick; add labels with `-->|text|`.
- Shapes: `["…"]` box, `("…")` rounded, `(["…"])` stadium, `{"…"}` diamond,
  `[("…")]` database, `{{"…"}}` queue, `(("…"))` circle.
- Icons: `{icon:aws:lambda}`, `{icon:mdi:web}` — the AWS/infra set is bundled;
  unknown icons are fetched on demand from the Iconify API.
- Diagram types: `flowchart`, `sequenceDiagram`, `stateDiagram-v2`,
  `classDiagram`, `erDiagram`, `gantt`, `pie`, `mindmap`, `gitGraph`.

## Run / build

The playground is a static site. Building compiles the engine to WASM with
`wasm-pack` and assembles everything into `playground/dist/`.

### Prerequisites

- A Rust toolchain (`rustup`, with the `wasm32-unknown-unknown` target)
- [`wasm-pack`](https://rustwasm.github.io/wasm-pack/)
- Python 3 (only for the simple dev server below)

### Build

```sh
source ~/.cargo/env        # if cargo isn't already on PATH
./playground/build.sh
```

This writes the WASM package into `playground/public/pkg/` and copies the
finished site to `playground/dist/`.

### Serve locally

```sh
cd playground/dist
python3 -m http.server 8951
# then open http://localhost:8951/
```

Any static file server works; the site is plain HTML/CSS/ES modules plus the
generated `pkg/` WASM bundle. A secure context (https or `localhost`) is needed
for clipboard copy and share-link features.

## Project layout

```
playground/
  build.sh            # wasm-pack build + assemble dist/
  public/
    index.html        # app shell (header, editor, preview, dialogs)
    app.js            # all playground logic (editor, canvas, export, palettes…)
    style.css         # styles + light/dark theme tokens
    pkg/              # generated WASM bundle (created by build.sh)
    infra-icons.json  # bundled AWS/infra icon glyphs for the palette
    icons-blog.json   # additional bundled icon pack loaded at boot
    layra-types.ts    # TypeScript types for the engine's JSON output
  dist/               # build output (generated; git-ignored)
```

`app.js` talks to the engine through two WASM exports:

- `render_lenient(source, dark) -> { svg, warnings }` — renders SVG and returns
  line-numbered warnings for anything it had to skip.
- `load_icons(jsonPack) -> count` — registers additional icon packs at runtime.
