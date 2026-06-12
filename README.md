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

| Graph | Time |
|---|---|
| 12 nodes, 11 edges, 1 cluster | **~51 µs** |
| 500-node tree | **~1.6 ms** |

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

```bash
cargo run --release --example demo   # writes demo.svg + demo-dark.svg, prints timings
cargo test                           # 18 tests across the pipeline
```

## Roadmap

- [ ] Orthogonal edge routing (visibility graph + A* with bend penalties)
- [ ] Brandes-Köpf coordinate assignment
- [ ] Subgraph-aware layout (cluster containment constraints)
- [ ] Iconify icon rendering in nodes
- [ ] Shaped text measurement (`cosmic-text`) for CJK precision
- [ ] WASM playground at layra.dev (editor + live preview + PNG/SVG export)
- [ ] `vello`/WebGPU renderer for 10k-node interactive canvases

## License

MIT
