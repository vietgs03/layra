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
  aws: `flowchart TB
  dns["{icon:mdi:dns} Route 53"]:::client
  cdn["{icon:aws:cdn} CloudFront"]:::client
  static[("{icon:aws:s3} S3 static site")]:::storage

  subgraph vpc["VPC · 10.0.0.0/16"]
    alb["{icon:aws:load-balancer} App Load Balancer"]:::gateway
    subgraph azA["Availability Zone A"]
      web1["{icon:aws:server} EC2 web"]:::compute
      svc1["{icon:aws:container} ECS service"]:::compute
    end
    subgraph azB["Availability Zone B"]
      web2["{icon:aws:server} EC2 web"]:::compute
      svc2["{icon:aws:container} ECS service"]:::compute
    end
    rds[("{icon:aws:database} RDS · Multi-AZ")]:::database
  end

  api["{icon:aws:gateway} API Gateway"]:::gateway
  fn["{icon:aws:lambda} Lambda"]:::highlight
  ddb[("{icon:aws:database} DynamoDB")]:::database
  cache[("{icon:aws:cache} ElastiCache")]:::cache
  q{{"{icon:aws:queue} SQS"}}:::queue
  sns["{icon:mdi:bullhorn} SNS"]:::queue

  dns --> cdn
  cdn -->|static| static
  cdn ==>|HTTPS| alb
  alb --> web1
  alb --> web2
  alb --> svc1
  alb --> svc2
  web1 --> rds
  web2 --> rds
  svc1 -.-> cache
  svc2 -.-> cache
  cdn ==>|/api| api
  api ==>|invoke| fn
  fn --> ddb
  fn ==>|enqueue| q
  q -.->|fan-out| sns
`,
  microservices: `flowchart LR
  client["{icon:mdi:web} Web & Mobile"]:::client
  gw["{icon:aws:gateway} API Gateway"]:::gateway

  subgraph mesh["Service Mesh"]
    auth["{icon:aws:server} Auth"]:::service
    orders["{icon:aws:server} Orders"]:::service
    catalog["{icon:aws:server} Catalog"]:::service
    pay["{icon:aws:server} Payments"]:::service
  end

  authdb[("{icon:aws:database} Auth DB")]:::database
  ordersdb[("{icon:aws:database} Orders DB")]:::database
  cache[("{icon:aws:cache} Redis")]:::cache
  bus{{"{icon:aws:queue} Event Bus"}}:::queue

  client ==>|HTTPS| gw
  gw --> auth
  gw --> orders
  gw --> catalog
  gw --> pay
  auth --> authdb
  orders --> ordersdb
  catalog -.->|read-through| cache
  orders ==>|publish| bus
  pay ==>|publish| bus
  bus -.->|subscribe| orders
`,
  cicd: `flowchart LR
  dev(["{icon:mdi:account} Developer"]):::client
  repo["{icon:mdi:git} Git push"]:::service

  subgraph ci["CI · build & verify"]
    build["{icon:aws:container} Build"]:::compute
    test["{icon:mdi:test-tube} Test"]:::compute
    scan["{icon:mdi:shield-check} Security scan"]:::service
  end

  registry[("{icon:aws:s3} Artifact registry")]:::storage

  subgraph cd["CD · release"]
    stage["{icon:aws:server} Staging"]:::service
    prod["{icon:aws:server} Production"]:::highlight
  end

  dev --> repo
  repo ==>|trigger| build
  build --> test
  test --> scan
  scan ==>|publish| registry
  registry ==>|deploy| stage
  stage -->|approve| prod
  prod -.->|rollback| stage
`,
  eventdriven: `flowchart TB
  subgraph producers["Producers"]
    web["{icon:mdi:web} Web app"]:::client
    iot["{icon:mdi:chip} IoT devices"]:::client
  end

  bus{{"{icon:aws:queue} EventBridge bus"}}:::queue

  subgraph consumers["Consumers · Lambda"]
    fn1["{icon:aws:lambda} Process order"]:::highlight
    fn2["{icon:aws:lambda} Send email"]:::highlight
    fn3["{icon:aws:lambda} Update analytics"]:::highlight
  end

  stream["{icon:mdi:chart-line} Kinesis stream"]:::service
  ddb[("{icon:aws:database} DynamoDB")]:::database
  warehouse[("{icon:aws:database} Redshift")]:::database

  web ==>|events| bus
  iot ==>|telemetry| bus
  bus --> fn1
  bus --> fn2
  bus --> fn3
  fn1 --> ddb
  fn2 -.-> ddb
  fn3 ==>|stream| stream
  stream -.->|load| warehouse
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

let lastGoodSvg = "";
let rafPending = false;

/* ---------------- theme / brand (U20) ---------------- */
// Layra ships two base palettes (light/dark, baked into the engine SVG via the
// `dark` boolean) plus brand presets and a fully custom theme. A theme is
// {base, accent, font, bg}; the playground maps it onto CSS custom properties
// for the chrome + canvas, and bakes font/background into exported SVG so a
// shared diagram keeps its look. The choice is persisted in localStorage.

const THEME_KEY = "layra-theme";

// Named font stacks the theme editor can pick from (keys are persisted).
const FONTS = {
  inter:   { label: "Inter (default)", stack: "Inter, 'Helvetica Neue', Arial, sans-serif" },
  system:  { label: "System UI",       stack: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif" },
  rounded: { label: "Rounded",         stack: "'Trebuchet MS', 'Segoe UI', Verdana, sans-serif" },
  serif:   { label: "Serif",           stack: "Georgia, 'Times New Roman', serif" },
  mono:    { label: "Monospace",       stack: "ui-monospace, 'SF Mono', Menlo, Consolas, monospace" },
};
const fontStack = (key) => (FONTS[key] ?? FONTS.inter).stack;

// Preset themes. `base` selects the engine palette (drives the `dark` bool);
// `accent` tints UI chrome + the canvas; `bg` is baked into exports.
const THEMES = {
  light:   { name: "Light",   base: "light", accent: "#3b82f6", font: "inter",   bg: "#ffffff", builtin: true },
  dark:    { name: "Dark",    base: "dark",  accent: "#60a5fa", font: "inter",   bg: "#0f1115", builtin: true },
  aws:     { name: "AWS",     base: "light", accent: "#ff9900", font: "inter",   bg: "#fbf8f3" },
  gcp:     { name: "GCP",     base: "light", accent: "#1a73e8", font: "rounded", bg: "#f6f9fe" },
  azure:   { name: "Azure",   base: "dark",  accent: "#2899f5", font: "inter",   bg: "#0a1626" },
  neutral: { name: "Neutral", base: "light", accent: "#475569", font: "system",  bg: "#fafaf9" },
};
const THEME_ORDER = ["light", "dark", "aws", "gcp", "azure", "neutral"];

// Active selection: a preset id, or "custom" backed by `customTheme`.
let themeId = "light";
let customTheme = { base: "light", accent: "#7c5cff", font: "inter", bg: "#ffffff" };

function loadThemePref() {
  try {
    const raw = JSON.parse(localStorage.getItem(THEME_KEY) ?? "null");
    if (raw && typeof raw === "object") {
      if (raw.custom && typeof raw.custom === "object") {
        customTheme = { ...customTheme, ...raw.custom };
      }
      if (raw.id && (raw.id === "custom" || THEMES[raw.id])) themeId = raw.id;
      return;
    }
  } catch { /* fall through to first-run default */ }
  // First run: honour the OS preference for light vs dark.
  themeId = matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
loadThemePref();

function saveThemePref() {
  localStorage.setItem(THEME_KEY, JSON.stringify({ id: themeId, custom: customTheme }));
}

// The resolved active theme as a normalized object (always has id/name/base/
// accent/font/bg with a concrete font *stack*).
function resolveTheme() {
  const t = themeId === "custom" ? { ...customTheme, name: "Custom" } : THEMES[themeId] ?? THEMES.light;
  return {
    id: themeId,
    name: t.name ?? "Custom",
    base: t.base === "dark" ? "dark" : "light",
    accent: t.accent || "#3b82f6",
    fontKey: t.font || "inter",
    font: fontStack(t.font),
    bg: t.bg || (t.base === "dark" ? "#0f1115" : "#ffffff"),
  };
}

// Mirrors the engine palette switch; `true` => the dark engine SVG.
let dark = resolveTheme().base === "dark";

// Mix two #rrggbb colors by `amt` (0..1) toward `b`.
function mixHex(a, b, amt) {
  const pa = parseHex(a), pb = parseHex(b);
  if (!pa || !pb) return a;
  const m = (x, y) => Math.round(x + (y - x) * amt);
  const h = (n) => n.toString(16).padStart(2, "0");
  return `#${h(m(pa[0], pb[0]))}${h(m(pa[1], pb[1]))}${h(m(pa[2], pb[2]))}`;
}
function parseHex(s) {
  const m = /^#([0-9a-f]{6})$/i.exec((s || "").trim());
  if (!m) {
    const m3 = /^#([0-9a-f]{3})$/i.exec((s || "").trim());
    if (!m3) return null;
    const [r, g, b] = m3[1].split("");
    return [parseInt(r + r, 16), parseInt(g + g, 16), parseInt(b + b, 16)];
  }
  const h = m[1];
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

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
  // Space toggles pan-mode, but only when focus isn't on an interactive
  // control (so Space still activates buttons/links and types in inputs).
  if (e.code !== "Space") return;
  const ae = document.activeElement;
  const interactive = ae && ae.closest?.("button, a, input, textarea, select, [role=menuitem], [role=option]");
  if (interactive) return;
  spaceHeld = true;
  viewport.classList.add("pan-mode");
  e.preventDefault();
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

// Keyboard resize: the splitter is focusable (role=separator). Left/Right
// nudge by 24px, Home/End jump to sensible extremes; the ratio is persisted.
splitter.addEventListener("keydown", (e) => {
  const cur = editorPane.getBoundingClientRect().width;
  const step = 24;
  let w = cur;
  if (e.key === "ArrowLeft") w = cur - step;
  else if (e.key === "ArrowRight") w = cur + step;
  else if (e.key === "Home") w = 280;
  else if (e.key === "End") w = window.innerWidth - 360;
  else return;
  e.preventDefault();
  w = Math.min(Math.max(w, 240), window.innerWidth - 320);
  editorPane.style.width = `${w}px`;
  localStorage.setItem(SPLIT_KEY, (w / window.innerWidth).toFixed(4));
  splitter.setAttribute("aria-valuenow", Math.round((w / window.innerWidth) * 100));
  if (!userTouchedView) fitToView();
  else updateMinimapView();
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
  const t = resolveTheme();
  dark = t.base === "dark";
  const root = document.documentElement;
  root.classList.toggle("dark", dark);
  // Tint the chrome: accent everywhere, and a per-base derivation of the canvas
  // and dot grid so brand themes feel cohesive (custom CSS props override the
  // light/dark defaults from style.css).
  root.style.setProperty("--accent", t.accent);
  root.style.setProperty("--app-font", t.font);
  // Canvas background comes straight from the theme; the dot grid is a subtle
  // mix toward the accent so brand colour reads on the canvas too.
  root.style.setProperty("--canvas", t.bg);
  root.style.setProperty("--dots", mixHex(t.bg, t.accent, dark ? 0.16 : 0.12));
  // Keep an attribute so CSS / tests can see the active theme id.
  root.dataset.theme = t.id;

  const btn = $("btn-theme");
  if (btn) {
    btn.setAttribute("aria-pressed", String(dark));
    btn.title = `Theme: ${t.name} — click to open the theme editor`;
    btn.setAttribute("aria-label", `Theme: ${t.name}. Open theme editor`);
  }
  syncThemeEditor();
}

// Quick light/dark flip from the "D" shortcut: jump between the two base
// palettes without leaving a brand preset stuck on the wrong base.
function toggleTheme() {
  if (themeId === "dark") setTheme("light");
  else if (themeId === "light") setTheme("dark");
  else {
    // On a brand/custom theme: flip just the base and re-render.
    if (themeId === "custom") customTheme.base = dark ? "light" : "dark";
    else {
      // Switch to the nearest base preset to avoid mutating a builtin.
      setTheme(dark ? "light" : "dark");
      return;
    }
    saveThemePref();
    applyTheme();
    scheduleRender();
  }
}

// Apply + persist a named theme (preset id or "custom") and re-render so the
// engine SVG picks up the matching light/dark palette.
function setTheme(id) {
  themeId = id;
  saveThemePref();
  applyTheme();
  scheduleRender();
}

/* ---------------- theme editor dialog (U20) ---------------- */

const themeDialog = $("theme");
let themeLastFocus = null;
let themeEditorBuilt = false;

// Build the preset swatch grid + font picker once.
function buildThemeEditor() {
  if (themeEditorBuilt) return;
  const presets = $("theme-presets");
  const frag = document.createDocumentFragment();
  for (const id of THEME_ORDER) {
    const t = THEMES[id];
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "theme-preset";
    btn.dataset.theme = id;
    btn.setAttribute("role", "radio");
    btn.title = `${t.name} theme`;
    btn.innerHTML =
      `<span class="theme-swatch" style="background:${t.bg}">` +
      `<span class="theme-swatch-dot" style="background:${t.accent}"></span></span>` +
      `<span class="theme-preset-name">${t.name}</span>`;
    btn.addEventListener("click", () => setTheme(id));
    frag.appendChild(btn);
  }
  presets.replaceChildren(frag);

  const fontSel = $("theme-font");
  fontSel.replaceChildren();
  for (const [key, def] of Object.entries(FONTS)) {
    const opt = document.createElement("option");
    opt.value = key;
    opt.textContent = def.label;
    fontSel.appendChild(opt);
  }

  // Editing any control switches to the "custom" theme, seeded from the
  // currently-resolved theme so tweaks start from what's on screen.
  const startCustom = () => {
    if (themeId !== "custom") {
      const cur = resolveTheme();
      customTheme = { base: cur.base, accent: cur.accent, font: cur.fontKey, bg: cur.bg };
      themeId = "custom";
    }
  };
  const accent = $("theme-accent");
  const accentHex = $("theme-accent-hex");
  const bg = $("theme-bg");
  const bgHex = $("theme-bg-hex");
  const base = $("theme-base");

  const onAccent = (val) => {
    if (!parseHex(val)) return;
    startCustom();
    customTheme.accent = val.toLowerCase();
    saveThemePref();
    applyTheme();
  };
  const onBg = (val) => {
    if (!parseHex(val)) return;
    startCustom();
    customTheme.bg = val.toLowerCase();
    saveThemePref();
    applyTheme();
  };
  accent.addEventListener("input", (e) => onAccent(e.target.value));
  accentHex.addEventListener("input", (e) => onAccent(e.target.value));
  bg.addEventListener("input", (e) => onBg(e.target.value));
  bgHex.addEventListener("input", (e) => onBg(e.target.value));
  base.addEventListener("change", (e) => {
    startCustom();
    customTheme.base = e.target.value === "dark" ? "dark" : "light";
    saveThemePref();
    applyTheme();
    scheduleRender();
  });
  fontSel.addEventListener("change", (e) => {
    startCustom();
    customTheme.font = e.target.value;
    saveThemePref();
    applyTheme();
  });
  themeEditorBuilt = true;
}

// Reflect the active theme into the editor controls + preset selection.
function syncThemeEditor() {
  const presets = $("theme-presets");
  if (presets) {
    for (const b of presets.querySelectorAll(".theme-preset")) {
      const on = b.dataset.theme === themeId;
      b.classList.toggle("active", on);
      b.setAttribute("aria-checked", String(on));
    }
  }
  const t = resolveTheme();
  const set = (id, v) => { const el = $(id); if (el && document.activeElement !== el) el.value = v; };
  set("theme-accent", t.accent);
  set("theme-accent-hex", t.accent);
  set("theme-bg", t.bg);
  set("theme-bg-hex", t.bg);
  set("theme-base", t.base);
  set("theme-font", t.fontKey);
}

function openThemeEditor() {
  if (!themeDialog.hidden) return;
  if (!cmdk.hidden) closeCmdk();
  if (!help.hidden) closeHelp();
  buildThemeEditor();
  syncThemeEditor();
  themeLastFocus = document.activeElement;
  themeDialog.hidden = false;
  requestAnimationFrame(() => $("theme-close").focus());
}

function closeThemeEditor() {
  if (themeDialog.hidden) return;
  themeDialog.hidden = true;
  themeLastFocus?.focus?.();
}

function reportError(message) {
  status.textContent = message;
  status.className = "status err";
  const m = /line (\d+)/.exec(message);
  const line = m ? Number(m[1]) : null;
  highlightLine(line);
  editor.setAttribute("aria-invalid", "true");
  showIssueToast(message, line, "error");
}

// Jump the editor caret to (and focus) a 1-based line, scrolling it into view.
function jumpToLine(line) {
  if (!Number.isFinite(line) || line < 1) return;
  const lines = editor.value.split("\n");
  let pos = 0;
  for (let i = 0; i < line - 1 && i < lines.length; i++) pos += lines[i].length + 1;
  const end = pos + (lines[line - 1]?.length ?? 0);
  editor.focus();
  editor.setSelectionRange(pos, end);
  // Centre the line vertically.
  const lh = parseFloat(getComputedStyle(editor).lineHeight) || 18;
  editor.scrollTop = Math.max(0, (line - 1) * lh - editor.clientHeight / 2);
  syncGutterScroll();
  highlightLine(line);
}

// Surface a syntax error / lenient-render warning as a clickable toast that
// shows the offending source line; clicking jumps the editor caret there.
// Engine messages are line-numbered ("... line N ..."), parsed out for the
// snippet. `severity` is "error" (hard parse failure) or "warn" (rendered with
// skipped lines). De-duped by signature so it doesn't re-flash on every
// keystroke while the same issue persists.
let lastIssueSig = null;
function showIssueToast(message, line, severity = "error", count = 1) {
  const sig = `${severity}|${line}`;
  const el = ensureToast();
  const alreadyShowing = el.classList.contains("show") && lastIssueSig === sig;
  lastIssueSig = sig;

  el.className = `toast error ${severity === "warn" ? "is-warn" : ""}`.trim();
  el.replaceChildren();
  const title = document.createElement("span");
  title.className = "toast-title";
  const what = severity === "warn"
    ? (count > 1 ? `${count} lines skipped` : "Line skipped")
    : "Syntax error";
  title.textContent = line ? `${what} · line ${line}` : what;
  el.appendChild(title);
  const msg = document.createElement("span");
  msg.textContent = message;
  el.appendChild(msg);
  if (line) {
    const src = editor.value.split("\n")[line - 1];
    if (src != null) {
      const code = document.createElement("span");
      code.className = "toast-code";
      code.textContent = `${line} │ ${src.trim() || "(empty line)"}`;
      el.appendChild(code);
    }
    const hint = document.createElement("span");
    hint.className = "toast-hint";
    hint.textContent = "Click to jump to this line";
    el.appendChild(hint);
    el.onclick = () => { jumpToLine(line); el.classList.remove("show"); };
  } else {
    el.onclick = null;
  }
  el.classList.add("show");
  // Errors linger; warnings auto-dismiss. Don't restart the timer if the same
  // issue is already on screen (avoids flicker while typing).
  clearTimeout(toastTimer);
  if (severity === "warn") {
    toastTimer = setTimeout(() => el.classList.remove("show"), alreadyShowing ? 4200 : 5000);
  }
}

// Hide the issue toast (called on a clean render).
function dismissIssueToast() {
  lastIssueSig = null;
  const el = $("toast");
  if (el && el.classList.contains("error")) el.classList.remove("show");
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
      const line = m ? Number(m[1]) : null;
      highlightLine(line);
      editor.setAttribute("aria-invalid", "false");
      showIssueToast(warnings[0], line, "warn", warnings.length);
    } else {
      status.textContent = "ok";
      status.className = "status ok";
      highlightLine(null);
      editor.setAttribute("aria-invalid", "false");
      dismissIssueToast();
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
  aws: `flowchart TB
  cdn["{icon:aws:cdn} CloudFront"]:::client
  subgraph vpc["VPC"]
    alb["{icon:aws:load-balancer} Load Balancer"]:::gateway
    web["{icon:aws:server} EC2 / ECS"]:::compute
    db[("{icon:aws:database} RDS · Multi-AZ")]:::database
  end
  api["{icon:aws:gateway} API Gateway"]:::gateway
  fn["{icon:aws:lambda} Lambda"]:::highlight
  ddb[("{icon:aws:database} DynamoDB")]:::database

  cdn ==>|HTTPS| alb
  alb --> web
  web --> db
  cdn ==>|/api| api
  api ==>|invoke| fn
  fn --> ddb
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
  { key: "aws", title: "AWS 3-tier architecture", desc: "VPC, Multi-AZ, ALB → EC2/ECS → RDS, Lambda → DynamoDB" },
  { key: "microservices", title: "Microservices", desc: "API gateway, service mesh & event bus" },
  { key: "eventdriven", title: "Event-driven", desc: "EventBridge fan-out to Lambda consumers" },
  { key: "cicd", title: "CI/CD pipeline", desc: "Build, test, scan → staged release" },
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

// Examples that show off nested clusters + animated edges + colored icons —
// flagged so the gallery card gets the "featured" accent treatment.
const FEATURED_EXAMPLES = new Set(["aws", "microservices", "eventdriven", "cicd"]);

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
    if (FEATURED_EXAMPLES.has(ex.key)) card.classList.add("featured");

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
  // The architecture showcases are designed around data-flow animation: turn
  // it on automatically so they land with edges marching out of the box.
  if (FEATURED_EXAMPLES.has(key) && !animateEdges) {
    animateEdges = true;
    localStorage.setItem(ANIMATE_KEY, "1");
    applyAnimateEdges();
  }
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

/* ---------------- help dialog ---------------- */
// Keyboard shortcuts + a syntax cheat-sheet. Opened from the ? button, the "?"
// key, or the command palette. Esc closes; focus is trapped within the panel
// and restored to the opener on close.

const help = $("help");
let helpLastFocus = null;

function openHelp() {
  if (!help.hidden) return;
  // Close any other overlay first so we never stack dialogs.
  if (!cmdk.hidden) closeCmdk();
  helpLastFocus = document.activeElement;
  help.hidden = false;
  requestAnimationFrame(() => $("help-close").focus());
}

function closeHelp() {
  if (help.hidden) return;
  help.hidden = true;
  helpLastFocus?.focus?.();
}

$("btn-help").addEventListener("click", openHelp);
$("help-close").addEventListener("click", closeHelp);
help.querySelector(".help-backdrop").addEventListener("click", closeHelp);
help.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    e.preventDefault();
    closeHelp();
  } else if (e.key === "Tab") {
    // Focus trap: keep Tab within the dialog's focusable controls.
    const focusable = help.querySelectorAll("button, a[href], [tabindex]:not([tabindex='-1'])");
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
  }
});

// Theme editor dialog wiring: close button, backdrop click, Esc + focus trap.
$("theme-close").addEventListener("click", closeThemeEditor);
themeDialog.querySelector(".theme-backdrop").addEventListener("click", closeThemeEditor);
themeDialog.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    e.preventDefault();
    closeThemeEditor();
  } else if (e.key === "Tab") {
    const focusable = themeDialog.querySelectorAll(
      "button, a[href], select, input, [tabindex]:not([tabindex='-1'])");
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
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
// `color` matches the AWS service palette (and the engine's role hues): icons
// drawn with `currentColor` pick this up; L9's multi-color icons keep their own
// fills, so the palette lights up correctly either way (never hardcoded grey).
const INFRA_CATEGORIES = [
  { id: "compute",   title: "Compute",   color: "#ed7100" }, // AWS compute orange
  { id: "storage",   title: "Storage",   color: "#7aa116" }, // AWS storage green
  { id: "database",  title: "Database",  color: "#2563eb" }, // database blue
  { id: "network",   title: "Network",   color: "#8c4fff" }, // AWS networking purple
  { id: "messaging", title: "Messaging", color: "#e7157b" }, // AWS app-integration pink
  { id: "security",  title: "Security",  color: "#dd344c" }, // AWS security red
  { id: "other",     title: "Other",     color: "#64748b" }, // neutral slate
];
const INFRA_CAT_COLOR = Object.fromEntries(INFRA_CATEGORIES.map((c) => [c.id, c.color]));

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

// Create one draggable palette tile for an infra icon, tinted by its category
// color. The glyph inherits `color` (so single-color `currentColor` icons take
// the category hue) while L9's multi-color icons keep their own fills.
function infraTile(key, def, n, cat) {
  const name = key.split(":")[1];            // e.g. "lambda"
  const id = name.replace(/[^a-z0-9]/g, ""); // node id base, e.g. "loadbalancer"
  const label = def.label ?? name;
  const color = INFRA_CAT_COLOR[cat] ?? INFRA_CAT_COLOR.other;
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "palette-icon";
  btn.draggable = true;
  btn.dataset.cat = cat;
  btn.style.setProperty("--cat", color);
  btn.title = `Insert {icon:${key}} ${label} · click or drag to canvas`;
  // Unique node id per insertion so repeated clicks don't collide.
  btn.dataset.snip = `${id}${n}["{icon:${key}} ${label}"]`;
  btn.dataset.infra = key;
  const svg =
    `<svg viewBox="0 0 ${def.width} ${def.height}" width="22" height="22" ` +
    `xmlns="http://www.w3.org/2000/svg" aria-hidden="true">${def.body}</svg>`;
  // A larger preview glyph is shown in a popover on hover (see CSS).
  btn.innerHTML =
    `<span class="pi-icon">${svg}</span>` +
    `<span class="pi-name">${label}</span>` +
    `<span class="pi-preview" aria-hidden="true">` +
      `<span class="pi-preview-glyph">${svg}</span>` +
      `<span class="pi-preview-label">${label}</span>` +
    `</span>`;
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
    head.dataset.cat = cat.id;
    head.style.setProperty("--cat", cat.color);
    // A colored dot + label so each category reads at a glance.
    head.innerHTML = `<span class="palette-cat-dot" aria-hidden="true"></span>${cat.title}`;
    frag.appendChild(head);
    const grid = document.createElement("div");
    grid.className = "palette-icon-grid";
    for (const [key, def] of entries) grid.appendChild(infraTile(key, def, n++, cat.id));
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
// Friendly labels for the confirmation toast / drop ghost.
const DROP_LABEL = {
  rect: "Process", rounded: "Rounded box", stadium: "Terminal", decision: "Decision",
  database: "Database", queue: "Queue", circle: "Circle", subgraph: "Subgraph",
  arrow: "Arrow", labeled: "Labeled edge", dashed: "Dashed edge", thick: "Thick edge",
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
// single node, the id we can position at the drop point, and a human label for
// the confirmation toast / drag ghost.
function dropPayload(el) {
  if (el.dataset.infra) {
    const key = el.dataset.infra;                       // e.g. "aws:lambda"
    const base = (key.split(":")[1] || "node").replace(/[^a-z0-9]/gi, "");
    const label = el.querySelector(".pi-name")?.textContent?.trim() || base;
    const name = `${base}${++dropCounter}`;
    return { text: `${name}["{icon:${key}} ${label}"]`, name, label };
  }
  const key = el.dataset.snip;
  if (DROP_NODE[key]) {
    const name = `n${++dropCounter}`;
    return { text: DROP_NODE[key](name), name, label: DROP_LABEL[key] ?? "Node" };
  }
  if (DROP_RAW[key]) return { text: DROP_RAW[key](), name: null, label: DROP_LABEL[key] ?? "Snippet" };
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

/* Pointer-based palette drag. Native HTML5 drag (`dragstart`/`drop`) gives no
   live visual feedback and — crucially — never fires for synthetic pointer
   input, which is exactly why v0.3 felt like "I can't drag". Here we drive the
   whole gesture from pointer events: a ghost element tracks the cursor, the
   canvas highlights as a drop target, a dashed placeholder shows precisely
   where the new node will land, and a confirmation toast fires on drop. Works
   identically for real mice, touch, and automated tests. */

const DRAG_THRESHOLD = 4; // px the pointer must travel before a drag begins
let pendingDrag = null;   // { el, payload, sx, sy, pointerId } before threshold
let dragGhost = null;     // floating element following the cursor
let dropIndicator = null; // dashed placeholder inside the viewport

// Lazily create the floating ghost that tracks the cursor during a drag.
function ensureGhost() {
  if (!dragGhost) {
    dragGhost = document.createElement("div");
    dragGhost.className = "drag-ghost";
    dragGhost.setAttribute("aria-hidden", "true");
    document.body.appendChild(dragGhost);
  }
  return dragGhost;
}

// Lazily create the dashed drop placeholder shown inside the viewport.
function ensureDropIndicator() {
  if (!dropIndicator) {
    dropIndicator = document.createElement("div");
    dropIndicator.className = "drop-indicator";
    dropIndicator.setAttribute("aria-hidden", "true");
    dropIndicator.innerHTML = `<span class="drop-indicator-label"></span>`;
    viewport.appendChild(dropIndicator);
  }
  return dropIndicator;
}

// Is the given client point inside the canvas viewport?
function pointInViewport(clientX, clientY) {
  const r = viewport.getBoundingClientRect();
  return clientX >= r.left && clientX <= r.right && clientY >= r.top && clientY <= r.bottom;
}

// Position the ghost + drop indicator for the current pointer location.
function updateDragVisuals(clientX, clientY) {
  const ghost = ensureGhost();
  ghost.style.transform = `translate(${clientX + 14}px, ${clientY + 14}px)`;
  const over = pointInViewport(clientX, clientY);
  viewport.classList.toggle("drag-over", over);
  ghost.classList.toggle("over-canvas", over);
  const ind = ensureDropIndicator();
  if (over && activeDrag) {
    const r = viewport.getBoundingClientRect();
    ind.style.left = `${clientX - r.left}px`;
    ind.style.top = `${clientY - r.top}px`;
    ind.querySelector(".drop-indicator-label").textContent = activeDrag.label || "Drop here";
    ind.classList.add("show");
  } else {
    ind.classList.remove("show");
  }
}

// Begin a drag once the pointer has moved past the threshold.
function beginPaletteDrag(clientX, clientY) {
  const { el, payload } = pendingDrag;
  activeDrag = payload;
  el.classList.add("dragging");
  document.body.classList.add("dragging-palette");
  const ghost = ensureGhost();
  // Mirror the palette tile so the ghost reads as "this thing you grabbed".
  const glyph = el.querySelector(".pi-icon")?.innerHTML
    || `<span class="drag-ghost-glyph">${el.querySelector(".pi-glyph")?.textContent ?? "▭"}</span>`;
  ghost.innerHTML = `<span class="drag-ghost-icon">${glyph}</span>` +
    `<span class="drag-ghost-label">${payload.label || "Node"}</span>`;
  ghost.classList.add("show");
  updateDragVisuals(clientX, clientY);
}

// Tear down all drag visuals (called on drop or cancel).
function endPaletteDrag() {
  if (dragGhost) dragGhost.classList.remove("show", "over-canvas");
  if (dropIndicator) dropIndicator.classList.remove("show");
  viewport.classList.remove("drag-over");
  document.body.classList.remove("dragging-palette");
  for (const el of palette.querySelectorAll(".dragging")) el.classList.remove("dragging");
  pendingDrag = null;
  activeDrag = null;
}

// Complete a drop: append the source, render, position the node, confirm.
function finishDropAt(clientX, clientY) {
  const payload = activeDrag;
  if (!payload) return;
  appendToDiagram(payload.text);
  doRender(); // render now so the new node exists for positioning
  if (payload.name) placeDroppedNodeAt(payload.name, clientX, clientY);
  showToast(`Added ${payload.label || "node"} to the canvas`);
}

palette.addEventListener("pointerdown", (e) => {
  if (e.button !== 0) return;
  const el = e.target.closest?.(".palette-item[data-snip], .palette-icon[data-infra]");
  if (!el) return;
  const payload = dropPayload(el);
  if (!payload) return;
  pendingDrag = { el, payload, sx: e.clientX, sy: e.clientY, pointerId: e.pointerId };
});

window.addEventListener("pointermove", (e) => {
  if (!pendingDrag) return;
  if (!activeDrag) {
    // Still waiting to cross the movement threshold.
    if (Math.hypot(e.clientX - pendingDrag.sx, e.clientY - pendingDrag.sy) < DRAG_THRESHOLD) return;
    beginPaletteDrag(e.clientX, e.clientY);
  }
  updateDragVisuals(e.clientX, e.clientY);
  e.preventDefault();
});

window.addEventListener("pointerup", (e) => {
  if (!pendingDrag) return;
  const wasDragging = !!activeDrag;
  const overCanvas = pointInViewport(e.clientX, e.clientY);
  if (wasDragging && overCanvas) finishDropAt(e.clientX, e.clientY);
  endPaletteDrag();
});

// Esc cancels an in-flight drag without dropping.
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && (pendingDrag || activeDrag)) endPaletteDrag();
});

// Suppress the browser's native drag image (we draw our own ghost).
palette.addEventListener("dragstart", (e) => e.preventDefault());
$("palette-toggle").addEventListener("click", () => {
  const collapsed = palette.classList.toggle("collapsed");
  $("palette-toggle").setAttribute("aria-expanded", String(!collapsed));
  $("palette-toggle").textContent = collapsed ? "⊞" : "⊟";
  localStorage.setItem("layra-palette-collapsed", collapsed ? "1" : "0");
});
// Collapse the palette if the user previously collapsed it, or by default on
// narrow viewports (where an expanded palette would overlay the editor).
const palettePref = localStorage.getItem("layra-palette-collapsed");
if (palettePref === "1" || (palettePref === null && window.innerWidth <= 760)) {
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
  download("diagram.svg", new Blob([themedSvg(lastGoodSvg)], { type: "image/svg+xml" }));
}

// Bake the active theme into an SVG string so a shared/exported diagram keeps
// its brand look: override the root font-family, repaint the full-bleed
// background rect with the theme background, and stamp the theme onto the root
// as `data-layra-theme` so the playground can restore it on import.
function themedSvg(svgText) {
  const t = resolveTheme();
  const doc = svgParser.parseFromString(svgText, "image/svg+xml");
  const svg = doc.documentElement;
  if (svg.nodeName !== "svg") return svgText; // parse failure: leave untouched
  svg.setAttribute("font-family", t.font);
  svg.setAttribute("data-layra-theme",
    JSON.stringify({ id: t.id, base: t.base, accent: t.accent, font: t.fontKey, bg: t.bg }));
  // The engine emits one full-bleed background rect (matching the viewBox) as
  // the first <rect>. Repaint it so brand themes export their background.
  const vb = svg.viewBox.baseVal;
  for (const rect of svg.querySelectorAll("rect")) {
    const w = parseFloat(rect.getAttribute("width"));
    const h = parseFloat(rect.getAttribute("height"));
    if (vb && Math.abs(w - vb.width) < 1.5 && Math.abs(h - vb.height) < 1.5) {
      rect.setAttribute("fill", t.bg);
      break;
    }
  }
  return new XMLSerializer().serializeToString(svg);
}

// Rasterize the current SVG to a canvas at the given scale, then hand the
// resulting blob to `onBlob`. Shared by PNG download and clipboard copy.
function rasterizePng(scale, onBlob) {
  if (!lastGoodSvg) return;
  const svgEl = preview.querySelector("svg");
  if (!svgEl) return;
  const w = Math.max(1, Math.round(svgEl.viewBox.baseVal.width * scale));
  const h = Math.max(1, Math.round(svgEl.viewBox.baseVal.height * scale));
  let svgText = themedSvg(lastGoodSvg);
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
// share confirmation, syntax errors, and other lightweight feedback.
let toastTimer = null;

// Lazily create (once) the shared toast element.
function ensureToast() {
  let el = $("toast");
  if (!el) {
    el = document.createElement("div");
    el.id = "toast";
    el.className = "toast";
    el.setAttribute("role", "status");
    el.setAttribute("aria-live", "polite");
    document.body.appendChild(el);
  }
  return el;
}

function showToast(message, ms = 2400) {
  const el = ensureToast();
  el.onclick = null;
  el.className = "toast";
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
  { id: "theme-editor", title: "Theme & brand…", desc: "Presets + custom accent / font / background", icon: "🎨", run: () => openThemeEditor() },
  { id: "toggle-animate", title: "Animate edges", desc: "March the dashes on edges", hint: "A", icon: "⇝", run: () => toggleAnimateEdges() },
  { id: "examples", title: "Open examples gallery", icon: "✦", run: () => openGallery() },
  { id: "help", title: "Help & keyboard shortcuts", hint: "?", icon: "?", run: () => openHelp() },
  { id: "star", title: "Star Layra on GitHub", icon: "★", run: () => window.open("https://github.com/vietgs03/layra", "_blank", "noopener") },
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

// Hide the WASM loading overlay (fade out, then remove from the DOM so it
// never traps focus or intercepts pointer events).
function hideBoot() {
  const boot = $("boot");
  if (!boot) return;
  boot.classList.add("hide");
  setTimeout(() => boot.remove(), 320);
}

// If the engine fails to initialize, keep the overlay but show a clear,
// actionable error instead of leaving a silent spinner.
function bootError(err) {
  const boot = $("boot");
  if (!boot) return;
  boot.classList.remove("hide");
  boot.innerHTML =
    `<div class="boot-inner">` +
    `<span class="boot-logo" aria-hidden="true">⚠</span>` +
    `<span class="boot-text">Couldn't start the diagram engine.</span>` +
    `<span class="boot-text" style="font-size:11px">${String(err?.message ?? err)}</span>` +
    `<button class="empty-cta" onclick="location.reload()">Reload</button>` +
    `</div>`;
}

async function main() {
  try {
    await init();
  } catch (e) {
    console.error("layra: WASM init failed", e);
    bootError(e);
    return;
  }

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
  // Engine is ready and the first frame is painted: drop the loading overlay.
  hideBoot();

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

  $("btn-theme").addEventListener("click", openThemeEditor);
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
    else if (e.key === "d" || e.key === "D") toggleTheme();
    else if (e.key === "a" || e.key === "A") $("btn-animate").click();
    else if (e.key === "?") { e.preventDefault(); openHelp(); }
    // Arrow-key panning when the canvas itself is focused (keyboard a11y).
    else if (document.activeElement === viewport &&
             ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
      e.preventDefault();
      const step = e.shiftKey ? 120 : 40;
      if (e.key === "ArrowUp") view.y += step;
      else if (e.key === "ArrowDown") view.y -= step;
      else if (e.key === "ArrowLeft") view.x += step;
      else if (e.key === "ArrowRight") view.x -= step;
      userTouchedView = true;
      applyView();
    }
  });
}

main();
