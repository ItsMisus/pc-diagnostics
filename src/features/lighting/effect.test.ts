import { describe, expect, it } from "vitest";
import type { LightingEffect } from "@/types/generated/LightingEffect";
import { colorAt, cycleMs, defaultPosition, dim, effectColors, isDynamic, levelAt, monochromeCompatible } from "./effect";
import vectors from "./effect-vectors.json";

interface Vector {
  effect: LightingEffect;
  t: number;
  index: number;
  count: number;
  position: number | null;
  expected: [number, number, number];
}

describe("effect math (shared with the Rust engine)", () => {
  it("matches every shared vector exactly", () => {
    const list = vectors as unknown as Vector[];
    expect(list.length).toBeGreaterThanOrEqual(30);
    for (const v of list) {
      const c = colorAt(v.effect, v.t, v.index, v.count, v.position);
      expect([c.r, c.g, c.b], JSON.stringify(v)).toEqual(v.expected);
    }
  });

  it("covers every effect kind in the vectors", () => {
    const kinds = new Set((vectors as unknown as Vector[]).map((v) => v.effect.kind));
    for (const k of ["off", "static", "breathing", "rainbow", "wave", "gradient", "colorCycle"]) expect(kinds.has(k as LightingEffect["kind"])).toBe(true);
  });

  it("breathes from black to the color and back", () => {
    const red = { r: 255, g: 0, b: 0 };
    const b: LightingEffect = { kind: "breathing", color: red, period: 4000 };
    expect(colorAt(b, 0, 0, 1)).toEqual({ r: 0, g: 0, b: 0 });
    expect(colorAt(b, 2000, 0, 1)).toEqual(red);
    expect(levelAt(b, 2000)).toBeCloseTo(1);
    expect(levelAt({ kind: "off" }, 0)).toBe(0);
    expect(levelAt({ kind: "rainbow", speed: 2 }, 0)).toBe(1);
  });

  it("helpers", () => {
    expect(cycleMs(0)).toBe(12_000);
    expect(cycleMs(10)).toBe(1200);
    expect(defaultPosition(0, 1)).toBe(0.5);
    expect(defaultPosition(2, 3)).toBe(1);
    expect(dim({ r: 200, g: 100, b: 0 }, 50)).toEqual({ r: 100, g: 50, b: 0 });
    expect(isDynamic({ kind: "static", color: { r: 1, g: 2, b: 3 } })).toBe(false);
    expect(isDynamic({ kind: "colorCycle", colors: [], speed: 1 })).toBe(true);
    expect(monochromeCompatible({ kind: "wave", colors: [], speed: 1, direction: "leftToRight" })).toBe(false);
    expect(effectColors({ kind: "rainbow", speed: 1 })).toHaveLength(6);
  });
});
