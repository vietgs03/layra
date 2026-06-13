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
  aws: `flowchart LR
  cdn["{icon:aws:cdn} CloudFront"]:::client
  gw["{icon:aws:gateway} API Gateway"]
  fn["{icon:aws:lambda} Lambda"]:::highlight
  cache[("{icon:aws:cache} ElastiCache")]:::database
  db[("{icon:aws:database} DynamoDB")]:::database
  bucket["{icon:aws:s3} S3 Bucket"]
  q{{"{icon:aws:queue} SQS Queue"}}:::queue

  cdn -->|HTTPS| gw
  gw -->|invoke| fn
  fn -->|read/write| db
  fn -.->|cache| cache
  fn -->|store| bucket
  fn ==>|enqueue| q
`,
};

const AUTOSAVE_KEY = "layra-playground-source";

const $ = (id) => document.getElementById(id);
const editor = $("editor");
const preview = $("preview");
const viewport = $("viewport");
const status = $("status");
const perf = $("perf");
const gutter = $("gutter");
const typeBadge = $("type-badge");

// Human-readable labels for each detected diagram type (badge text).
const TYPE_LABELS = {
  flowchart: "Flowchart",
  sequence: "Sequence",
  state: "State",
  class: "Class",
  er: "ER",
  gantt: "Gantt",
  pie: "Pie",
  mindmap: "Mindmap",
  timeline: "Timeline",
  journey: "Journey",
  git: "Git",
};

let dark = matchMedia("(prefers-color-scheme: dark)").matches;
let lastGoodSvg = "";
let rafPending = false;

/* ---------------- animated edges ---------------- */
// A persisted toggle that marches the dashes on the rendered edge paths via a
// CSS class on #preview (see style.css). Driven purely in the playground so it
// works regardless of any engine-side <animate> support. The class lives on
// #preview (which survives SVG swaps), so it persists across re-renders.

const ANIMATE_KEY = "layra-animate-edges";
let animateEdges = localStorage.getItem(ANIMATE_KEY) === "1";

function applyAnimateEdges() {
  preview.classList.toggle("animate-edges", animateEdges);
  const btn = $("btn-animate");
  if (btn) {
    btn.setAttribute("aria-pressed", String(animateEdges));
    btn.title = animateEdges ? "Stop animating edges (A)" : "Animate edges (A)";
  }
}

function toggleAnimateEdges() {
  animateEdges = !animateEdges;
  localStorage.setItem(ANIMATE_KEY, animateEdges ? "1" : "0");
  applyAnimateEdges();
}

/* ---------------- pan & zoom ---------------- */

const view = { x: 40, y: 40, scale: 1 };
let userTouchedView = false; // stop auto-fit once the user pans/zooms

function applyView() {
  preview.style.transform = `translate(${view.x}px, ${view.y}px) scale(${view.scale})`;
  $("zoom-level").textContent = `${Math.round(view.scale * 100)}%`;
  updateMinimapView();
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

// Animated zoom for buttons/keyboard: a brief CSS transition on the
// transform. Never used for wheel/pan (those must track input 1:1).
let zoomTransitionTimer = null;
function smoothZoom(cx, cy, factor) {
  preview.classList.add("zooming");
  zoomAt(cx, cy, factor);
  clearTimeout(zoomTransitionTimer);
  zoomTransitionTimer = setTimeout(() => preview.classList.remove("zooming"), 220);
}

function fitToView(animate = false) {
  const svgEl = preview.querySelector("svg");
  if (!svgEl) return;
  const vw = viewport.clientWidth;
  const vh = viewport.clientHeight;
  const w = svgEl.viewBox.baseVal.width || svgEl.clientWidth;
  const h = svgEl.viewBox.baseVal.height || svgEl.clientHeight;
  if (!w || !h) return;
  const scale = Math.min((vw - 64) / w, (vh - 64) / h, 2);
  if (animate) {
    preview.classList.add("zooming");
    clearTimeout(zoomTransitionTimer);
    zoomTransitionTimer = setTimeout(() => preview.classList.remove("zooming"), 220);
  }
  view.scale = Math.max(0.1, scale);
  view.x = (vw - w * view.scale) / 2;
  view.y = (vh - h * view.scale) / 2;
  userTouchedView = false;
  applyView();
}

/* ---------------- minimap ---------------- */
// A theme-aware bird's-eye view in the bottom-left of the preview pane.
// Shows the whole diagram with a viewport rectangle; click/drag jumps the
// main view. Rebuilds its thumbnail on render, tracks the rect on pan/zoom.

const minimap = $("minimap");
const minimapCanvas = $("minimap-canvas");
const minimapView = $("minimap-view");
let minimapGeom = null; // { ms, mox, moy, W, H } cached for view-rect math

function updateMinimapContent() {
  const svgEl = preview.querySelector("svg");
  if (!svgEl) {
    minimap.hidden = true;
    minimapGeom = null;
    return;
  }
  const W = svgEl.viewBox.baseVal.width || svgEl.clientWidth;
  const H = svgEl.viewBox.baseVal.height || svgEl.clientHeight;
  if (!W || !H) {
    minimap.hidden = true;
    minimapGeom = null;
    return;
  }
  // Unhide before measuring: while display:none the element has 0 size.
  minimap.hidden = false;
  const pad = 8;
  const mw = minimap.clientWidth - pad * 2;
  const mh = minimap.clientHeight - pad * 2;
  const ms = Math.min(mw / W, mh / H);
  const mox = pad + (mw - W * ms) / 2;
  const moy = pad + (mh - H * ms) / 2;
  minimapGeom = { ms, mox, moy, W, H };

  // Clone the rendered SVG; namespace its ids so its marker/gradient refs
  // never collide with the live preview's defs.
  const clone = svgEl.cloneNode(true);
  const idMap = new Map();
  for (const el of clone.querySelectorAll("[id]")) {
    const fresh = `mm-${el.id}`;
    idMap.set(el.id, fresh);
    el.id = fresh;
  }
  const refAttrs = ["href", "xlink:href", "fill", "stroke", "marker-start", "marker-end", "marker-mid", "mask", "clip-path", "filter"];
  for (const el of clone.querySelectorAll("*")) {
    for (const attr of refAttrs) {
      const v = el.getAttribute(attr);
      if (!v) continue;
      const m = /^url\(#(.+?)\)$|^#(.+)$/.exec(v.trim());
      if (m) {
        const old = m[1] ?? m[2];
        if (idMap.has(old)) {
          el.setAttribute(attr, v.startsWith("url(") ? `url(#${idMap.get(old)})` : `#${idMap.get(old)}`);
        }
      }
    }
  }
  clone.removeAttribute("style");
  clone.setAttribute("width", `${W * ms}`);
  clone.setAttribute("height", `${H * ms}`);
  clone.style.left = `${mox}px`;
  clone.style.top = `${moy}px`;
  minimapCanvas.replaceChildren(clone);
  minimap.hidden = false;
  updateMinimapView();
}

function updateMinimapView() {
  if (!minimapGeom || minimap.hidden) return;
  const { ms, mox, moy, W, H } = minimapGeom;
  const vw = viewport.clientWidth;
  const vh = viewport.clientHeight;
  // World (SVG-unit) region currently visible in the main viewport.
  const worldLeft = -view.x / view.scale;
  const worldTop = -view.y / view.scale;
  const worldW = vw / view.scale;
  const worldH = vh / view.scale;
  // Clamp to the diagram bounds so the rect stays meaningful.
  const l = Math.max(0, worldLeft);
  const t = Math.max(0, worldTop);
  const r = Math.min(W, worldLeft + worldW);
  const b = Math.min(H, worldTop + worldH);
  minimapView.style.left = `${mox + l * ms}px`;
  minimapView.style.top = `${moy + t * ms}px`;
  minimapView.style.width = `${Math.max(2, (r - l) * ms)}px`;
  minimapView.style.height = `${Math.max(2, (b - t) * ms)}px`;
}

// Click/drag on the minimap recenters the main view on that diagram point.
function minimapJump(e) {
  if (!minimapGeom) return;
  const { ms, mox, moy } = minimapGeom;
  const rect = minimap.getBoundingClientRect();
  const sx = (e.clientX - rect.left - mox) / ms; // diagram x
  const sy = (e.clientY - rect.top - moy) / ms;  // diagram y
  // Center the main viewport on (sx, sy).
  view.x = viewport.clientWidth / 2 - sx * view.scale;
  view.y = viewport.clientHeight / 2 - sy * view.scale;
  userTouchedView = true;
  applyView();
}

minimap.addEventListener("pointerdown", (e) => {
  e.preventDefault();
  e.stopPropagation();
  minimap.classList.add("dragging");
  minimap.setPointerCapture(e.pointerId);
  minimapJump(e);
});
minimap.addEventListener("pointermove", (e) => {
  if (minimap.hasPointerCapture(e.pointerId)) minimapJump(e);
});
minimap.addEventListener("pointerup", (e) => {
  minimap.classList.remove("dragging");
  if (minimap.hasPointerCapture(e.pointerId)) minimap.releasePointerCapture(e.pointerId);
});

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
let dragState = null; // node being dragged
// Manual node offsets keyed by node name; survive re-renders and apply
// to both the node group and its connected edges (which fade while
// detached from their routed path).
let nodeOffsets = new Map();
let spaceHeld = false;

function applyOffsets() {
  const svg = preview.querySelector("svg");
  if (!svg) return;
  // index -> name lookup for edges
  const nameOf = {};
  for (const g of svg.querySelectorAll("[data-node]")) {
    nameOf[g.dataset.node] = g.dataset.name;
    const off = nodeOffsets.get(g.dataset.name);
    g.setAttribute("transform", off ? `translate(${off.x} ${off.y})` : "");
  }
  for (const g of svg.querySelectorAll("[data-edge]")) {
    const a = nodeOffsets.get(nameOf[g.dataset.from]);
    const bOff = nodeOffsets.get(nameOf[g.dataset.to]);
    if (!a && !bOff) {
      g.setAttribute("transform", "");
      g.style.opacity = "";
      continue;
    }
    // Both ends moved by the same delta: translate the edge along.
    if (a && bOff && Math.abs(a.x - bOff.x) < 0.5 && Math.abs(a.y - bOff.y) < 0.5) {
      g.setAttribute("transform", `translate(${a.x} ${a.y})`);
      g.style.opacity = "";
    } else {
      // Ends diverged: keep the edge but dim it so it's clearly stale.
      g.setAttribute("transform", "");
      g.style.opacity = "0.35";
    }
  }
}

viewport.addEventListener("pointerdown", (e) => {
  const isPanButton = e.button === 1; // middle mouse always pans
  if (e.button !== 0 && !isPanButton) return;

  const nodeGroup = e.target.closest?.("[data-node]");
  // Space-drag or middle-drag pans from anywhere, draw.io-style;
  // plain left-drag on a node moves the node.
  if (nodeGroup && !spaceHeld && !isPanButton) {
    const name = nodeGroup.dataset.name;
    const off = nodeOffsets.get(name) ?? { x: 0, y: 0 };
    dragState = { name, sx: e.clientX, sy: e.clientY, ox: off.x, oy: off.y };
    viewport.setPointerCapture(e.pointerId);
    e.preventDefault();
    return;
  }
  panState = { px: e.clientX, py: e.clientY, vx: view.x, vy: view.y };
  viewport.classList.add("panning");
  viewport.setPointerCapture(e.pointerId);
  e.preventDefault();
});
viewport.addEventListener("pointermove", (e) => {
  if (dragState) {
    nodeOffsets.set(dragState.name, {
      x: dragState.ox + (e.clientX - dragState.sx) / view.scale,
      y: dragState.oy + (e.clientY - dragState.sy) / view.scale,
    });
    applyOffsets();
    return;
  }
  if (!panState) return;
  view.x = panState.vx + (e.clientX - panState.px);
  view.y = panState.vy + (e.clientY - panState.py);
  userTouchedView = true;
  applyView();
});
viewport.addEventListener("pointerup", () => {
  dragState = null;
  panState = null;
  viewport.classList.remove("panning");
});
// Space bar = pan mode (grab cursor over nodes too).
window.addEventListener("keydown", (e) => {
  if (e.code === "Space" && document.activeElement !== editor) {
    spaceHeld = true;
    viewport.classList.add("pan-mode");
    e.preventDefault();
  }
});
window.addEventListener("keyup", (e) => {
  if (e.code === "Space") {
    spaceHeld = false;
    viewport.classList.remove("pan-mode");
  }
});

$("zoom-in").addEventListener("click", () =>
  smoothZoom(viewport.clientWidth / 2, viewport.clientHeight / 2, 1.25));
$("zoom-out").addEventListener("click", () =>
  smoothZoom(viewport.clientWidth / 2, viewport.clientHeight / 2, 1 / 1.25));
$("zoom-fit").addEventListener("click", () => fitToView(true));

/* ---------------- split pane ---------------- */
// The editor/preview divider persists its ratio (editor width / window width)
// in localStorage so the workspace layout survives reloads and adapts to
// window resizes.

const splitter = $("splitter");
const editorPane = $("editor-pane");
const SPLIT_KEY = "layra-split-ratio";

function applySplitRatio(ratio) {
  const w = Math.min(Math.max(ratio * window.innerWidth, 240), window.innerWidth - 320);
  editorPane.style.width = `${w}px`;
}

// Restore the saved ratio (clamped). Falls back to the CSS default (44%).
const savedRatio = parseFloat(localStorage.getItem(SPLIT_KEY));
if (Number.isFinite(savedRatio) && savedRatio > 0.1 && savedRatio < 0.9) {
  applySplitRatio(savedRatio);
}

splitter.addEventListener("pointerdown", (e) => {
  splitter.classList.add("dragging");
  splitter.setPointerCapture(e.pointerId);
  const move = (ev) => {
    const w = Math.min(Math.max(ev.clientX, 240), window.innerWidth - 320);
    editorPane.style.width = `${w}px`;
  };
  const up = () => {
    splitter.classList.remove("dragging");
    splitter.removeEventListener("pointermove", move);
    splitter.removeEventListener("pointerup", up);
    // Persist as a window-relative ratio so it adapts when resized.
    const ratio = editorPane.getBoundingClientRect().width / window.innerWidth;
    localStorage.setItem(SPLIT_KEY, ratio.toFixed(4));
    if (!userTouchedView) fitToView();
    else updateMinimapView();
  };
  splitter.addEventListener("pointermove", move);
  splitter.addEventListener("pointerup", up);
});

// Keep the editor width proportional to the window on resize.
window.addEventListener("resize", () => {
  const r = parseFloat(localStorage.getItem(SPLIT_KEY));
  if (Number.isFinite(r) && r > 0.1 && r < 0.9) applySplitRatio(r);
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
  const btn = $("btn-theme");
  if (btn) {
    btn.setAttribute("aria-pressed", String(dark));
    btn.title = dark ? "Switch to light mode (D)" : "Switch to dark mode (D)";
  }
}

// Flip the theme and re-render (icons/colours are theme-aware). Shared by the
// header toggle, the "D" shortcut, and the command palette.
function toggleTheme() {
  dark = !dark;
  applyTheme();
  scheduleRender();
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
    activeGutterLine = null;
    markGutterActive();
    return;
  }
  const lh = parseFloat(getComputedStyle(editor).lineHeight);
  const pad = parseFloat(getComputedStyle(editor).paddingTop);
  const y = pad + (line - 1) * lh - editor.scrollTop;
  editor.style.backgroundImage =
    `linear-gradient(to bottom, transparent ${y}px, rgba(220,38,38,.12) ${y}px, ` +
    `rgba(220,38,38,.12) ${y + lh}px, transparent ${y + lh}px)`;
  activeGutterLine = line;
  markGutterActive();
}

/* ---------------- line-number gutter ---------------- */
// A lightweight gutter rendered alongside the textarea. Line count is kept in
// sync on every render; scroll position is mirrored via transform; the active
// (error/warning) line is highlighted.

let gutterCount = 0;
let activeGutterLine = null;

function updateGutter() {
  const lines = editor.value.split("\n").length;
  if (lines !== gutterCount) {
    gutterCount = lines;
    let html = "";
    for (let i = 1; i <= lines; i++) html += `<div class="gl" data-ln="${i}">${i}</div>`;
    gutter.innerHTML = html;
    markGutterActive();
  }
}

function markGutterActive() {
  const prev = gutter.querySelector(".gl.active");
  if (prev) prev.classList.remove("active");
  if (activeGutterLine != null) {
    gutter.querySelector(`.gl[data-ln="${activeGutterLine}"]`)?.classList.add("active");
  }
}

// Mirror the textarea's vertical scroll onto the gutter (translate, no reflow).
function syncGutterScroll() {
  gutter.scrollTop = editor.scrollTop;
}

// Update the live diagram-type badge from the detected type.
function updateTypeBadge(type) {
  const label = type ? TYPE_LABELS[type] ?? type : "—";
  typeBadge.textContent = label;
  typeBadge.dataset.type = type ?? "";
}

// A tasteful onboarding placeholder shown in the preview when the editor is
// empty. Anchored to the viewport (not #preview, which sizes to its content)
// so it stays centred. Invites the user to open the gallery or start typing.
function showEmptyState() {
  preview.replaceChildren();
  let el = $("empty-state");
  if (!el) {
    el = document.createElement("div");
    el.id = "empty-state";
    el.className = "empty-state";
    el.innerHTML =
      `<div class="empty-glyph">◆</div>` +
      `<h2>Diagrams at the speed of thought</h2>` +
      `<p>Start typing in the editor, or load a ready-made example.</p>` +
      `<button type="button" class="empty-cta" id="empty-cta">Browse examples</button>` +
      `<p class="empty-hint">Tip: drag shapes &amp; infra icons from the left palette.</p>`;
    viewport.appendChild(el);
    $("empty-cta").addEventListener("click", openGallery);
  }
  el.hidden = false;
}

// Remove the onboarding placeholder once real content renders.
function hideEmptyState() {
  $("empty-state")?.remove();
}

let lastType = null;
function doRender() {
  rafPending = false;
  const src = editor.value;
  const type = detectType(src);
  if (type !== lastType) {
    lastType = type;
    nodeOffsets = new Map();
  }
  updateGutter();
  updateTypeBadge(type);
  localStorage.setItem(AUTOSAVE_KEY, src);
  markActiveTemplate(src);

  // Empty editor: show a tasteful onboarding placeholder instead of an error.
  if (!src.trim()) {
    showEmptyState();
    perf.textContent = "—";
    status.textContent = "empty — pick an example or start typing";
    status.className = "status ok";
    highlightLine(null);
    lastGoodSvg = "";
    minimap.hidden = true;
    return;
  }

  const t0 = performance.now();
  try {
    const { svg, warnings } = JSON.parse(render_lenient(src, dark));
    const dt = performance.now() - t0;
    perf.textContent = dt < 1 ? `${(dt * 1000).toFixed(0)} µs` : `${dt.toFixed(1)} ms`;
    lastGoodSvg = svg;
    hideEmptyState();
    swapSvg(svg);
    applyOffsets();
    if (!userTouchedView) fitToView();
    updateMinimapContent();
    resolveMissingIcons(src);
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

/* On-demand icon resolution: when the source references an icon that the
   local pack doesn't have, fetch it from the public Iconify API once and
   re-render. Failures are cached so we never refetch a bad name. */
const iconAttempts = new Set();
async function resolveMissingIcons(src) {
  const wanted = new Set();
  for (const m of src.matchAll(/\{icon:([a-z0-9-]+):([a-z0-9-]+)\}/g)) {
    wanted.add(`${m[1]}:${m[2]}`);
  }
  for (const m of src.matchAll(/\/icons\/([a-z0-9]+)-([a-z0-9-]+)\.svg/g)) {
    wanted.add(`${m[1]}:${m[2]}`);
  }
  const missing = [...wanted].filter((k) => !iconAttempts.has(k));
  if (!missing.length) return;
  missing.forEach((k) => iconAttempts.add(k));

  // Group by prefix: one API call per pack.
  const byPrefix = new Map();
  for (const key of missing) {
    const [prefix, name] = key.split(":");
    (byPrefix.get(prefix) ?? byPrefix.set(prefix, []).get(prefix)).push(name);
  }
  let added = 0;
  await Promise.all([...byPrefix].map(async ([prefix, names]) => {
    try {
      const res = await fetch(
        `https://api.iconify.design/${prefix}.json?icons=${names.join(",")}`);
      if (!res.ok) return;
      const data = await res.json();
      if (!data?.icons) return;
      const pack = { icons: {} };
      for (const [name, icon] of Object.entries(data.icons)) {
        pack.icons[`${prefix}:${name}`] = {
          body: icon.body,
          width: icon.width ?? data.width ?? 16,
          height: icon.height ?? data.height ?? 16,
        };
      }
      added += load_icons(JSON.stringify(pack));
    } catch { /* offline or unknown pack: fall back to text-only nodes */ }
  }));
  if (added > 0) scheduleRender();
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
  // Reflect the live diagram type on the "New" trigger so the switcher always
  // shows what kind of diagram you're editing.
  const type = detectType(src);
  const label = type ? (TYPE_LABELS[type] ?? type) : null;
  const btn = $("btn-new");
  if (!btn) return;
  const sub = btn.querySelector(".new-current");
  if (sub) sub.textContent = label ? ` · ${label}` : "";
  for (const item of newList.querySelectorAll("[data-tpl]")) {
    item.classList.toggle("active", item.dataset.tpl === type);
  }
}

/* ---------------- diagram-type quick switcher ("New") ---------------- */
// Clean, minimal starter templates — one per diagram type plus "AWS
// architecture" (built from the bundled {icon:aws:*} infra icons). These are
// intentionally smaller than the rich examples gallery: a fast "new diagram of
// type X" you can build on, not a showcase.

const STARTERS = {
  flowchart: `flowchart LR
  start(["Start"]) --> step["Do something"]
  step --> decision{"OK?"}
  decision -->|yes| done(["Done"])
  decision -->|no| step
`,
  sequence: `sequenceDiagram
  participant A as Alice
  participant B as Bob
  A->>B: Hello Bob
  B-->>A: Hi Alice
`,
  state: `stateDiagram-v2
  [*] --> Idle
  Idle --> Running : start
  Running --> Idle : stop
  Running --> [*] : done
`,
  class: `classDiagram
  class Shape {
    +area() float
  }
  class Circle {
    +radius float
  }
  Shape <|-- Circle
`,
  er: `erDiagram
  USER ||--o{ POST : writes
  USER {
    int id PK
    string name
  }
  POST {
    int id PK
    string title
  }
`,
  gantt: `gantt
  title New plan
  dateFormat YYYY-MM-DD
  section Phase 1
  Task A :a1, 2026-01-01, 7d
  Task B :after a1, 5d
`,
  pie: `pie title Distribution
  "A" : 40
  "B" : 35
  "C" : 25
`,
  mindmap: `mindmap
  root((Idea))
    Branch 1
      Leaf
    Branch 2
`,
  gitGraph: `gitGraph
  commit
  branch feature
  commit
  checkout main
  merge feature
`,
  aws: `flowchart LR
  user["{icon:aws:cdn} CloudFront"]:::client
  api["{icon:aws:gateway} API Gateway"]
  fn["{icon:aws:lambda} Lambda"]:::highlight
  db[("{icon:aws:database} DynamoDB")]:::database

  user --> api
  api --> fn
  fn --> db
`,
};

// Display order, icons and one-word descriptors for the dropdown. "AWS
// architecture" is featured at the top; the rest follow in a logical order.
const STARTER_ITEMS = [
  { key: "aws", label: "AWS architecture", ic: "☁", sub: "infra icons", featured: true },
  { key: "_sep" },
  { key: "flowchart", label: "Flowchart", ic: "▱", sub: "boxes & arrows" },
  { key: "sequence", label: "Sequence", ic: "⇄", sub: "messages" },
  { key: "state", label: "State machine", ic: "◉", sub: "states" },
  { key: "class", label: "Class", ic: "◰", sub: "types" },
  { key: "er", label: "Entity-relationship", ic: "▤", sub: "tables" },
  { key: "gantt", label: "Gantt", ic: "▦", sub: "timeline" },
  { key: "pie", label: "Pie", ic: "◔", sub: "shares" },
  { key: "mindmap", label: "Mindmap", ic: "✸", sub: "ideas" },
  { key: "gitGraph", label: "Git graph", ic: "⎇", sub: "branches" },
];

const newMenu = $("new-menu");
const newTrigger = $("btn-new");
const newList = $("new-list");

// Map a starter key to the detected-type key used for the active highlight.
const STARTER_TYPE = { gitGraph: "git", aws: "flowchart" };

function buildNewMenu() {
  const frag = document.createDocumentFragment();
  for (const it of STARTER_ITEMS) {
    if (it.key === "_sep") {
      const sep = document.createElement("div");
      sep.className = "menu-sep";
      frag.appendChild(sep);
      continue;
    }
    const btn = document.createElement("button");
    btn.type = "button";
    btn.setAttribute("role", "menuitem");
    btn.dataset.starter = it.key;
    btn.dataset.tpl = STARTER_TYPE[it.key] ?? it.key;
    if (it.featured) btn.classList.add("new-featured");
    btn.innerHTML =
      `<span class="new-ic" aria-hidden="true">${it.ic}</span>` +
      `<span class="new-label">${it.label}</span>` +
      `<span class="new-sub">${it.sub}</span>`;
    frag.appendChild(btn);
  }
  newList.replaceChildren(frag);
}

function loadStarter(key) {
  const src = STARTERS[key];
  if (!src) return;
  editor.value = src;
  nodeOffsets = new Map();
  userTouchedView = false;
  // Put the caret at the end so the user can keep typing.
  editor.focus();
  editor.setSelectionRange(src.length, src.length);
  scheduleRender();
  fitToView();
}

// Keyboard-accessible "New" dropdown: mirrors the Export menu pattern
// (Enter/Space/ArrowDown opens, arrows move, Esc/Tab closes, click-outside).
function setupNewMenu() {
  buildNewMenu();
  const items = () => [...newList.querySelectorAll("[role=menuitem]")];

  const open = () => {
    newList.hidden = false;
    newTrigger.setAttribute("aria-expanded", "true");
    items()[0]?.focus();
  };
  const close = (focusTrigger = false) => {
    newList.hidden = true;
    newTrigger.setAttribute("aria-expanded", "false");
    if (focusTrigger) newTrigger.focus();
  };
  const toggle = () => (newList.hidden ? open() : close());

  newTrigger.addEventListener("click", (e) => { e.stopPropagation(); toggle(); });
  newTrigger.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      open();
    }
  });

  newList.addEventListener("click", (e) => {
    const item = e.target.closest("[data-starter]");
    if (!item) return;
    loadStarter(item.dataset.starter);
    close(true);
  });
  newList.addEventListener("keydown", (e) => {
    const list = items();
    const idx = list.indexOf(document.activeElement);
    if (e.key === "ArrowDown") { e.preventDefault(); list[(idx + 1) % list.length].focus(); }
    else if (e.key === "ArrowUp") { e.preventDefault(); list[(idx - 1 + list.length) % list.length].focus(); }
    else if (e.key === "Home") { e.preventDefault(); list[0].focus(); }
    else if (e.key === "End") { e.preventDefault(); list[list.length - 1].focus(); }
    else if (e.key === "Escape") { e.preventDefault(); close(true); }
    else if (e.key === "Tab") close();
  });

  document.addEventListener("click", (e) => {
    if (!newList.hidden && !e.target.closest("#new-menu")) close();
  });
}

/* ---------------- examples gallery / onboarding ---------------- */
// A tasteful gallery of starter diagrams (one per diagram type plus an AWS
// architecture example built from the bundled infra icons). Shown on first
// load / when the editor is empty, or any time via the Examples button.
// Each card renders a live SVG thumbnail through the wasm renderer.

const GALLERY_SEEN_KEY = "layra-gallery-seen";

// Display order + titles/descriptions for the gallery cards.
const EXAMPLES = [
  { key: "aws", title: "AWS architecture", desc: "Serverless stack with infra icons" },
  { key: "flowchart", title: "Flowchart", desc: "Boxes, arrows & decisions" },
  { key: "sequence", title: "Sequence", desc: "Actors exchanging messages" },
  { key: "state", title: "State machine", desc: "States & transitions" },
  { key: "class", title: "Class diagram", desc: "Types & relationships" },
  { key: "er", title: "Entity-relationship", desc: "Tables & cardinality" },
  { key: "gantt", title: "Gantt chart", desc: "Tasks on a timeline" },
  { key: "pie", title: "Pie chart", desc: "Proportional shares" },
  { key: "mindmap", title: "Mindmap", desc: "Radial idea tree" },
  { key: "timeline", title: "Timeline", desc: "Events by period" },
  { key: "journey", title: "User journey", desc: "Steps with sentiment" },
  { key: "git", title: "Git graph", desc: "Branches, commits & merges" },
];

const gallery = $("gallery");
const galleryGrid = $("gallery-grid");
let galleryBuilt = false;

// Render a compact thumbnail SVG (sized to fit the card) for an example.
function thumbSvg(src) {
  try {
    const { svg } = JSON.parse(render_lenient(src, dark));
    const doc = svgParser.parseFromString(svg, "image/svg+xml");
    if (doc.querySelector("parsererror")) return null;
    const el = doc.documentElement;
    // Let CSS size it; preserve aspect via the existing viewBox.
    el.removeAttribute("width");
    el.removeAttribute("height");
    el.setAttribute("preserveAspectRatio", "xMidYMid meet");
    return el;
  } catch {
    return null;
  }
}

function buildGallery() {
  if (galleryBuilt) return;
  galleryBuilt = true;
  const frag = document.createDocumentFragment();
  for (const ex of EXAMPLES) {
    const src = TEMPLATES[ex.key];
    if (!src) continue;
    const card = document.createElement("button");
    card.type = "button";
    card.className = "gallery-card";
    card.dataset.example = ex.key;
    if (ex.key === "aws") card.classList.add("featured");

    const thumb = document.createElement("div");
    thumb.className = "gallery-thumb";
    const svg = thumbSvg(src);
    if (svg) thumb.appendChild(svg);

    const meta = document.createElement("div");
    meta.className = "gallery-meta";
    meta.innerHTML =
      `<span class="gallery-card-title">${ex.title}</span>` +
      `<span class="gallery-card-desc">${ex.desc}</span>`;

    card.append(thumb, meta);
    frag.appendChild(card);
  }
  galleryGrid.replaceChildren(frag);
}

function openGallery() {
  buildGallery();
  gallery.hidden = false;
  // Focus the close button for keyboard users.
  requestAnimationFrame(() => $("gallery-close").focus());
}

function closeGallery() {
  gallery.hidden = true;
  localStorage.setItem(GALLERY_SEEN_KEY, "1");
}

function loadExample(key) {
  const src = TEMPLATES[key];
  if (!src) return;
  editor.value = src;
  nodeOffsets = new Map();
  userTouchedView = false;
  closeGallery();
  scheduleRender();
  fitToView();
}

galleryGrid.addEventListener("click", (e) => {
  const card = e.target.closest("[data-example]");
  if (card) loadExample(card.dataset.example);
});
$("btn-examples").addEventListener("click", openGallery);
$("gallery-close").addEventListener("click", closeGallery);
$("gallery").querySelector(".gallery-backdrop").addEventListener("click", closeGallery);
$("gallery-blank").addEventListener("click", () => {
  editor.value = "";
  nodeOffsets = new Map();
  closeGallery();
  editor.focus();
  scheduleRender();
});
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !gallery.hidden) {
    e.preventDefault();
    closeGallery();
  }
});

/* ---------------- shape / snippet palette ---------------- */

// Each snippet is inserted at the caret on its own line(s). `$` marks where
// the caret should land after insertion (so you can type the label right
// away); a trailing newline is added when the snippet defines a block.
const SNIPPETS = {
  rect:     `node["$Label"]`,
  rounded:  `node("$Label")`,
  stadium:  `node(["$Label"])`,
  decision: `decision{"$Is it ready?"}`,
  database: `db[("$Database")]:::database`,
  queue:    `queue{{"$Queue"}}:::queue`,
  circle:   `node(("$Label"))`,
  subgraph: `subgraph cluster["$Group"]\n  a --> b\nend`,
  arrow:    `a --> b$`,
  labeled:  `a -->|$label| b`,
  dashed:   `a -.->|$async| b`,
  thick:    `a ==>|$hot path| b`,
};

// Insert a snippet on its own fresh line directly below the caret's line,
// inheriting that line's indentation so it lands inside subgraphs etc.
// Inserting at end-of-line (never mid-token) means snippets compose cleanly
// even when clicked back-to-back. The `$` sentinel sets the caret so you can
// type the label immediately; otherwise the caret lands after the text.
function insertSnippet(key) {
  const raw = SNIPPETS[key];
  if (raw == null) return;
  const caretMark = raw.indexOf("$");
  const text = raw.replace("$", "");

  const value = editor.value;
  const caret = editor.selectionStart;
  // End of the line the caret is on.
  let lineEnd = value.indexOf("\n", caret);
  if (lineEnd === -1) lineEnd = value.length;
  // Indentation of the caret's line.
  const lineStart = value.lastIndexOf("\n", caret - 1) + 1;
  const indent = /^[ \t]*/.exec(value.slice(lineStart))?.[0] ?? "";

  // For an empty editor, don't lead with a blank line.
  const lead = value.length === 0 ? "" : "\n";
  const body = lead + text;
  const indented = body.replace(/\n/g, "\n" + indent);

  editor.focus();
  editor.setRangeText(indented, lineEnd, lineEnd, "end");

  // Place caret at the sentinel: its offset in `body` plus indent added by
  // every newline that precedes it.
  if (caretMark >= 0) {
    const before = lead.length + caretMark;
    const newlinesBefore = (body.slice(0, before).match(/\n/g) || []).length;
    const caretPos = lineEnd + before + newlinesBefore * indent.length;
    editor.setSelectionRange(caretPos, caretPos);
  }
  scheduleRender();
}

// Insert an arbitrary node snippet (text) at the caret, on its own indented
// line, with the caret landing on the `$` sentinel if present. Shares the
// indentation/positioning logic with insertSnippet but takes raw text so the
// infra icon grid can build snippets dynamically.
function insertNodeText(raw) {
  const caretMark = raw.indexOf("$");
  const text = raw.replace("$", "");
  const value = editor.value;
  const caret = editor.selectionStart;
  let lineEnd = value.indexOf("\n", caret);
  if (lineEnd === -1) lineEnd = value.length;
  const lineStart = value.lastIndexOf("\n", caret - 1) + 1;
  const indent = /^[ \t]*/.exec(value.slice(lineStart))?.[0] ?? "";
  const lead = value.length === 0 ? "" : "\n";
  const body = lead + text;
  const indented = body.replace(/\n/g, "\n" + indent);
  editor.focus();
  editor.setRangeText(indented, lineEnd, lineEnd, "end");
  if (caretMark >= 0) {
    const before = lead.length + caretMark;
    const newlinesBefore = (body.slice(0, before).match(/\n/g) || []).length;
    const caretPos = lineEnd + before + newlinesBefore * indent.length;
    editor.setSelectionRange(caretPos, caretPos);
  }
  scheduleRender();
}

// Infra icon categories, in display order. The build's infra-icons.json may
// grow to ~46 icons; we read an explicit `category` field when present and
// otherwise classify by keyword so new icons still group sensibly.
const INFRA_CATEGORIES = [
  { id: "compute",   title: "Compute" },
  { id: "storage",   title: "Storage" },
  { id: "database",  title: "Database" },
  { id: "network",   title: "Network" },
  { id: "messaging", title: "Messaging" },
  { id: "security",  title: "Security" },
  { id: "other",     title: "Other" },
];

// Keyword → category heuristic (first match wins, checked against the icon
// name + label). Used only when the icon has no explicit `category` field.
const INFRA_KEYWORD_CATEGORY = [
  [/lambda|function|server|container|compute|ec2|fargate|ecs|eks|kubernet|batch|vm|instance/, "compute"],
  [/s3|bucket|storage|disk|volume|efs|ebs|glacier|backup|archive/, "storage"],
  [/db|database|dynamo|rds|aurora|sql|mongo|redis|cache|elasticache|memcached|table/, "database"],
  [/cdn|cloudfront|gateway|vpc|network|route|dns|load.?balance|elb|alb|nlb|subnet|router|firewall.?net|nat|peering|transit|endpoint|ip|api/, "network"],
  [/queue|sqs|sns|topic|kafka|kinesis|event|stream|bus|mq|notification|pubsub|message|mail|ses/, "messaging"],
  [/security|iam|kms|secret|cert|waf|shield|guard|cognito|auth|key|vault|policy|role/, "security"],
];

function categorizeInfra(key, def) {
  if (def.category) return String(def.category).toLowerCase();
  const hay = `${key.split(":")[1] ?? ""} ${def.label ?? ""}`.toLowerCase();
  for (const [re, cat] of INFRA_KEYWORD_CATEGORY) if (re.test(hay)) return cat;
  return "other";
}

// Create one draggable palette tile for an infra icon.
function infraTile(key, def, n) {
  const name = key.split(":")[1];            // e.g. "lambda"
  const id = name.replace(/[^a-z0-9]/g, ""); // node id base, e.g. "loadbalancer"
  const label = def.label ?? name;
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "palette-icon";
  btn.draggable = true;
  btn.title = `Insert {icon:${key}} ${label} · click or drag to canvas`;
  // Unique node id per insertion so repeated clicks don't collide.
  btn.dataset.snip = `${id}${n}["{icon:${key}} ${label}"]`;
  btn.dataset.infra = key;
  const svg =
    `<svg viewBox="0 0 ${def.width} ${def.height}" width="22" height="22" ` +
    `xmlns="http://www.w3.org/2000/svg" aria-hidden="true">${def.body}</svg>`;
  btn.innerHTML = `<span class="pi-icon">${svg}</span><span class="pi-name">${label}</span>`;
  return btn;
}

// Build the Infra icon palette: tiles grouped under category headers (Compute,
// Storage, Database, Network, Messaging, Security, …). Each tile shows the real
// inline SVG glyph; clicking inserts a node like `node["{icon:aws:lambda} Lambda"]`
// (caret lands ready to edit the label) and tiles are draggable onto the canvas.
async function buildInfraPalette() {
  const host = $("palette-infra");
  if (!host) return;
  let icons;
  try {
    const res = await fetch("./infra-icons.json");
    if (!res.ok) return;
    icons = await res.json();
  } catch { return; }

  // Bucket icons by category, preserving insertion order within each.
  const byCat = new Map();
  for (const { id } of INFRA_CATEGORIES) byCat.set(id, []);
  for (const [key, def] of Object.entries(icons)) {
    const cat = categorizeInfra(key, def);
    (byCat.get(cat) ?? byCat.get("other")).push([key, def]);
  }

  const frag = document.createDocumentFragment();
  let n = 1;
  for (const cat of INFRA_CATEGORIES) {
    const entries = byCat.get(cat.id);
    if (!entries || !entries.length) continue;
    const head = document.createElement("div");
    head.className = "palette-cat-title";
    head.textContent = cat.title;
    frag.appendChild(head);
    const grid = document.createElement("div");
    grid.className = "palette-icon-grid";
    for (const [key, def] of entries) grid.appendChild(infraTile(key, def, n++));
    frag.appendChild(grid);
  }
  host.replaceChildren(frag);
}

const palette = $("palette");
$("palette-body").addEventListener("click", (e) => {
  const infra = e.target.closest("[data-infra]");
  if (infra) {
    // Counter keeps node ids unique across repeated clicks.
    insertNodeText(infra.dataset.snip);
    infra.dataset.snip = infra.dataset.snip.replace(/^(\D*)\d+/, (_, p) => p + Math.floor(Math.random() * 1e6));
    return;
  }
  const btn = e.target.closest("[data-snip]");
  if (btn) insertSnippet(btn.dataset.snip);
});

/* ---------------- drag palette items onto the canvas ---------------- */
// Excalidraw/draw.io-style: drag a shape or infra icon from the palette and
// drop it on the canvas. The corresponding source line is appended to the
// diagram, and (for single-node drops) the new node is offset so it lands
// right where you released the pointer.

let dropCounter = 0;

// Single-node shape factories: produce a node declaration with a unique id.
const DROP_NODE = {
  rect:     (id) => `${id}["Label"]`,
  rounded:  (id) => `${id}("Label")`,
  stadium:  (id) => `${id}(["Label"])`,
  decision: (id) => `${id}{"Decision?"}`,
  database: (id) => `${id}[("Database")]:::database`,
  queue:    (id) => `${id}{{"Queue"}}:::queue`,
  circle:   (id) => `${id}(("Label"))`,
};
// Multi-line / multi-node snippets (no single node to position).
const DROP_RAW = {
  subgraph: () => `subgraph cluster${++dropCounter}["Group"]\n  a --> b\nend`,
  arrow:    () => `a --> b`,
  labeled:  () => `a -->|label| b`,
  dashed:   () => `a -.->|async| b`,
  thick:    () => `a ==>|hot path| b`,
};

// Resolve a draggable palette element to the text to append plus, when it is a
// single node, the id we can position at the drop point.
function dropPayload(el) {
  if (el.dataset.infra) {
    const key = el.dataset.infra;                       // e.g. "aws:lambda"
    const base = (key.split(":")[1] || "node").replace(/[^a-z0-9]/gi, "");
    const label = el.querySelector(".pi-name")?.textContent?.trim() || base;
    const name = `${base}${++dropCounter}`;
    return { text: `${name}["{icon:${key}} ${label}"]`, name };
  }
  const key = el.dataset.snip;
  if (DROP_NODE[key]) {
    const name = `n${++dropCounter}`;
    return { text: DROP_NODE[key](name), name };
  }
  if (DROP_RAW[key]) return { text: DROP_RAW[key](), name: null };
  return null;
}

// Append a snippet to the end of the diagram, starting a flowchart if the
// editor is empty, with consistent 2-space body indentation.
function appendToDiagram(text) {
  let base = editor.value;
  if (!base.trim()) base = "flowchart LR\n";
  else if (!base.endsWith("\n")) base += "\n";
  const indent = "  ";
  const body = text.split("\n").map((l) => indent + l).join("\n");
  editor.value = base + body + "\n";
}

// Offset a freshly-added node so its centre lands at the drop point.
function placeDroppedNodeAt(name, clientX, clientY) {
  try {
    const svg = preview.querySelector("svg");
    if (!svg) return;
    const g = svg.querySelector(`[data-node][data-name="${CSS.escape(name)}"]`);
    if (!g) return;
    const bb = g.getBBox();
    const cx = bb.x + bb.width / 2;
    const cy = bb.y + bb.height / 2;
    const rect = viewport.getBoundingClientRect();
    const worldX = (clientX - rect.left - view.x) / view.scale;
    const worldY = (clientY - rect.top - view.y) / view.scale;
    nodeOffsets.set(name, { x: worldX - cx, y: worldY - cy });
    userTouchedView = true; // we've manually placed it; don't auto-refit
    applyOffsets();
  } catch { /* layout/geometry unavailable: node is still added, just unpositioned */ }
}

let activeDrag = null; // payload of the in-flight palette drag

palette.addEventListener("dragstart", (e) => {
  const el = e.target.closest?.(".palette-item[data-snip], .palette-icon[data-infra]");
  if (!el) return;
  activeDrag = dropPayload(el);
  if (!activeDrag) return;
  el.classList.add("dragging");
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "copy";
    // Standards-correct fallback so a drop onto another app/tab still works.
    e.dataTransfer.setData("text/plain", activeDrag.text);
  }
});
palette.addEventListener("dragend", (e) => {
  e.target.closest?.(".dragging")?.classList.remove("dragging");
  activeDrag = null;
  viewport.classList.remove("drag-over");
});

viewport.addEventListener("dragover", (e) => {
  if (!activeDrag) return;
  e.preventDefault(); // required so the drop event fires
  if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
  viewport.classList.add("drag-over");
});
viewport.addEventListener("dragleave", (e) => {
  // Only clear when the pointer actually leaves the viewport.
  if (e.target === viewport) viewport.classList.remove("drag-over");
});
viewport.addEventListener("drop", (e) => {
  e.preventDefault();
  viewport.classList.remove("drag-over");
  const payload = activeDrag ?? (e.dataTransfer?.getData("text/plain")
    ? { text: e.dataTransfer.getData("text/plain"), name: null } : null);
  activeDrag = null;
  if (!payload) return;
  appendToDiagram(payload.text);
  doRender(); // render now so the new node exists for positioning
  if (payload.name) placeDroppedNodeAt(payload.name, e.clientX, e.clientY);
});
$("palette-toggle").addEventListener("click", () => {
  const collapsed = palette.classList.toggle("collapsed");
  $("palette-toggle").setAttribute("aria-expanded", String(!collapsed));
  $("palette-toggle").textContent = collapsed ? "⊞" : "⊟";
  localStorage.setItem("layra-palette-collapsed", collapsed ? "1" : "0");
});
if (localStorage.getItem("layra-palette-collapsed") === "1") {
  palette.classList.add("collapsed");
  $("palette-toggle").setAttribute("aria-expanded", "false");
  $("palette-toggle").textContent = "⊞";
}

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

// Rasterize the current SVG to a canvas at the given scale, then hand the
// resulting blob to `onBlob`. Shared by PNG download and clipboard copy.
function rasterizePng(scale, onBlob) {
  if (!lastGoodSvg) return;
  const svgEl = preview.querySelector("svg");
  if (!svgEl) return;
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
      if (blob) onBlob(blob);
      else reportError("PNG export failed: canvas too large");
    }, "image/png");
  };
  img.src = url;
}

function exportPng(scale = 2) {
  rasterizePng(scale, (blob) => download(`diagram@${scale}x.png`, blob));
}

async function copyPngToClipboard(scale = 2) {
  rasterizePng(scale, async (blob) => {
    try {
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      flashExport("Copied PNG!");
    } catch {
      reportError("Copy failed: clipboard image write not permitted");
    }
  });
}

// Briefly flash a label on the Export trigger as feedback.
let exportFlashTimer = null;
function flashExport(text) {
  const btn = $("btn-export");
  if (!btn) return;
  const original = btn.dataset.label ?? btn.innerHTML;
  btn.dataset.label = original;
  btn.textContent = text;
  clearTimeout(exportFlashTimer);
  exportFlashTimer = setTimeout(() => {
    btn.innerHTML = original;
    delete btn.dataset.label;
  }, 1200);
}

// Keyboard-accessible export dropdown: click or Enter/Space to open,
// arrow keys to move, Escape closes, click-outside closes.
function setupExportMenu() {
  const trigger = $("btn-export");
  const list = $("export-list");
  const items = [...list.querySelectorAll("[role=menuitem]")];

  const open = () => {
    list.hidden = false;
    trigger.setAttribute("aria-expanded", "true");
    items[0]?.focus();
  };
  const close = (focusTrigger = false) => {
    list.hidden = true;
    trigger.setAttribute("aria-expanded", "false");
    if (focusTrigger) trigger.focus();
  };
  const toggle = () => (list.hidden ? open() : close());

  const run = (action) => {
    switch (action) {
      case "svg": exportSvg(); break;
      case "png-1": exportPng(1); break;
      case "png-2": exportPng(2); break;
      case "png-4": exportPng(4); break;
      case "copy-png": copyPngToClipboard(2); break;
    }
  };

  trigger.addEventListener("click", (e) => { e.stopPropagation(); toggle(); });
  trigger.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      open();
    }
  });

  list.addEventListener("click", (e) => {
    const action = e.target?.dataset?.export;
    if (!action) return;
    run(action);
    close(true);
  });
  list.addEventListener("keydown", (e) => {
    const idx = items.indexOf(document.activeElement);
    if (e.key === "ArrowDown") { e.preventDefault(); items[(idx + 1) % items.length].focus(); }
    else if (e.key === "ArrowUp") { e.preventDefault(); items[(idx - 1 + items.length) % items.length].focus(); }
    else if (e.key === "Escape") { e.preventDefault(); close(true); }
    else if (e.key === "Tab") close();
  });

  document.addEventListener("click", (e) => {
    if (!list.hidden && !e.target.closest("#export-menu")) close();
  });
}

async function shareLink() {
  const hash = await encodeSource(editor.value);
  const url = `${location.origin}${location.pathname}#${hash}`;
  history.replaceState(null, "", `#${hash}`);
  const btn = $("btn-share");
  const old = btn.dataset.label ?? btn.textContent;
  btn.dataset.label = old;
  try {
    await navigator.clipboard.writeText(url);
    btn.textContent = "Copied!";
    showToast("Shareable link copied to clipboard");
  } catch {
    // Clipboard unavailable (e.g. insecure context): surface the link in a
    // non-blocking toast the user can select and copy manually.
    btn.textContent = "Link ready";
    showToast(`Copy this link: ${url}`, 6000);
  }
  setTimeout(() => {
    btn.textContent = btn.dataset.label ?? old;
    delete btn.dataset.label;
  }, 1400);
}

// A small, transient toast in the bottom-centre of the viewport. Reused for
// share confirmation and other lightweight, non-blocking feedback.
let toastTimer = null;
function showToast(message, ms = 2400) {
  let el = $("toast");
  if (!el) {
    el = document.createElement("div");
    el.id = "toast";
    el.className = "toast";
    el.setAttribute("role", "status");
    el.setAttribute("aria-live", "polite");
    document.body.appendChild(el);
  }
  el.textContent = message;
  el.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove("show"), ms);
}

/* ---------------- command palette (Cmd/Ctrl+K) ---------------- */
// A searchable overlay of every playground action with arrow-key navigation,
// a focus trap, and Esc-to-close. Each command carries a title, optional
// keyboard hint, and a run() callback. Global shortcuts (below) call the same
// callbacks so the palette and hotkeys never drift.

const centerXY = () => [viewport.clientWidth / 2, viewport.clientHeight / 2];

// Clear all manual node placements and re-fit (the "Reset layout" action).
function resetLayout() {
  nodeOffsets = new Map();
  userTouchedView = false;
  applyOffsets();
  fitToView(true);
}

const COMMANDS = [
  { id: "fit", title: "Fit to view", hint: "0", icon: "⛶", run: () => fitToView(true) },
  { id: "zoom-in", title: "Zoom in", hint: "+", icon: "＋", run: () => smoothZoom(...centerXY(), 1.25) },
  { id: "zoom-out", title: "Zoom out", hint: "−", icon: "－", run: () => smoothZoom(...centerXY(), 1 / 1.25) },
  { id: "zoom-reset", title: "Reset zoom to 100%", icon: "⌖", run: () => smoothZoom(...centerXY(), 1 / view.scale) },
  { id: "reset-layout", title: "Reset layout", desc: "Clear dragged nodes & re-fit", icon: "↺", run: resetLayout },
  { id: "export-svg", title: "Export SVG", icon: "▤", run: () => exportSvg() },
  { id: "export-png-1", title: "Export PNG · 1×", icon: "▤", run: () => exportPng(1) },
  { id: "export-png-2", title: "Export PNG · 2×", icon: "▤", run: () => exportPng(2) },
  { id: "export-png-4", title: "Export PNG · 4×", icon: "▤", run: () => exportPng(4) },
  { id: "copy-png", title: "Copy PNG to clipboard", icon: "⧉", run: () => copyPngToClipboard(2) },
  { id: "toggle-theme", title: "Toggle dark mode", hint: "D", icon: "◐", run: () => toggleTheme() },
  { id: "toggle-animate", title: "Animate edges", desc: "March the dashes on edges", hint: "A", icon: "⇝", run: () => toggleAnimateEdges() },
  { id: "examples", title: "Open examples gallery", icon: "✦", run: () => openGallery() },
  { id: "share", title: "Copy shareable link", icon: "↗", run: () => shareLink() },
];

const cmdk = $("cmdk");
const cmdkInput = $("cmdk-input");
const cmdkList = $("cmdk-list");
const cmdkEmpty = $("cmdk-empty");
let cmdkActive = 0;          // index into the currently-visible commands
let cmdkVisible = [];        // command objects matching the current query
let cmdkLastFocus = null;    // element to restore focus to on close

// Build the static list once; we toggle [hidden] + reorder per query.
function buildCmdk() {
  const frag = document.createDocumentFragment();
  for (const cmd of COMMANDS) {
    const row = document.createElement("div");
    row.className = "cmdk-item";
    row.id = `cmdk-item-${cmd.id}`;
    row.dataset.cmd = cmd.id;
    row.setAttribute("role", "option");
    row.innerHTML =
      `<span class="cmdk-ic" aria-hidden="true">${cmd.icon ?? "›"}</span>` +
      `<span class="cmdk-text"><span class="cmdk-title">${cmd.title}</span>` +
      (cmd.desc ? `<span class="cmdk-desc">${cmd.desc}</span>` : "") +
      `</span>` +
      (cmd.hint ? `<kbd class="cmdk-kbd">${cmd.hint}</kbd>` : "");
    frag.appendChild(row);
  }
  cmdkList.replaceChildren(frag);
}

// Lightweight fuzzy-ish match: every query char must appear in order.
function cmdkMatches(cmd, q) {
  if (!q) return true;
  const hay = (cmd.title + " " + (cmd.desc ?? "")).toLowerCase();
  let i = 0;
  for (const ch of q) {
    i = hay.indexOf(ch, i);
    if (i === -1) return false;
    i++;
  }
  return true;
}

function filterCmdk() {
  const q = cmdkInput.value.trim().toLowerCase();
  cmdkVisible = COMMANDS.filter((c) => cmdkMatches(c, q));
  const visibleIds = new Set(cmdkVisible.map((c) => c.id));
  for (const cmd of COMMANDS) {
    const row = cmdkList.querySelector(`#cmdk-item-${cmd.id}`);
    row.hidden = !visibleIds.has(cmd.id);
  }
  // Reorder DOM to match filtered order so arrow nav follows the list.
  for (const cmd of cmdkVisible) cmdkList.appendChild(cmdkList.querySelector(`#cmdk-item-${cmd.id}`));
  cmdkEmpty.hidden = cmdkVisible.length > 0;
  cmdkActive = 0;
  markCmdkActive();
}

function markCmdkActive() {
  for (const row of cmdkList.querySelectorAll(".cmdk-item")) row.classList.remove("active");
  const cmd = cmdkVisible[cmdkActive];
  if (!cmd) {
    cmdkInput.removeAttribute("aria-activedescendant");
    return;
  }
  const row = cmdkList.querySelector(`#cmdk-item-${cmd.id}`);
  row.classList.add("active");
  row.scrollIntoView({ block: "nearest" });
  cmdkInput.setAttribute("aria-activedescendant", row.id);
}

function openCmdk() {
  if (!cmdk.hidden) return;
  cmdkLastFocus = document.activeElement;
  buildCmdk();
  cmdk.hidden = false;
  cmdkInput.value = "";
  filterCmdk();
  requestAnimationFrame(() => cmdkInput.focus());
}

function closeCmdk() {
  if (cmdk.hidden) return;
  cmdk.hidden = true;
  cmdkLastFocus?.focus?.();
}

function runCmdk(cmd) {
  if (!cmd) return;
  closeCmdk();
  cmd.run();
}

cmdkInput.addEventListener("input", filterCmdk);
cmdkInput.addEventListener("keydown", (e) => {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    cmdkActive = Math.min(cmdkActive + 1, cmdkVisible.length - 1);
    markCmdkActive();
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    cmdkActive = Math.max(cmdkActive - 1, 0);
    markCmdkActive();
  } else if (e.key === "Enter") {
    e.preventDefault();
    runCmdk(cmdkVisible[cmdkActive]);
  } else if (e.key === "Escape") {
    e.preventDefault();
    closeCmdk();
  } else if (e.key === "Tab") {
    // Focus trap: the input is the only focusable control, so keep it here.
    e.preventDefault();
    cmdkActive = e.shiftKey
      ? Math.max(cmdkActive - 1, 0)
      : Math.min(cmdkActive + 1, cmdkVisible.length - 1);
    markCmdkActive();
  }
});
cmdkList.addEventListener("click", (e) => {
  const row = e.target.closest(".cmdk-item");
  if (row) runCmdk(COMMANDS.find((c) => c.id === row.dataset.cmd));
});
cmdkList.addEventListener("mousemove", (e) => {
  const row = e.target.closest(".cmdk-item");
  if (!row) return;
  const idx = cmdkVisible.findIndex((c) => c.id === row.dataset.cmd);
  if (idx >= 0 && idx !== cmdkActive) { cmdkActive = idx; markCmdkActive(); }
});
cmdk.querySelector(".cmdk-backdrop").addEventListener("click", closeCmdk);

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
  const saved = localStorage.getItem(AUTOSAVE_KEY);
  if (!fromHash) {
    editor.value = saved ?? "";
  }
  applyTheme();
  applyAnimateEdges();
  buildInfraPalette();
  setupNewMenu();
  doRender();
  fitToView();

  // Onboarding: open the examples gallery on a truly fresh start (no shared
  // link, no autosaved work) or whenever the editor is empty.
  if (!fromHash && (!saved || !saved.trim())) {
    openGallery();
  }

  editor.addEventListener("input", scheduleRender);
  editor.addEventListener("scroll", () => {
    syncGutterScroll();
    if (status.classList.contains("err") || status.classList.contains("warn")) {
      const m = /line (\d+)/.exec(status.textContent);
      highlightLine(m ? Number(m[1]) : null);
    }
  });
  window.addEventListener("hashchange", loadFromHash);
  window.addEventListener("resize", () => {
    if (!userTouchedView) fitToView();
    else updateMinimapView();
  });

  $("btn-theme").addEventListener("click", toggleTheme);
  $("btn-animate").addEventListener("click", toggleAnimateEdges);
  $("btn-share").addEventListener("click", shareLink);
  setupExportMenu();
  // Test hook: lets headless checks invoke Share without a click/clipboard.
  window.__layraShare = shareLink;

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

  // Global: Cmd/Ctrl+K toggles the command palette from anywhere (even while
  // typing in the editor). Esc closes it. The palette's own keydown handles
  // navigation once it's open.
  window.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && (e.key === "k" || e.key === "K")) {
      e.preventDefault();
      cmdk.hidden ? openCmdk() : closeCmdk();
    }
  });

  window.addEventListener("keydown", (e) => {
    if (document.activeElement === editor) return;
    const center = [viewport.clientWidth / 2, viewport.clientHeight / 2];
    if (e.key === "+" || e.key === "=") smoothZoom(...center, 1.25);
    else if (e.key === "-") smoothZoom(...center, 1 / 1.25);
    else if (e.key === "0") fitToView(true);
    else if (e.key === "d" || e.key === "D") $("btn-theme").click();
    else if (e.key === "a" || e.key === "A") $("btn-animate").click();
  });
}

main();
