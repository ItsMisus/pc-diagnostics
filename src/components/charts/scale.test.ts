import { describe, expect, it } from "vitest";
import { linear, linePath, nearestIndex, ticks, areaPath } from "./scale";

describe("linear", () => {
  it("maps endpoints and midpoints", () => {
    const f = linear([0, 100], [0, 200]);
    expect(f(0)).toBe(0);
    expect(f(50)).toBe(100);
    expect(f(100)).toBe(200);
  });

  it("supports inverted ranges (SVG y axis)", () => {
    const f = linear([0, 100], [80, 0]);
    expect(f(0)).toBe(80);
    expect(f(100)).toBe(0);
  });

  it("handles degenerate domains", () => {
    expect(linear([5, 5], [0, 10])(5)).toBe(5);
  });
});

describe("linePath", () => {
  it("draws a continuous line", () => {
    const d = linePath(
      [
        { t: 0, v: 0 },
        { t: 10, v: 100 },
      ],
      [0, 10],
      [0, 100],
      100,
      50,
    );
    expect(d).toBe("M0.0 50.0 L100.0 0.0");
  });

  it("breaks the line on gaps instead of interpolating", () => {
    const d = linePath(
      [
        { t: 0, v: 10 },
        { t: 1, v: null },
        { t: 2, v: 10 },
      ],
      [0, 2],
      [0, 100],
      100,
      100,
    );
    expect(d.match(/M/g)).toHaveLength(2);
    expect(d).not.toContain("L");
  });

  it("clamps out-of-domain values to the plot edge", () => {
    const d = linePath([{ t: 0, v: 150 }], [0, 1], [0, 100], 100, 100);
    expect(d).toBe("M0.0 0.0");
  });

  it("returns an empty path for no data", () => {
    expect(linePath([], [0, 1], [0, 1], 10, 10)).toBe("");
  });
});

describe("nearestIndex", () => {
  const pts = [0, 10, 20, 30].map((t) => ({ t, v: t }));

  it("finds the closest sample", () => {
    expect(nearestIndex(pts, -5)).toBe(0);
    expect(nearestIndex(pts, 14)).toBe(1);
    expect(nearestIndex(pts, 16)).toBe(2);
    expect(nearestIndex(pts, 99)).toBe(3);
  });

  it("returns -1 when empty", () => {
    expect(nearestIndex([], 1)).toBe(-1);
  });
});

describe("ticks", () => {
  it("is inclusive and evenly spaced", () => {
    expect(ticks([0, 100], 5)).toEqual([0, 25, 50, 75, 100]);
  });
});

describe("areaPath", () => {
  it("closes each subpath down to the baseline", () => {
    expect(areaPath("M0.0 50.0 L10.0 40.0", 100)).toBe("M0.0 50.0 L10.0 40.0 L10.0 100.0 L0.0 100.0 Z");
  });

  it("keeps gaps as separate shapes", () => {
    const line = linePath(
      [
        { t: 0, v: 0 },
        { t: 1, v: 1 },
        { t: 2, v: null },
        { t: 3, v: 1 },
      ],
      [0, 3],
      [0, 1],
      30,
      10,
    );
    const area = areaPath(line, 10);
    expect(area.match(/Z/g)).toHaveLength(2);
    expect(area).toContain("M30.0 0.0 L30.0 10.0 L30.0 10.0 Z");
  });

  it("is empty for an empty line", () => {
    expect(areaPath("", 10)).toBe("");
  });
});
