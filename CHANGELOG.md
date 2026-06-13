# Changelog

## v0.3.0

A major capability + polish release: AWS-architecture-grade engine and a
draw.io-class playground, built across two parallel engineering tracks.

### Engine (layout / routing / icons)
- Orthogonal A* edge routing as the default for flowcharts — clean
  axis-aligned segments with rounded corners that avoid node rects
- Edge label collision avoidance: labels never sit on non-endpoint nodes
- `&` fan-in/out groups (`a & b --> c & d`) and invisible links (`~~~`)
- 47 bundled cloud/infra icons (`{icon:aws:*}` / `{icon:infra:*}`):
  compute, storage, network, database, messaging, security
- Nesting-aware subgraph/cluster layout (correct nested pills + padding)
- Accurate upper-bound text measurement so labels never overflow nodes
- Dotted edges + animation hints; full edge style × kind matrix
- Barycenter cross-axis compaction for large-diagram quality
  (width inflation 1.7x → 1.05x, zero overlap)
- Port-aware edge attachment: edges leave/enter the side facing the
  other endpoint

### Playground (draw.io / Excalidraw class)
- Minimap with viewport rectangle + smooth programmatic zoom
- Drag palette shapes/infra icons onto the canvas
- Command palette (Cmd/Ctrl+K) + global keyboard shortcuts
- AWS/infra icon palette grouped by category with real glyphs
- Animated edges toggle (persisted)
- Diagram-type quick switcher ("New") with clean starters incl. AWS
- Export dropdown (SVG, PNG 1x/2x/4x, copy-to-clipboard) + theme toggle
- Two-column live editor: gutter, type badge, persistent split
- Share/permalink (deflate + base64url URL hash, round-trip verified)
- Onboarding empty-state + examples gallery
- Robustness + a11y: error toast with offending line & click-to-jump,
  ARIA roles, full keyboard nav, WASM loading overlay, responsive layout
- Landing polish: header pitch, Star-on-GitHub CTA, in-app Help dialog

### Agent integration (MCP / VS Code)
- MCP `list_shapes` tool (alongside `validate_diagram`, `render_diagram`)
- VS Code extension live-preview panel polish

### Engineering
- Every feature landed test-first; 35 test suites green, blog corpus
  25/25 in both themes, zero clippy warnings, versions CI-synced
- Two parallel agent tracks (engine + UI) integrated via review gates

## v0.2.0

The "everything for agents and everyone else" release: one engine, five
distribution channels, hardened by a security/robustness audit.

### Added
- **MCP server** (`layra mcp`): `validate_diagram` + `render_diagram`
  tools for Claude Code / Cursor / Zed / Cline — agents validate and fix
  their own diagram syntax before you see it (docs/AGENTS.md)
- **Watch mode** (`layra watch <dir>`): `.mmd` saved → sibling `.svg`,
  dependency-free polling, editor-agnostic
- **npm package `@vietgs03/layra`**: render/renderLenient/layout/loadIcons
  for Node ≥18 and bundlers, typed, 170KB tarball
- **VS Code extension**: live preview for `.mmd` and markdown fences,
  lenient rendering through syntax errors while typing
- **CLI** (`layra render`): stdin/stdout, `--check` CI gate, `--dark`,
  `--icons`; binaries for Linux/macOS/Windows on every release
- 4 new diagram types: mindmap, timeline, journey, gitGraph (11 total)
- Node dragging with persistent offsets, Space-pan, pannable/zoomable
  canvas, 11-template gallery in the playground
- On-demand Iconify icon fetching in the playground (200k+ icons)

### Fixed (audit)
- Version drift: CLI reported 0.1.0 under the v0.1.2 tag — versions now
  synced across cargo/npm/vsix with a CI guard + tag-match check
- MCP: `resources/list` / `prompts/list` returned -32601, which some
  clients treat as fatal — now empty-list responses
- MCP: `render_diagram` failed into non-existent directories — now
  creates parents (agents constantly write into fresh `docs/diagrams/`)
- Unclosed sequence frames (`loop` without `end`) silently disappeared —
  auto-closed with a warning
- Invalid gantt dates (`2026-13-45`) silently produced bars at the 1970
  epoch — the task is rejected with a line warning instead
- Disconnected graph components overlapped each other's clusters
- Edge labels sat on the arrow line; parallel A→B/B→A edges overlapped
- Nodes dragged outside the original viewBox were clipped invisible

### Performance
- 3.7x faster crossing minimization, Cow-based SVG escaping, integer
  coordinate formatting, spatial-grid collision pruning for A* routing
- dense-5000 full pipeline: ~16ms native; 12-node diagrams ~20µs
