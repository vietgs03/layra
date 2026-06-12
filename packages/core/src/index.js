/**
 * @vietgs03/layra — Mermaid-compatible diagram renderer (Rust → WASM).
 *
 * Works in Node ≥18 and every modern bundler/browser. The WASM module is
 * loaded once, lazily, on the first call; all subsequent calls are sync-fast
 * (~100µs per diagram).
 *
 * ```js
 * import { render } from "@vietgs03/layra";
 * const svg = await render("flowchart LR\n  a --> b");
 * ```
 */

import initWasm, {
  render as wasmRender,
  render_lenient as wasmRenderLenient,
  layout_json as wasmLayoutJson,
  load_icons as wasmLoadIcons,
} from "./wasm/layra_wasm.js";

/** @type {Promise<void> | null} */
let ready = null;

async function init() {
  if (!ready) {
    ready = (async () => {
      if (typeof window === "undefined" && typeof process !== "undefined") {
        // Node: feed wasm-bindgen the bytes directly (no fetch in Node).
        const { readFile } = await import("node:fs/promises");
        const url = new URL("./wasm/layra_wasm_bg.wasm", import.meta.url);
        await initWasm({ module_or_path: await readFile(url) });
      } else {
        await initWasm();
      }
    })();
  }
  return ready;
}

/**
 * Render diagram source to an SVG string. Throws on the first parse error
 * (message carries the line number).
 *
 * @param {string} source - Mermaid-compatible diagram text.
 * @param {{ dark?: boolean }} [options]
 * @returns {Promise<string>} standalone SVG markup
 */
export async function render(source, options = {}) {
  await init();
  return wasmRender(source, options.dark ?? false);
}

/**
 * Lenient render: unparseable lines are skipped and reported instead of
 * failing the whole document. Throws only when nothing could be parsed.
 *
 * @param {string} source
 * @param {{ dark?: boolean }} [options]
 * @returns {Promise<{ svg: string, warnings: string[] }>}
 */
export async function renderLenient(source, options = {}) {
  await init();
  return JSON.parse(wasmRenderLenient(source, options.dark ?? false));
}

/**
 * Run parse + measure + layout + route and return the laid-out document as
 * structured geometry (node rects, edge polylines, label anchors) for
 * custom renderers (Canvas/WebGL/React). See layra-types.ts in the repo.
 *
 * @param {string} source
 * @returns {Promise<object>} discriminated union on `kind`
 */
export async function layout(source) {
  await init();
  return JSON.parse(wasmLayoutJson(source));
}

/**
 * Load an Iconify-format icon pack (`{"icons":{"mdi:laptop":{body,width,
 * height}}}`). Packs merge; call any number of times.
 *
 * @param {object | string} pack - parsed JSON or JSON string
 * @returns {Promise<number>} number of icons added
 */
export async function loadIcons(pack) {
  await init();
  return wasmLoadIcons(typeof pack === "string" ? pack : JSON.stringify(pack));
}
