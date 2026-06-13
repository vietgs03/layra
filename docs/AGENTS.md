# Layra for AI agents

AI coding assistants generate Mermaid constantly — and get the syntax
wrong constantly. Layra closes the loop: the agent **validates and
renders its own diagrams** before showing them to you.

## MCP (Claude Code, Cursor, Zed, Cline, Windsurf, ...)

Layra ships an MCP server: `layra mcp` (stdio transport, zero config).

**Claude Code:**

```bash
claude mcp add layra -- layra mcp
```

**Cursor / Cline / generic** (`mcp.json` / settings):

```json
{
  "mcpServers": {
    "layra": { "command": "layra", "args": ["mcp"] }
  }
}
```

**Zed** (`settings.json`):

```json
{
  "context_servers": {
    "layra": { "command": { "path": "layra", "args": ["mcp"] } }
  }
}
```

### Tools the agent gets

| Tool | Purpose |
|---|---|
| `validate_diagram {source}` | Parse-check; returns per-line errors. The agent's inner loop: generate → validate → fix → repeat |
| `render_diagram {source, path?, dark?}` | Write final SVG to disk (or return inline) |
| `list_shapes {}` | List the node shapes, role classes, edge styles, and icon syntax the engine supports — call this first so you only use shapes that actually render |

Typical agent flow:

```text
user: "diagram the auth flow"
agent: writes sequenceDiagram source
agent: validate_diagram → "line 7: cannot parse ..."
agent: fixes line 7
agent: validate_diagram → ok
agent: render_diagram {path: "docs/auth-flow.svg"}
agent: embeds ![auth flow](docs/auth-flow.svg) in the doc
```

The agent never ships a broken diagram, and you never debug Mermaid
syntax by hand.

## Watch mode (any editor, no MCP needed)

```bash
layra watch docs/        # every .mmd gets a sibling .svg on save
```

The agent (or you) writes `docs/architecture.mmd`; `docs/architecture.svg`
appears within ~500ms. Works with any tool that can write files.

## CI gate

```bash
layra render docs/**/*.mmd --check   # exit 1 if any diagram is broken
```

Drop it in pre-commit or CI so agent-generated diagrams can't rot.

## Install

Binaries: https://github.com/vietgs03/layra/releases (Linux/macOS/Windows)
or `cargo install --path crates/layra-cli`.
