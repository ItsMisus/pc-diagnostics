import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { contrastRatio, parseColor, type Rgba } from "../lib/color";
import { duration } from "../lib/motion";

// Read from disk: Vite transforms CSS during tests, so a `?raw` import would come back empty.
// Type-checked by tsconfig.node.json (Node types), not by the app's tsconfig.
const css = readFileSync(new URL("./index.css", import.meta.url), "utf8");
/** The stylesheet without comments, for checks on the declarations themselves. */
const code = css.replace(/\/\*[\s\S]*?\*\//g, "");

/** Custom properties declared in the first block that follows `selector`. */
function tokens(selector: string): Map<string, string> {
  const start = css.indexOf(selector);
  expect(start, `${selector} not found in index.css`).toBeGreaterThanOrEqual(0);
  const open = css.indexOf("{", start);
  const body = css.slice(open + 1, css.indexOf("}", open));
  // Multi-line values (gradients) are compared with their whitespace collapsed.
  return new Map([...body.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((m) => [m[1]!, m[2]!.replace(/\s+/g, " ").trim()]));
}

const light = tokens(":root {");
const darkOs = tokens(':root:not([data-theme="light"]) {');
const darkExplicit = tokens(':root[data-theme="dark"] {');

describe("theme tokens", () => {
  it("declares the dark palette identically for the OS preference and the explicit choice", () => {
    expect(darkExplicit).toEqual(darkOs);
  });

  it("defines every light token in the dark palette too", () => {
    expect([...darkOs.keys()].sort()).toEqual([...light.keys()].sort());
  });

  it("does not use light-dark(), which the WebView targets lack", () => {
    expect(code).not.toMatch(/light-dark\(/);
  });

  it("pairs every unprefixed backdrop-filter with the -webkit- one (WKWebView)", () => {
    const plain = code.match(/(?<!-webkit-)backdrop-filter:/g)?.length ?? 0;
    const prefixed = code.match(/-webkit-backdrop-filter:/g)?.length ?? 0;
    expect(prefixed).toBe(plain);
  });
});

/** Opaque color of a token in a theme; fails the test when it is not a plain color. */
function color(theme: Map<string, string>, name: string): Rgba {
  const value = theme.get(name);
  const parsed = value ? parseColor(value) : null;
  expect(parsed, `${name} is not a parseable color (${value})`).not.toBeNull();
  return parsed!;
}

describe("text contrast (WCAG AA)", () => {
  // Text tokens on the surfaces they are used on.
  const textOnSurfaces: [string, string[], number][] = [
    ["--fg", ["--canvas", "--surface", "--surface-raised", "--surface-2", "--surface-3"], 4.5],
    ["--fg-muted", ["--canvas", "--surface", "--surface-raised", "--surface-2"], 4.5],
    ["--fg-subtle", ["--surface", "--surface-raised", "--surface-2"], 4.5],
    ["--accent-text", ["--surface", "--surface-raised", "--surface-2"], 4.5],
    ["--ok", ["--surface", "--surface-raised"], 4.5],
    ["--warn", ["--surface", "--surface-raised"], 4.5],
    ["--bad", ["--surface", "--surface-raised"], 4.5],
  ];

  for (const [name, theme] of [
    ["light", light],
    ["dark", darkOs],
  ] as const) {
    it(`keeps text readable in the ${name} theme`, () => {
      for (const [fg, bgs, min] of textOnSurfaces) {
        for (const bg of bgs) {
          const ratio = contrastRatio(color(theme, fg), color(theme, bg));
          expect(ratio, `${fg} on ${bg} (${name}): ${ratio.toFixed(2)}`).toBeGreaterThanOrEqual(min);
        }
      }
    });

    it(`keeps the primary button label readable in the ${name} theme`, () => {
      const ratio = contrastRatio(color(theme, "--accent-fg"), color(theme, "--accent-solid"));
      expect(ratio).toBeGreaterThanOrEqual(4.5);
    });
  }
});

describe("motion tokens", () => {
  it("mirrors the CSS durations in lib/motion.ts", () => {
    const root = tokens(":root {\n  --dur-");
    const pairs: [string, number][] = [
      ["--dur-instant", duration.instant],
      ["--dur-micro", duration.micro],
      ["--dur-fast", duration.fast],
      ["--dur-component", duration.component],
      ["--dur-panel", duration.panel],
      ["--dur-nav", duration.nav],
    ];
    for (const [name, seconds] of pairs) {
      expect(root.get(name), name).toBe(`${Math.round(seconds * 1000)}ms`);
    }
  });

  it("keeps control feedback and panel transitions within the documented ranges", () => {
    expect(duration.micro).toBeGreaterThanOrEqual(0.1);
    expect(duration.fast).toBeLessThanOrEqual(0.2);
    expect(duration.component).toBeGreaterThanOrEqual(0.2);
    expect(duration.panel).toBeLessThanOrEqual(0.35);
    expect(duration.exit).toBeLessThan(duration.component);
  });
});
