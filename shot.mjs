// Usage: node shot.mjs <outfile.png> [dark] [extraJsFile]
import { chromium } from "/home/viethx/.nvm/versions/node/v22.22.2/lib/node_modules/playwright/index.mjs";

const out = process.argv[2] || "/tmp/shot.png";
const dark = process.argv[3] === "dark";
const extraJs = process.argv[4];

const browser = await chromium.launch();
const page = await browser.newPage({
  viewport: { width: 1400, height: 860 },
  colorScheme: dark ? "dark" : "light",
  deviceScaleFactor: 2,
});
const errors = [];
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
page.on("pageerror", (e) => errors.push(String(e)));

await page.goto("http://localhost:8951/", { waitUntil: "networkidle" });
await page.waitForSelector("#preview svg", { timeout: 8000 }).catch(() => {});
await page.waitForTimeout(700);

if (extraJs) {
  const fs = await import("node:fs");
  const code = fs.readFileSync(extraJs, "utf8");
  await page.evaluate(code);
  await page.waitForTimeout(500);
}

await page.screenshot({ path: out });
console.log("SHOT:", out, dark ? "(dark)" : "(light)");
if (errors.length) console.log("CONSOLE ERRORS:\n" + errors.join("\n"));
else console.log("no console errors");

await browser.close();
