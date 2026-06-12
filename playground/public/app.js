// Layra playground: editor left, pannable/zoomable preview right.
// Renders on every input event, coalesced via requestAnimationFrame.

import init, { render_lenient, load_icons } from "./pkg/layra_wasm.js";

const TEMPLATES = {
  flowchart: `flowchart LR
  laptop["{icon:mdi:laptop} Your laptop\\n192.168.1.42:51000"]:::client
  router["{icon:mdi:router-wireless} Router (NAT)\\ntranslation table"]:::highlight
  target["{icon:mdi:web} example.com\\n93.184.216.34:443"]:::external

  laptop -->|outbound| router
  router ==>|rewritten src| target
  target -.->|reply| router
  router -.->|rewritten dst| laptop
`,
  sequence: `sequenceDiagram
  autonumber
  participant C as Client
  participant S as Server

  rect rgb(219, 234, 254)
  Note over C,S: TLS 1.3 handshake (1 RTT)
  C->>+S: ClientHello
  S-->>-C: ServerHello · cert · Finished
  end
  C->>S: HTTP GET / (encrypted)
  S-->>C: HTTP 200 OK
`,
  state: `stateDiagram-v2
  direction LR
  [*] --> Closed
  Closed --> Open : failures > threshold
  Open --> HalfOpen : after cool-down
  HalfOpen --> Closed : probe succeeds
  HalfOpen --> Open : probe fails
`,
  class: `classDiagram
  direction LR
  class Animal {
    +String name
    +int age
    +eat()
    +sleep()
  }
  class Dog {
    +bark()
  }
  class Cat {
    +meow()
  }
  Animal <|-- Dog
  Animal <|-- Cat
  Dog "1" --> "*" Bone : buries
`,
  er: `erDiagram
  CUSTOMER ||--o{ ORDER : places
  ORDER ||--|{ LINE_ITEM : contains
  CUSTOMER {
    string name
    string email UK
  }
  ORDER {
    int id PK
    string status
  }
`,
  gantt: `gantt
  title Release plan
  dateFormat YYYY-MM-DD
  section Build
  Engine        :done,   eng,  2026-01-01, 30d
  Playground    :active, play, after eng, 14d
  section Ship
  Beta          :crit,   beta, after play, 7d
  Launch        :milestone, 2026-03-01, 0d
`,
  pie: `pie showData title Language share
  "Rust" : 62
  "TypeScript" : 28
  "Other" : 10
`,
  mindmap: `mindmap
  root((Layra))
    Engine
      Layout
      Routing
      Text
    Playground
      Editor
      Export
    Docs
`,
  timeline: `timeline
  title Project history
  section 2026 H1
  Jan : idea : first commit
  Feb : eleven diagram types
  section 2026 H2
  Jul : v1.0
`,
  journey: `journey
  title Deploy day
  section Build
  Write code: 5: Dev
  Fix CI: 2: Dev
  section Release
  Ship it: 4: Dev, PM
  Celebrate: 5: Team
`,
  git: `gitGraph
  commit id: "init"
  commit
  branch develop
  commit id: "feat"
  commit
  checkout main
  commit
  merge develop tag: "v1.0"
  commit
`,
};

const AUTOSAVE_KEY = "layra-playground-source";

const $ = (id) => document.getElementById(id);
const editor = $("editor");
const preview = $("preview");
const viewport = $("viewport");
const status = $("status");
const perf = $("perf");

let dark = matchMedia("(prefers-color-scheme: dark)").matches;
let lastGoodSvg = "";
let rafPending = false;

/* ---------------- pan & zoom ---------------- */

const view = { x: 40, y: 40, scale: 1 };
let userTouchedView = false; // stop auto-fit once the user pans/zooms

function applyView() {
  preview.style.transform = `translate(${view.x}px, ${view.y}px) scale(${view.scale})`;
  $("zoom-level").textContent = `${Math.round(view.scale * 100)}%`;
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

function fitToView() {
  const svgEl = preview.querySelector("svg");
  if (!svgEl) return;
  const vw = viewport.clientWidth;
  const vh = viewport.clientHeight;
  const w = svgEl.viewBox.baseVal.width || svgEl.clientWidth;
  const h = svgEl.viewBox.baseVal.height || svgEl.clientHeight;
  if (!w || !h) return;
  const scale = Math.min((vw - 64) / w, (vh - 64) / h, 2);
  view.scale = Math.max(0.1, scale);
  view.x = (vw - w * view.scale) / 2;
  view.y = (vh - h * view.scale) / 2;
  userTouchedView = false;
  applyView();
}

viewport.addEventListener("wheel", (e) => {
  e.preventDefault();
  const rect = viewport.getBoundingClientRect();
  const cx = e.clientX - rect.left;
  const cy = e.clientY - rect.top;
  if (e.ctrlKey || e.metaKey || !e.shiftKey) {
    // Wheel = zoom (the natural default for a canvas).
    zoomAt(cx, cy, e.deltaY < 0 ? 1.12 : 1 / 1.12);
  } else {
    view.x -= e.deltaX;
    view.y -= e.deltaY;
    userTouchedView = true;
    applyView();
  }
}, { passive: false });

let panState = null;
viewport.addEventListener("pointerdown", (e) => {
  if (e.button !== 0) return;
  panState = { px: e.clientX, py: e.clientY, vx: view.x, vy: view.y };
  viewport.classList.add("panning");
  viewport.setPointerCapture(e.pointerId);
});
viewport.addEventListener("pointermove", (e) => {
  if (!panState) return;
  view.x = panState.vx + (e.clientX - panState.px);
  view.y = panState.vy + (e.clientY - panState.py);
  userTouchedView = true;
  applyView();
});
viewport.addEventListener("pointerup", () => {
  panState = null;
  viewport.classList.remove("panning");
});

$("zoom-in").addEventListener("click", () =>
  zoomAt(viewport.clientWidth / 2, viewport.clientHeight / 2, 1.25));
$("zoom-out").addEventListener("click", () =>
  zoomAt(viewport.clientWidth / 2, viewport.clientHeight / 2, 1 / 1.25));
$("zoom-fit").addEventListener("click", fitToView);

/* ---------------- split pane ---------------- */

const splitter = $("splitter");
const editorPane = $("editor-pane");
splitter.addEventListener("pointerdown", (e) => {
  splitter.classList.add("dragging");
  splitter.setPointerCapture(e.pointerId);
  const move = (ev) => {
    const w = Math.min(Math.max(ev.clientX, 240), window.innerWidth - 320);
    editorPane.style.setProperty("--editor-w", `${w}px`);
    editorPane.style.width = `${w}px`;
  };
  const up = () => {
    splitter.classList.remove("dragging");
    splitter.removeEventListener("pointermove", move);
    splitter.removeEventListener("pointerup", up);
  };
  splitter.addEventListener("pointermove", move);
  splitter.addEventListener("pointerup", up);
});

/* ---------------- encode / decode share links ---------------- */

function bytesToB64url(bytes) {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}
function b64urlToBytes(b64url) {
  const b64 = b64url.replaceAll("-", "+").replaceAll("_", "/");
  return Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
}
async function encodeSource(src) {
  const raw = new TextEncoder().encode(src);
  if (typeof CompressionStream !== "undefined") {
    const stream = new Blob([raw]).stream().pipeThrough(new CompressionStream("deflate-raw"));
    const compressed = new Uint8Array(await new Response(stream).arrayBuffer());
    return "c:" + bytesToB64url(compressed);
  }
  return bytesToB64url(raw);
}
async function decodeSource(hash) {
  try {
    if (hash.startsWith("c:")) {
      const bytes = b64urlToBytes(hash.slice(2));
      const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      return await new Response(stream).text();
    }
    return new TextDecoder().decode(b64urlToBytes(hash));
  } catch {
    return null;
  }
}

/* ---------------- rendering ---------------- */

function applyTheme() {
  document.documentElement.classList.toggle("dark", dark);
}

function reportError(message) {
  status.textContent = message;
  status.className = "status err";
  const m = /line (\d+)/.exec(message);
  highlightLine(m ? Number(m[1]) : null);
}

function highlightLine(line) {
  if (line == null) {
    editor.style.backgroundImage = "";
    return;
  }
  const lh = parseFloat(getComputedStyle(editor).lineHeight);
  const pad = parseFloat(getComputedStyle(editor).paddingTop);
  const y = pad + (line - 1) * lh - editor.scrollTop;
  editor.style.backgroundImage =
    `linear-gradient(to bottom, transparent ${y}px, rgba(220,38,38,.12) ${y}px, ` +
    `rgba(220,38,38,.12) ${y + lh}px, transparent ${y + lh}px)`;
}

function doRender() {
  rafPending = false;
  const src = editor.value;
  localStorage.setItem(AUTOSAVE_KEY, src);
  markActiveTemplate(src);
  const t0 = performance.now();
  try {
    const { svg, warnings } = JSON.parse(render_lenient(src, dark));
    const dt = performance.now() - t0;
    perf.textContent = dt < 1 ? `${(dt * 1000).toFixed(0)} µs` : `${dt.toFixed(1)} ms`;
    lastGoodSvg = svg;
    swapSvg(svg);
    if (!userTouchedView) fitToView();
    if (warnings.length) {
      status.textContent = `rendered, ${warnings.length} skipped — ${warnings[0]}`;
      status.className = "status warn";
      const m = /line (\d+)/.exec(warnings[0]);
      highlightLine(m ? Number(m[1]) : null);
    } else {
      status.textContent = "ok";
      status.className = "status ok";
      highlightLine(null);
    }
  } catch (e) {
    reportError(String(e.message ?? e));
  }
}

const svgParser = new DOMParser();
function swapSvg(svgText) {
  const doc = svgParser.parseFromString(svgText, "image/svg+xml");
  if (doc.querySelector("parsererror")) {
    reportError("renderer produced invalid SVG (please report this diagram)");
    return;
  }
  const fresh = doc.documentElement;
  const current = preview.querySelector("svg");
  if (!current) {
    preview.replaceChildren(fresh);
    return;
  }
  const freshNames = new Set([...fresh.attributes].map((a) => a.name));
  for (const { name } of [...current.attributes]) {
    if (!freshNames.has(name)) current.removeAttribute(name);
  }
  for (const { name, value } of [...fresh.attributes]) {
    current.setAttribute(name, value);
  }
  current.replaceChildren(...fresh.childNodes);
}

function scheduleRender() {
  if (!rafPending) {
    rafPending = true;
    requestAnimationFrame(doRender);
  }
}

/* ---------------- templates ---------------- */

function detectType(src) {
  const head = src.trimStart().split(/\s/, 1)[0] ?? "";
  if (head.startsWith("flowchart") || head.startsWith("graph")) return "flowchart";
  if (head === "sequenceDiagram") return "sequence";
  if (head.startsWith("stateDiagram")) return "state";
  if (head.startsWith("classDiagram")) return "class";
  if (head.startsWith("erDiagram")) return "er";
  if (head === "gantt") return "gantt";
  if (head.startsWith("pie")) return "pie";
  if (head === "mindmap") return "mindmap";
  if (head === "timeline") return "timeline";
  if (head === "journey") return "journey";
  if (head.startsWith("gitGraph")) return "git";
  return null;
}

function markActiveTemplate(src) {
  const type = detectType(src);
  for (const btn of $("templates").querySelectorAll("button")) {
    btn.classList.toggle("active", btn.dataset.tpl === type);
  }
}

$("templates").addEventListener("click", (e) => {
  const tpl = e.target?.dataset?.tpl;
  if (!tpl || !TEMPLATES[tpl]) return;
  editor.value = TEMPLATES[tpl];
  userTouchedView = false;
  scheduleRender();
});

/* ---------------- export & share ---------------- */

function download(filename, blob) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = filename;
  a.click();
  URL.revokeObjectURL(a.href);
}

function exportSvg() {
  if (!lastGoodSvg) return;
  download("diagram.svg", new Blob([lastGoodSvg], { type: "image/svg+xml" }));
}

function exportPng() {
  if (!lastGoodSvg) return;
  const svgEl = preview.querySelector("svg");
  const scale = 2;
  const w = Math.max(1, Math.round(svgEl.viewBox.baseVal.width * scale));
  const h = Math.max(1, Math.round(svgEl.viewBox.baseVal.height * scale));
  let svgText = lastGoodSvg;
  if (!/<svg[^>]*\swidth=/.test(svgText)) {
    svgText = svgText.replace("<svg", `<svg width="${w / scale}" height="${h / scale}"`);
  }
  const img = new Image();
  const url = URL.createObjectURL(new Blob([svgText], { type: "image/svg+xml" }));
  img.onerror = () => {
    URL.revokeObjectURL(url);
    reportError("PNG export failed: SVG did not rasterize");
  };
  img.onload = () => {
    const canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    canvas.getContext("2d").drawImage(img, 0, 0, w, h);
    URL.revokeObjectURL(url);
    canvas.toBlob((blob) => {
      if (blob) download("diagram.png", blob);
      else reportError("PNG export failed: canvas too large");
    }, "image/png");
  };
  img.src = url;
}

async function shareLink() {
  const hash = await encodeSource(editor.value);
  const url = `${location.origin}${location.pathname}#${hash}`;
  history.replaceState(null, "", `#${hash}`);
  const btn = $("btn-share");
  const old = btn.textContent;
  try {
    await navigator.clipboard.writeText(url);
    btn.textContent = "Copied!";
  } catch {
    prompt("Copy this link:", url);
    btn.textContent = "Link ready";
  }
  setTimeout(() => (btn.textContent = old), 1200);
}

async function loadFromHash() {
  if (location.hash.length <= 1) return false;
  const src = await decodeSource(location.hash.slice(1));
  if (src == null) return false;
  editor.value = src;
  userTouchedView = false;
  scheduleRender();
  return true;
}

/* ---------------- boot ---------------- */

async function main() {
  await init();

  try {
    const res = await fetch("./icons-blog.json");
    if (res.ok) load_icons(await res.text());
  } catch (e) {
    console.warn("layra: icon pack failed to load", e);
  }

  const fromHash = await loadFromHash();
  if (!fromHash) {
    editor.value = localStorage.getItem(AUTOSAVE_KEY) ?? TEMPLATES.flowchart;
  }
  applyTheme();
  doRender();
  fitToView();

  editor.addEventListener("input", scheduleRender);
  editor.addEventListener("scroll", () => {
    if (status.classList.contains("err") || status.classList.contains("warn")) {
      const m = /line (\d+)/.exec(status.textContent);
      highlightLine(m ? Number(m[1]) : null);
    }
  });
  window.addEventListener("hashchange", loadFromHash);
  window.addEventListener("resize", () => {
    if (!userTouchedView) fitToView();
  });

  $("btn-theme").addEventListener("click", () => {
    dark = !dark;
    applyTheme();
    scheduleRender();
  });
  $("btn-share").addEventListener("click", shareLink);
  $("btn-svg").addEventListener("click", exportSvg);
  $("btn-png").addEventListener("click", exportPng);

  // Keyboard: Tab/Shift+Tab indent in editor, Escape blurs;
  // +/−/0 zoom when focus is outside the editor.
  editor.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      editor.blur();
      return;
    }
    if (e.key !== "Tab") return;
    e.preventDefault();
    const { selectionStart: s, selectionEnd: end } = editor;
    if (e.shiftKey) {
      const lineStart = editor.value.lastIndexOf("\n", s - 1) + 1;
      if (editor.value.startsWith("  ", lineStart)) {
        editor.setRangeText("", lineStart, lineStart + 2, "end");
        editor.setSelectionRange(Math.max(lineStart, s - 2), Math.max(lineStart, end - 2));
      }
    } else {
      editor.setRangeText("  ", s, end, "end");
    }
    scheduleRender();
  });

  window.addEventListener("keydown", (e) => {
    if (document.activeElement === editor) return;
    const center = [viewport.clientWidth / 2, viewport.clientHeight / 2];
    if (e.key === "+" || e.key === "=") zoomAt(...center, 1.25);
    else if (e.key === "-") zoomAt(...center, 1 / 1.25);
    else if (e.key === "0") fitToView();
    else if (e.key === "d" || e.key === "D") $("btn-theme").click();
  });
}

main();
