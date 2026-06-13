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
  const type = detectType(src);
  for (const btn of $("templates").querySelectorAll("button")) {
    btn.classList.toggle("active", btn.dataset.tpl === type);
  }
}

$("templates").addEventListener("click", (e) => {
  const tpl = e.target?.dataset?.tpl;
  if (!tpl || !TEMPLATES[tpl]) return;
  editor.value = TEMPLATES[tpl];
  nodeOffsets = new Map();
  userTouchedView = false;
  scheduleRender();
});

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

// Build the Infra icon grid: one button per bundled aws:* icon, each showing
// the real inline SVG glyph. Clicking inserts a node like
// `node["{icon:aws:lambda} Lambda"]` (caret lands ready to edit the label).
async function buildInfraPalette() {
  const host = $("palette-infra");
  if (!host) return;
  let icons;
  try {
    const res = await fetch("./infra-icons.json");
    if (!res.ok) return;
    icons = await res.json();
  } catch { return; }

  const frag = document.createDocumentFragment();
  let n = 1;
  for (const [key, def] of Object.entries(icons)) {
    const name = key.split(":")[1];           // e.g. "lambda"
    const id = name.replace(/[^a-z0-9]/g, ""); // node id base, e.g. "loadbalancer"
    const label = def.label ?? name;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "palette-icon";
    btn.title = `Insert {icon:${key}} ${label}`;
    // Unique node id per insertion so repeated clicks don't collide.
    btn.dataset.snip = `${id}${n}["{icon:${key}} ${label}"]`;
    btn.dataset.infra = key;
    const svg =
      `<svg viewBox="0 0 ${def.width} ${def.height}" width="22" height="22" ` +
      `xmlns="http://www.w3.org/2000/svg" aria-hidden="true">${def.body}</svg>`;
    btn.innerHTML = `<span class="pi-icon">${svg}</span><span class="pi-name">${label}</span>`;
    frag.appendChild(btn);
    n++;
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
  const saved = localStorage.getItem(AUTOSAVE_KEY);
  if (!fromHash) {
    editor.value = saved ?? "";
  }
  applyTheme();
  buildInfraPalette();
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

  $("btn-theme").addEventListener("click", () => {
    dark = !dark;
    applyTheme();
    scheduleRender();
  });
  $("btn-share").addEventListener("click", shareLink);
  setupExportMenu();

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
    if (e.key === "+" || e.key === "=") smoothZoom(...center, 1.25);
    else if (e.key === "-") smoothZoom(...center, 1 / 1.25);
    else if (e.key === "0") fitToView(true);
    else if (e.key === "d" || e.key === "D") $("btn-theme").click();
  });
}

main();
