// Smoke test: the published package surface, exactly as a consumer uses it.
import { render, renderLenient, layout, loadIcons } from "../dist/index.js";
import assert from "node:assert/strict";

// 1. Basic render across several diagram types.
for (const src of [
  "flowchart LR\n  a[Start] --> b{Decision} --> c[End]",
  "sequenceDiagram\n  A->>B: hello",
  "pie\n  \"x\" : 60\n  \"y\" : 40",
  "gantt\n  dateFormat YYYY-MM-DD\n  section S\n  T :2026-01-01, 5d",
]) {
  const svg = await render(src);
  assert.ok(svg.startsWith("<svg"), `bad svg for ${src.slice(0, 20)}`);
  assert.ok(svg.endsWith("</svg>"));
}

// 2. Dark theme changes output.
const light = await render("flowchart LR\n a --> b");
const dark = await render("flowchart LR\n a --> b", { dark: true });
assert.notEqual(light, dark);

// 3. Lenient mode reports warnings without failing.
const { svg, warnings } = await renderLenient(
  "flowchart LR\n  a --> b\n  ((((broken"
);
assert.ok(svg.includes("<svg"));
assert.equal(warnings.length, 1);
assert.match(warnings[0], /line 3/);

// 4. Structured layout output.
const doc = await layout("flowchart LR\n  a --> b");
assert.equal(doc.kind, "graph");
assert.equal(doc.graph.nodes.length, 2);
assert.ok(doc.graph.nodes[0].rect.width > 0);

// 5. Icon loading + usage.
const n = await loadIcons({
  icons: {
    "mdi:test": { body: '<path d="M0 0h24v24H0z"/>', width: 24, height: 24 },
  },
});
assert.equal(n, 1);
const withIcon = await render('flowchart LR\n  a["{icon:mdi:test} Hi"]');
assert.ok(withIcon.includes('viewBox="0 0 24 24"'), "icon must be inlined");

// 6. Errors carry line numbers.
await assert.rejects(() => render("flowchart LR\nend"), /line 2/);

console.log("smoke: 6/6 passed");
