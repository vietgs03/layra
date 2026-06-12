# @vietgs03/layra

Mermaid-compatible diagram renderer in Rust/WASM. Layout in microseconds,
11 diagram types, inline SVG icons, ~110KB wasm (Mermaid: ~2.5MB).

```bash
npm install @vietgs03/layra
```

```js
import { render } from "@vietgs03/layra";

const svg = await render(`flowchart LR
  a[Start] --> b{Decision}
  b -->|yes| c[Ship it]
  b -->|no| a
`);
// → standalone <svg>…</svg> string, light theme
```

Works in Node ≥ 18, Vite/webpack/esbuild bundlers, and directly in the
browser. WASM loads lazily on the first call.

## API

| Export | What it does |
|---|---|
| `render(src, {dark})` | Source → SVG string. Throws with line numbers on parse errors |
| `renderLenient(src, {dark})` | `{svg, warnings}` — skips bad lines instead of failing (great for live editors) |
| `layout(src)` | Structured geometry (node rects, edge polylines) for custom Canvas/WebGL/React renderers |
| `loadIcons(pack)` | Load Iconify-format icon packs; icons render inline in the SVG |

## Diagram types

`flowchart` · `sequenceDiagram` · `stateDiagram-v2` · `classDiagram` ·
`erDiagram` · `gantt` · `pie` · `mindmap` · `timeline` · `journey` ·
`gitGraph`

## Icons

```js
import { render, loadIcons } from "@vietgs03/layra";

// Any Iconify pack: https://icon-sets.iconify.design
const mdi = await fetch("https://api.iconify.design/mdi.json?icons=laptop,web").then(r => r.json());
await loadIcons({ icons: Object.fromEntries(
  Object.entries(mdi.icons).map(([k, v]) => [`mdi:${k}`, { ...v, width: mdi.width ?? 24, height: mdi.height ?? 24 }])
)});

await render(`flowchart LR\n  a["{icon:mdi:laptop} Laptop"] --> b["{icon:mdi:web} Web"]`);
```

Icons are inlined as real `<svg>` elements — exported files carry them with
no external fetches and no `securityLevel` workarounds.

## Links

- Playground: https://vietgs03.github.io/layra/
- Engine source & CLI binaries: https://github.com/vietgs03/layra

MIT
