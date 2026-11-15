import { describe, expect, it } from "vitest";
import { averageColor, hsvToRgb, mixColor, parseChannel, parseHex, pushRecent, RECENT_MAX, rgbToHsv, stepHue, stepSv, svFromPoint, toHex } from "./colorMath";

describe("color conversions", () => {
  it("round-trips RGB through HSV", () => {
    for (let r = 0; r <= 255; r += 51)
      for (let g = 0; g <= 255; g += 51)
        for (let b = 0; b <= 255; b += 51) {
          expect(hsvToRgb(rgbToHsv({ r, g, b }))).toEqual({ r, g, b });
        }
  });

  it("knows the primaries", () => {
    expect(rgbToHsv({ r: 255, g: 0, b: 0 })).toEqual({ h: 0, s: 1, v: 1 });
    expect(rgbToHsv({ r: 0, g: 255, b: 0 }).h).toBeCloseTo(1 / 3);
    expect(rgbToHsv({ r: 0, g: 0, b: 0 })).toEqual({ h: 0, s: 0, v: 0 });
    expect(hsvToRgb({ h: 2 / 3, s: 1, v: 1 })).toEqual({ r: 0, g: 0, b: 255 });
  });

  it("parses and prints hex", () => {
    expect(parseHex("#4EA5FF")).toEqual({ r: 78, g: 165, b: 255 });
    expect(parseHex("fa0")).toEqual({ r: 255, g: 170, b: 0 });
    expect(parseHex(" #000000 ")).toEqual({ r: 0, g: 0, b: 0 });
    for (const bad of ["", "#12", "#12345", "#ggg000", "rgb(1,2,3)"]) expect(parseHex(bad)).toBeNull();
    expect(toHex({ r: 78, g: 165, b: 255 })).toBe("#4ea5ff");
    expect(parseChannel("255")).toBe(255);
    expect(parseChannel("256")).toBeNull();
    expect(parseChannel("-1")).toBeNull();
    expect(parseChannel("1.5")).toBeNull();
  });
});

describe("picker logic", () => {
  it("keeps recent colors unique and bounded", () => {
    let recent = pushRecent([], { r: 1, g: 1, b: 1 });
    recent = pushRecent(recent, { r: 2, g: 2, b: 2 });
    recent = pushRecent(recent, { r: 1, g: 1, b: 1 });
    expect(recent).toEqual([
      { r: 1, g: 1, b: 1 },
      { r: 2, g: 2, b: 2 },
    ]);
    for (let i = 0; i < 20; i++) recent = pushRecent(recent, { r: i, g: 0, b: 0 });
    expect(recent).toHaveLength(RECENT_MAX);
    expect(recent[0]).toEqual({ r: 19, g: 0, b: 0 });
  });

  it("moves on the square and the hue slider with the keyboard", () => {
    const hsv = { h: 0.5, s: 0.5, v: 0.5 };
    expect(stepSv(hsv, "ArrowRight", false)?.s).toBeCloseTo(0.51);
    expect(stepSv(hsv, "ArrowUp", true)?.v).toBeCloseTo(0.6);
    expect(stepSv({ ...hsv, s: 0.995 }, "ArrowRight", true)?.s).toBe(1);
    expect(stepSv(hsv, "Home", false)?.s).toBe(0);
    expect(stepSv(hsv, "PageDown", false)?.v).toBe(0);
    expect(stepSv(hsv, "a", false)).toBeNull();
    expect(stepHue(0, "ArrowLeft", false)).toBeCloseTo(359 / 360);
    expect(stepHue(0.99, "ArrowRight", true)).toBeCloseTo(0.99 + 15 / 360 - 1);
    expect(stepHue(0.3, "Tab", false)).toBeNull();
  });

  it("maps pointer positions and mixes colors", () => {
    expect(svFromPoint(1.2, -0.1)).toEqual({ s: 1, v: 1 });
    expect(svFromPoint(0.25, 0.75)).toEqual({ s: 0.25, v: 0.25 });
    expect(mixColor({ r: 0, g: 0, b: 0 }, { r: 200, g: 100, b: 50 }, 0.5)).toEqual({ r: 100, g: 50, b: 25 });
    expect(averageColor([])).toEqual({ r: 0, g: 0, b: 0 });
    expect(averageColor([{ r: 255, g: 0, b: 0 }, { r: 0, g: 0, b: 255 }])).toEqual({ r: 128, g: 0, b: 128 });
  });
});
