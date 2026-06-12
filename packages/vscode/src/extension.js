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
  html, body { height: 100%; margin: 0; overflow: hidden; }
  #root { width: 100%; height: 100%; display: grid; place-items: center; overflow: auto; }
  #root svg { max-width: none; }
  #err { position: fixed; bottom: 0; left: 0; right: 0; padding: 6px 12px;
         font: 12px monospace; color: var(--vscode-errorForeground);
         background: var(--vscode-editorWidget-background); display: none; }
</style>
</head>
<body>
<div id="root"></div>
<div id="err"></div>
<script type="module">
  import init, { render_lenient } from "${uri("layra_wasm.js")}";
  const root = document.getElementById("root");
  const err = document.getElementById("err");
  const dark = ${dark};
  let pending = null;

  await init({ module_or_path: "${uri("layra_wasm_bg.wasm")}" });
  const vscode = acquireVsCodeApi();
  vscode.postMessage({ type: "ready" });

  window.addEventListener("message", (e) => {
    if (e.data.type !== "source") return;
    pending = e.data.text;
    requestAnimationFrame(() => {
      if (pending == null) return;
      const text = pending;
      pending = null;
      try {
        const { svg, warnings } = JSON.parse(render_lenient(text, dark));
        root.innerHTML = svg;
        err.style.display = warnings.length ? "block" : "none";
        err.textContent = warnings[0] ?? "";
      } catch (ex) {
        err.style.display = "block";
        err.textContent = String(ex.message ?? ex);
      }
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
