import { describe, expect, it } from "vitest";
import type { CoolingCurvePoint } from "@/types/generated/CoolingCurvePoint";
import {
  addPoint,
  curvePath,
  dutyAt,
  dutyToY,
  isValidCurve,
  matchingPreset,
  MAX_POINTS,
  movePoint,
  nudgePoint,
  PRESETS,
  removePoint,
  snap,
  tempToX,
  validateCurve,
  xToTemp,
  yToDuty,
} from "./curve";

const c = (...list: [number, number][]): CoolingCurvePoint[] => list.map(([temperatureC, dutyPercent]) => ({ temperatureC, dutyPercent }));

describe("validateCurve", () => {
  it("accepts the presets", () => {
    for (const p of Object.values(PRESETS)) expect(validateCurve(p)).toEqual([]);
  });

  it("reports every rule the Rust side enforces", () => {
    expect(validateCurve(c([30, 10]))).toEqual([{ kind: "count", count: 1 }]);
    expect(validateCurve(c([30, 10], [30, 20]))).toEqual([{ kind: "order", index: 1 }]);
    expect(validateCurve(c([30, 50], [40, 20]))).toEqual([{ kind: "decreasing", index: 1 }]);
    expect(validateCurve(c([10, 0], [40, 20]))).toEqual([{ kind: "range", index: 0 }]);
    expect(validateCurve(c([30, 0], [40, 101]))).toEqual([{ kind: "range", index: 1 }]);
    expect(validateCurve(c([30, Number.NaN], [40, 20]))[0]).toEqual({ kind: "range", index: 0 });
    const nine = Array.from({ length: 9 }, (_, i) => [20 + i * 5, 50] as [number, number]);
    expect(validateCurve(c(...nine))).toEqual([{ kind: "count", count: 9 }]);
    expect(isValidCurve(c([30, 20], [40, 20]))).toBe(true);
  });
});

describe("dutyAt", () => {
  it("interpolates linearly and is flat beyond the ends", () => {
    const p = c([40, 20], [60, 60], [80, 100]);
    expect(dutyAt(p, 30)).toBe(20);
    expect(dutyAt(p, 50)).toBe(40);
    expect(dutyAt(p, 70)).toBe(80);
    expect(dutyAt(p, 95)).toBe(100);
    expect(dutyAt([], 50)).toBeNull();
  });
});

describe("snapping and moves", () => {
  it("snaps to the step", () => {
    expect(snap(41.4, 1)).toBe(41);
    expect(snap(41.6, 1)).toBe(42);
    expect(snap(43, 5)).toBe(45);
  });

  it("keeps a dragged point between its neighbours", () => {
    const p = c([30, 20], [50, 40], [70, 60]);
    expect(movePoint(p, 1, 49.6, 44.4)[1]).toEqual({ temperatureC: 50, dutyPercent: 44 });
    expect(movePoint(p, 1, 90, 90)[1]).toEqual({ temperatureC: 69, dutyPercent: 60 });
    expect(movePoint(p, 1, 0, 0)[1]).toEqual({ temperatureC: 31, dutyPercent: 20 });
    expect(movePoint(p, 0, 5, -10)[0]).toEqual({ temperatureC: 20, dutyPercent: 0 });
    expect(movePoint(p, 2, 140, 140)[2]).toEqual({ temperatureC: 100, dutyPercent: 100 });
    for (const t of [0, 25, 55, 120]) for (const d of [-5, 30, 99]) expect(isValidCurve(movePoint(p, 1, t, d))).toBe(true);
  });

  it("moves the focused point with the arrow keys", () => {
    const p = c([30, 20], [50, 40], [70, 60]);
    expect(nudgePoint(p, 1, "ArrowRight")[1]).toEqual({ temperatureC: 51, dutyPercent: 40 });
    expect(nudgePoint(p, 1, "ArrowLeft", true)[1]).toEqual({ temperatureC: 45, dutyPercent: 40 });
    expect(nudgePoint(p, 1, "ArrowUp")[1]).toEqual({ temperatureC: 50, dutyPercent: 41 });
    expect(nudgePoint(p, 1, "ArrowDown", true)[1]).toEqual({ temperatureC: 50, dutyPercent: 35 });
    // Blocked by the neighbour: stays valid.
    const tight = c([30, 20], [31, 20]);
    expect(nudgePoint(tight, 1, "ArrowLeft")).toEqual(tight);
    expect(nudgePoint(tight, 1, "ArrowDown")).toEqual(tight);
  });
});

describe("adding and removing points", () => {
  it("adds in the widest gap, on the line", () => {
    const r = addPoint(c([30, 20], [40, 30], [90, 80]));
    expect(r?.index).toBe(2);
    expect(r?.points[2]).toEqual({ temperatureC: 65, dutyPercent: 55 });
    expect(isValidCurve(r!.points)).toBe(true);
  });

  it("stops at the maximum and keeps the minimum", () => {
    const full = c(...Array.from({ length: MAX_POINTS }, (_, i) => [30 + i * 5, i * 10] as [number, number]));
    expect(addPoint(full)).toBeNull();
    expect(removePoint(c([30, 0], [40, 10]), 0)).toBeNull();
    expect(removePoint(c([30, 0], [40, 10], [50, 20]), 1)).toEqual(c([30, 0], [50, 20]));
  });

  it("recognises presets", () => {
    expect(matchingPreset(PRESETS.silent)).toBe("silent");
    expect(matchingPreset(c([30, 0], [40, 10]))).toBeNull();
  });
});

describe("chart geometry", () => {
  const plot = { x: 40, y: 10, w: 400, h: 200 };
  it("maps both ways", () => {
    expect(tempToX(plot, 20)).toBe(40);
    expect(tempToX(plot, 100)).toBe(440);
    expect(dutyToY(plot, 0)).toBe(210);
    expect(dutyToY(plot, 100)).toBe(10);
    expect(xToTemp(plot, tempToX(plot, 63))).toBeCloseTo(63);
    expect(yToDuty(plot, dutyToY(plot, 37))).toBeCloseTo(37);
  });
  it("draws flat to both edges", () => {
    expect(curvePath(plot, c([60, 50], [80, 100]))).toBe("M40.0,110.0 L240.0,110.0 L340.0,10.0 L440.0,10.0");
    expect(curvePath(plot, [])).toBe("");
  });
});
