# Layra Diagrams (VS Code)

Live preview for Mermaid-compatible diagrams powered by the
[Layra](https://github.com/vietgs03/layra) Rust/WASM engine —
render in microseconds, 11 diagram types, lenient parsing that
keeps drawing while you type through syntax errors.

## Use

1. Open a `.mmd` / `.mermaid` file (or markdown with a ```mermaid fence)
2. Run **Layra: Open Diagram Preview** (command palette or editor title icon)
3. Edit — the preview updates on every keystroke

The preview is a real canvas: scroll to zoom, drag to pan, **Fit** to
recenter, and it re-themes automatically when you switch VS Code between
light and dark. Switching the active editor repaints the matching preview.

## Build from source

```bash
packages/vscode/build.sh        # bundles the WASM into media/
cd packages/vscode && npx vsce package --no-dependencies
```
