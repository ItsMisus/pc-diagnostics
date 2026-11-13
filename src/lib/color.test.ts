import { describe, expect, it } from "vitest";
import { lastPoint } from "@/components/charts/scale";
import { composite, contrastRatio, parseColor, toHex } from "./color";
import { resolveTheme } from "./theme";

describe("parseColor", () => {
  it("reads hex colors", () => {
    expect(parseColor("#fff")).toEqual({ r: 255, g: 255, b: 255, a: 1 });
    expect(parseColor("#0a6be0")).toEqual({ r: 10, g: 107, b: 224, a: 1 });
    expect(parseColor("#00000080")?.a).toBeCloseTo(0.5, 2);
  });

  it("reads rgb() in both syntaxes", () => {
    expect(parseColor("rgb(16 24 40 / 0.1)")).toEqual({ r: 16, g: 24, b: 40, a: 0.1 });
    expect(parseColor("rgb(1, 2, 3)")).toEqual({ r: 1, g: 2, b: 3, a: 1 });
    expect(parseColor("rgba(1, 2, 3, 0.5)")).toEqual({ r: 1, g: 2, b: 3, a: 0.5 });
    expect(parseColor(" rgb(1 2 3 / 40%) ")?.a).toBeCloseTo(0.4);
  });

  it("returns null for non-colors", () => {
    expect(parseColor("radial-gradient(red, blue)")).toBeNull();
    expect(parseColor("var(--fg)")).toBeNull();
    expect(parseColor("#12")).toBeNull();
    expect(parseColor("rgb(a b c)")).toBeNull();
  });
});

describe("contrast", () => {
  it("matches the WCAG reference values", () => {
    expect(contrastRatio(parseColor("#000")!, parseColor("#fff")!)).toBeCloseTo(21, 5);
    expect(contrastRatio(parseColor("#777")!, parseColor("#fff")!)).toBeCloseTo(4.48, 2);
  });

  it("composites a translucent foreground first", () => {
    const half = composite({ r: 0, g: 0, b: 0, a: 0.5 }, { r: 255, g: 255, b: 255, a: 1 });
    expect(toHex(half)).toBe("#808080");
    expect(contrastRatio({ r: 0, g: 0, b: 0, a: 0 }, parseColor("#fff")!)).toBeCloseTo(1, 5);
  });
});

describe("resolveTheme", () => {
  it("lets an explicit preference win", () => {
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  it("follows the OS for system", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });
});

describe("Sparkline lastPoint", () => {
  it("finds the end of the last subpath", () => {
    expect(lastPoint("M0.0 10.0 L5.0 4.5 M8.0 2.0 L12.5 3.0")).toEqual([12.5, 3]);
    expect(lastPoint("")).toBeNull();
  });
});
