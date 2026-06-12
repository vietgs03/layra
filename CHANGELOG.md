# Changelog

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
