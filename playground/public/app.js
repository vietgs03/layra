// Layra playground: editor left, live preview right.
// Renders on every input event — the engine is fast enough that the only
// throttle we need is requestAnimationFrame coalescing.

import init, { render, load_icons } from "./pkg/layra_wasm.js";

const DEFAULT_SOURCE = `flowchart LR
  laptop["{icon:mdi:laptop} Your laptop\\n192.168.1.42:51000"]:::client
  router["{icon:mdi:router-wireless} Router (NAT)\\n translation table"]:::highlight
  target["{icon:mdi:web} example.com\\n93.184.216.34:443"]:::external

  laptop -->|outbound| router
  router ==>|rewritten src| target
  target -.->|reply| router
  router -.->|rewritten dst| laptop
`;

const AUTOSAVE_KEY = "layra-playground-source";

const $ = (id) => document.getElementById(id);
const editor = $("editor");
const preview = $("preview");
const status = $("status");
const perf = $("perf");

let dark = matchMedia("(prefers-color-scheme: dark)").matches;
let lastGoodSvg = "";
let rafPending = false;

// --- base64url helpers (chunked: spread blows the arg limit past ~64k) ---
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

// --- URL hash <-> source. Deflate-compressed (prefix "c:") with plain
// base64url fallback for older links/browsers. ---
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

function applyTheme() {
  document.documentElement.classList.toggle("dark", dark);
}

function reportError(message) {
  status.textContent = message;
  status.className = "status err";
  // Highlight the offending line if the engine reported one.
  const m = /line (\d+)/.exec(message);
  highlightLine(m ? Number(m[1]) : null);
}

// Crude but effective gutter: tint the editor background at the error line.
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
  const t0 = performance.now();
  try {
    const svg = render(src, dark);
    const dt = performance.now() - t0;
    perf.textContent = dt < 1 ? `${(dt * 1000).toFixed(0)} µs` : `${dt.toFixed(1)} ms`;
    lastGoodSvg = svg;
    swapSvg(svg);
    status.textContent = "ok";
    status.className = "status ok";
    highlightLine(null);
  } catch (e) {
    // Keep last good render on screen; surface the error instead.
    reportError(String(e.message ?? e));
  }
}

// Parse into a detached document, then swap nodes inside a persistent <svg>
// root — keeps element identity stable so pan/zoom CSS transforms survive.
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
  // Sync attributes both ways: set new ones, drop stale ones.
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
  const scale = 2; // 2x for crisp output
  const w = Math.max(1, Math.round(svgEl.viewBox.baseVal.width * scale));
  const h = Math.max(1, Math.round(svgEl.viewBox.baseVal.height * scale));
  // Firefox rasterizes blob-SVGs without intrinsic size as empty; the
  // renderer emits width/height, but enforce it defensively anyway.
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
    // Clipboard API needs HTTPS + focus; fall back to a manual prompt.
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
  scheduleRender();
  return true;
}

async function main() {
  await init();

  // Icon pack (extracted from blog.viethx.com usage): non-blocking-critical,
  // but await it so the first render already has icons.
  try {
    const res = await fetch("./icons-blog.json");
    if (res.ok) {
      const n = load_icons(await res.text());
      console.info(`layra: ${n} icons loaded`);
    }
  } catch (e) {
    console.warn("layra: icon pack failed to load", e);
  }

  const fromHash = await loadFromHash();
  if (!fromHash) {
    editor.value = localStorage.getItem(AUTOSAVE_KEY) ?? DEFAULT_SOURCE;
  }
  applyTheme();
  doRender();

  editor.addEventListener("input", scheduleRender);
  editor.addEventListener("scroll", () => {
    if (status.classList.contains("err")) {
      const m = /line (\d+)/.exec(status.textContent);
      highlightLine(m ? Number(m[1]) : null);
    }
  });
  window.addEventListener("hashchange", loadFromHash);

  $("btn-theme").addEventListener("click", () => {
    dark = !dark;
    applyTheme();
    scheduleRender();
  });
  $("btn-share").addEventListener("click", shareLink);
  $("btn-svg").addEventListener("click", exportSvg);
  $("btn-png").addEventListener("click", exportPng);

  // Tab indents, Shift+Tab outdents, Escape blurs (a11y: no keyboard trap).
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
}

main();
