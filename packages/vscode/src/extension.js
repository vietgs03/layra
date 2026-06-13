// Layra Diagrams — VS Code extension entry point.
//
// One webview panel per diagram document, updated live on every edit.
// Rendering happens inside the webview (WASM), so the extension host
// stays free of native/wasm loading concerns and the same bundle the
// playground uses is reused verbatim.

const vscode = require("vscode");
const path = require("path");

/** @type {Map<string, vscode.WebviewPanel>} doc uri -> open panel */
const panels = new Map();

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("layra.openPreview", () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        vscode.window.showWarningMessage("Layra: no active editor");
        return;
      }
      openPreview(context, editor.document);
    }),

    // Live updates: push the current text into the matching panel.
    vscode.workspace.onDidChangeTextDocument((e) => {
      const panel = panels.get(e.document.uri.toString());
      if (panel) {
        panel.webview.postMessage({
          type: "source",
          text: extractDiagram(e.document),
        });
      }
    }),

    // Follow the active editor: when you switch to another diagram doc that
    // already has a panel, repaint it from the freshest text.
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      if (!editor) return;
      const panel = panels.get(editor.document.uri.toString());
      if (panel) {
        panel.webview.postMessage({
          type: "source",
          text: extractDiagram(editor.document),
        });
      }
    }),

    // Live theme switching: tell every open preview to re-render for the
    // new light/dark kind without reopening.
    vscode.window.onDidChangeActiveColorTheme((theme) => {
      const dark =
        theme.kind === vscode.ColorThemeKind.Dark ||
        theme.kind === vscode.ColorThemeKind.HighContrast;
      for (const panel of panels.values()) {
        panel.webview.postMessage({ type: "theme", dark });
      }
    })
  );
}

function openPreview(context, document) {
  const key = document.uri.toString();
  const existing = panels.get(key);
  if (existing) {
    existing.reveal(vscode.ViewColumn.Beside, true);
    return;
  }

  const mediaRoot = vscode.Uri.file(path.join(context.extensionPath, "media"));
  const panel = vscode.window.createWebviewPanel(
    "layraPreview",
    `Layra: ${path.basename(document.fileName)}`,
    { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true },
    { enableScripts: true, localResourceRoots: [mediaRoot] }
  );
  panels.set(key, panel);
  panel.onDidDispose(() => panels.delete(key));

  const uri = (file) =>
    panel.webview.asWebviewUri(vscode.Uri.file(path.join(context.extensionPath, "media", file)));

  const dark =
    vscode.window.activeColorTheme.kind === vscode.ColorThemeKind.Dark ||
    vscode.window.activeColorTheme.kind === vscode.ColorThemeKind.HighContrast;

  panel.webview.html = `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<style>
  :root {
    --grid: color-mix(in srgb, var(--vscode-editorForeground) 8%, transparent);
  }
  html, body { height: 100%; margin: 0; overflow: hidden;
    color: var(--vscode-editorForeground);
    font: 12px var(--vscode-font-family, sans-serif); }
  #viewport {
    position: absolute; inset: 0; overflow: hidden;
    background-color: var(--vscode-editor-background);
    background-image: radial-gradient(var(--grid) 1.1px, transparent 1.1px);
    background-size: 22px 22px;
    cursor: grab;
  }
  #viewport.panning { cursor: grabbing; }
  #root { position: absolute; transform-origin: 0 0; will-change: transform; }
  #root svg { display: block; max-width: none; overflow: visible; }
  /* Toolbar: zoom controls + fit, bottom-right, VS Code-native chrome. */
  #bar {
    position: fixed; bottom: 10px; right: 10px;
    display: flex; align-items: center; gap: 2px;
    padding: 3px; border-radius: 8px;
    background: var(--vscode-editorWidget-background);
    border: 1px solid var(--vscode-editorWidget-border, transparent);
    box-shadow: 0 2px 8px rgba(0,0,0,0.2);
  }
  #bar button {
    width: 26px; height: 26px; border: 0; border-radius: 6px;
    background: transparent; color: var(--vscode-editorForeground);
    cursor: pointer; font-size: 14px; line-height: 1;
  }
  #bar button:hover { background: var(--vscode-toolbar-hoverBackground,
    color-mix(in srgb, var(--vscode-editorForeground) 12%, transparent)); }
  #bar #fit { width: auto; padding: 0 8px; font-size: 11px; }
  #bar #pct { min-width: 38px; text-align: center; font-variant-numeric: tabular-nums;
    color: var(--vscode-descriptionForeground); }
  #err { position: fixed; bottom: 0; left: 0; right: 0; padding: 6px 12px;
         font: 12px var(--vscode-editor-font-family, monospace);
         color: var(--vscode-errorForeground);
         background: var(--vscode-inputValidation-errorBackground,
           var(--vscode-editorWidget-background));
         border-top: 1px solid var(--vscode-errorForeground);
         display: none; z-index: 10; }
</style>
</head>
<body>
<div id="viewport"><div id="root"></div></div>
<div id="bar">
  <button id="out" title="Zoom out">−</button>
  <span id="pct">100%</span>
  <button id="in" title="Zoom in">+</button>
  <button id="fit" title="Fit to view">Fit</button>
</div>
<div id="err"></div>
<script type="module">
  import init, { render_lenient } from "${uri("layra_wasm.js")}";
  const viewport = document.getElementById("viewport");
  const root = document.getElementById("root");
  const err = document.getElementById("err");
  const pct = document.getElementById("pct");
  let dark = ${dark};
  let lastSource = "";
  let pending = null;
  let userTouchedView = false;
  const view = { x: 24, y: 24, scale: 1 };

  function applyView() {
    root.style.transform = \`translate(\${view.x}px, \${view.y}px) scale(\${view.scale})\`;
    pct.textContent = Math.round(view.scale * 100) + "%";
  }
  function fitToView() {
    const svg = root.querySelector("svg");
    if (!svg) return;
    const w = svg.viewBox.baseVal.width || svg.clientWidth;
    const h = svg.viewBox.baseVal.height || svg.clientHeight;
    if (!w || !h) return;
    const s = Math.min((viewport.clientWidth - 48) / w, (viewport.clientHeight - 48) / h, 2);
    view.scale = Math.max(0.1, s);
    view.x = (viewport.clientWidth - w * view.scale) / 2;
    view.y = (viewport.clientHeight - h * view.scale) / 2;
    userTouchedView = false;
    applyView();
  }
  function zoomAt(cx, cy, factor) {
    const next = Math.min(8, Math.max(0.1, view.scale * factor));
    const k = next / view.scale;
    view.x = cx - (cx - view.x) * k;
    view.y = cy - (cy - view.y) * k;
    view.scale = next;
    userTouchedView = true;
    applyView();
  }

  // wheel = zoom, shift+wheel = pan (matches the playground feel).
  viewport.addEventListener("wheel", (e) => {
    e.preventDefault();
    const r = viewport.getBoundingClientRect();
    if (e.shiftKey) { view.x -= e.deltaX; view.y -= e.deltaY; userTouchedView = true; applyView(); }
    else zoomAt(e.clientX - r.left, e.clientY - r.top, e.deltaY < 0 ? 1.12 : 1 / 1.12);
  }, { passive: false });

  let pan = null;
  viewport.addEventListener("pointerdown", (e) => {
    pan = { px: e.clientX, py: e.clientY, vx: view.x, vy: view.y };
    viewport.classList.add("panning");
    viewport.setPointerCapture(e.pointerId);
  });
  viewport.addEventListener("pointermove", (e) => {
    if (!pan) return;
    view.x = pan.vx + (e.clientX - pan.px);
    view.y = pan.vy + (e.clientY - pan.py);
    userTouchedView = true;
    applyView();
  });
  viewport.addEventListener("pointerup", () => { pan = null; viewport.classList.remove("panning"); });

  document.getElementById("in").onclick = () => zoomAt(viewport.clientWidth / 2, viewport.clientHeight / 2, 1.25);
  document.getElementById("out").onclick = () => zoomAt(viewport.clientWidth / 2, viewport.clientHeight / 2, 1 / 1.25);
  document.getElementById("fit").onclick = fitToView;
  window.addEventListener("resize", () => { if (!userTouchedView) fitToView(); });

  function paint(text) {
    try {
      const { svg, warnings } = JSON.parse(render_lenient(text, dark));
      root.innerHTML = svg;
      if (!userTouchedView) fitToView(); else applyView();
      err.style.display = warnings.length ? "block" : "none";
      err.textContent = warnings[0] ?? "";
    } catch (ex) {
      err.style.display = "block";
      err.textContent = String(ex.message ?? ex);
    }
  }

  await init({ module_or_path: "${uri("layra_wasm_bg.wasm")}" });
  const vscode = acquireVsCodeApi();
  vscode.postMessage({ type: "ready" });

  window.addEventListener("message", (e) => {
    const msg = e.data;
    if (msg.type === "theme") {
      dark = msg.dark;
      if (lastSource) paint(lastSource);
      return;
    }
    if (msg.type !== "source") return;
    pending = msg.text;
    requestAnimationFrame(() => {
      if (pending == null) return;
      lastSource = pending;
      const text = pending;
      pending = null;
      paint(text);
    });
  });
</script>
</body>
</html>`;

  // First paint once the webview signals ready.
  panel.webview.onDidReceiveMessage((message) => {
    if (message.type === "ready") {
      panel.webview.postMessage({ type: "source", text: extractDiagram(document) });
    }
  });
}

/**
 * For .mmd files: the whole document. For markdown: the mermaid fence the
 * cursor is in, or the first fence in the file.
 */
function extractDiagram(document) {
  if (document.languageId !== "markdown") {
    return document.getText();
  }
  const text = document.getText();
  const fence = /```mermaid\s*\n([\s\S]*?)```/m.exec(text);
  return fence ? fence[1] : text;
}

function deactivate() {}

module.exports = { activate, deactivate };
