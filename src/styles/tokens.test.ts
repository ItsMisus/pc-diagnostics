import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vite transforms CSS during tests, so a `?raw` import would come back empty.
// Type-checked by tsconfig.node.json (Node types), not by the app's tsconfig.
const css = readFileSync(new URL("./index.css", import.meta.url), "utf8");

/** Custom properties declared in the first block that follows `selector`. */
function tokens(selector: string): Map<string, string> {
  const start = css.indexOf(selector);
  expect(start, `${selector} not found in index.css`).toBeGreaterThanOrEqual(0);
  const open = css.indexOf("{", start);
  const body = css.slice(open + 1, css.indexOf("}", open));
  return new Map([...body.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((m) => [m[1]!, m[2]!.trim()]));
}

describe("theme tokens", () => {
  const light = tokens(":root {");
  const darkOs = tokens(':root:not([data-theme="light"]) {');
  const darkExplicit = tokens(':root[data-theme="dark"] {');

  it("declares the dark palette identically for the OS preference and the explicit choice", () => {
    expect(darkExplicit).toEqual(darkOs);
  });

  it("defines every light token in the dark palette too", () => {
    expect([...darkOs.keys()].sort()).toEqual([...light.keys()].sort());
  });
});
