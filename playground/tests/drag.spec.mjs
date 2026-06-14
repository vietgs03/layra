#!/usr/bin/env node
// Regression test for U16: real pointer drag from the icon palette onto the
// canvas must insert a node into the editor and show a confirmation toast.
//
// Run: node playground/tests/drag.spec.mjs
// Requires: playground/dist built (./playground/build.sh) and a local
// Playwright install. Serves dist on an ephemeral port, drives a real
// mouse drag (not a synthetic DataTransfer), and asserts the editor text
// grew and a toast appeared. Exits non-zero on failure.

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";

const DIST = join(fileURLToPath(new URL(".", import.meta.url)), "..", "dist");
const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".wasm": "application/wasm",
  ".svg": "image/svg+xml",
  ".png": "image/png",
};

const server = createServer(async (req, res) => {
  try {
    let p = decodeURIComponent(req.url.split("?")[0]);
    if (p === "/") p = "/index.html";
    const buf = await readFile(join(DIST, p));
    res.writeHead(200, { "Content-Type": TYPES[extname(p)] || "application/octet-stream" });
    res.end(buf);
  } catch {
    res.writeHead(404);
    res.end("not found");
  }
});

function locatePlaywright() {
  // Prefer a local node_modules; fall back to the known global path.
  const candidates = [
    "playwright",
    "/home/viethx/.nvm/versions/node/v22.22.2/lib/node_modules/playwright/index.mjs",
  ];
  return candidates;
}

async function importChromium() {
  for (const c of locatePlaywright()) {
    try {
      const mod = await import(c);
      if (mod.chromium) return mod.chromium;
    } catch {
      /* try next */
    }
  }
  throw new Error("playwright not found");
}

async function main() {
  const chromium = await importChromium();
  await new Promise((r) => server.listen(0, r));
  const port = server.address().port;
  const url = `http://127.0.0.1:${port}/`;

  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1400, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));

  await page.goto(url);
  // Leave the empty-state via the examples gallery if present.
  await page.waitForTimeout(1500);
  const thumb = await page.$(".gallery-thumb");
  if (thumb) await thumb.click();
  await page.waitForTimeout(800);

  // Find a draggable palette item and the canvas.
  const item = await page.$('[draggable="true"], .palette-item, .palette-icon');
  const canvas = await page.$("#viewport, #preview, .preview-pane");
  if (!item || !canvas) throw new Error("palette item or canvas not found");

  const editorValueBefore = await page.$eval("#editor, textarea", (el) => el.value);

  // Real mouse drag: down on the item, move in steps to the canvas, up.
  const ib = await item.boundingBox();
  const cb = await canvas.boundingBox();
  await page.mouse.move(ib.x + ib.width / 2, ib.y + ib.height / 2);
  await page.mouse.down();
  for (let i = 1; i <= 8; i++) {
    await page.mouse.move(
      ib.x + (cb.x + cb.width / 2 - ib.x) * (i / 8),
      ib.y + (cb.y + cb.height / 2 - ib.y) * (i / 8),
      { steps: 2 },
    );
  }
  await page.mouse.up();
  await page.waitForTimeout(500);

  const editorValueAfter = await page.$eval("#editor, textarea", (el) => el.value);
  const toast = await page.evaluate(() => {
    const t = document.querySelector('[class*="toast"]');
    return t ? t.textContent.trim() : null;
  });

  const grew = editorValueAfter.length > editorValueBefore.length;
  const ok = grew && !!toast && errors.length === 0;

  console.log("editor grew:", grew, `(+${editorValueAfter.length - editorValueBefore.length} chars)`);
  console.log("toast:", toast || "(none)");
  console.log("console errors:", errors.length ? errors.slice(0, 3) : "none");

  await browser.close();
  server.close();

  if (!ok) {
    console.error("DRAG TEST FAILED");
    process.exit(1);
  }
  console.log("DRAG TEST PASSED");
}

main().catch((e) => {
  console.error("DRAG TEST ERROR:", e.message);
  server.close();
  process.exit(1);
});
