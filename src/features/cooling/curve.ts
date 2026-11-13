import type { CoolingCurve } from "@/types/generated/CoolingCurve";
import type { CoolingCurvePoint } from "@/types/generated/CoolingCurvePoint";

/**
 * Fan curve logic of the editor, mirroring `CoolingCurve::validate` in
 * crates/models/src/control/cooling.rs: 2–8 points, temperature 20–100 °C strictly
 * increasing, duty 0–100 % never decreasing.
 */

export const TEMP_MIN = 20;
export const TEMP_MAX = 100;
export const DUTY_MIN = 0;
export const DUTY_MAX = 100;
export const MIN_POINTS = 2;
export const MAX_POINTS = 8;
/** Drag snapping and arrow-key steps. */
export const TEMP_STEP = 1;
export const DUTY_STEP = 1;
/** Shift + arrow. */
export const BIG_STEP = 5;

export type CurvePresetId = "silent" | "balanced" | "aggressive";

const pts = (list: [number, number][]): CoolingCurvePoint[] => list.map(([temperatureC, dutyPercent]) => ({ temperatureC, dutyPercent }));

/** Same values as the Rust presets (`CoolingCurve::silent` / `balanced` / `aggressive`). */
export const PRESETS: Record<CurvePresetId, CoolingCurvePoint[]> = {
  silent: pts([
    [30, 0],
    [55, 20],
    [75, 45],
    [90, 80],
  ]),
  balanced: pts([
    [30, 20],
    [50, 35],
    [70, 60],
    [85, 100],
  ]),
  aggressive: pts([
    [30, 35],
    [45, 50],
    [60, 75],
    [75, 100],
  ]),
};

export type CurveIssue =
  | { kind: "count"; count: number }
  | { kind: "range"; index: number }
  | { kind: "order"; index: number }
  | { kind: "decreasing"; index: number };

/** Every rule the curve breaks (empty: valid). */
export function validateCurve(points: readonly CoolingCurvePoint[]): CurveIssue[] {
  const issues: CurveIssue[] = [];
  if (points.length < MIN_POINTS || points.length > MAX_POINTS) issues.push({ kind: "count", count: points.length });
  points.forEach((p, i) => {
    const t = p.temperatureC;
    const d = p.dutyPercent;
    if (!Number.isFinite(t) || !Number.isFinite(d) || t < TEMP_MIN || t > TEMP_MAX || d < DUTY_MIN || d > DUTY_MAX) issues.push({ kind: "range", index: i });
    const prev = points[i - 1];
    if (prev) {
      if (t <= prev.temperatureC) issues.push({ kind: "order", index: i });
      if (d < prev.dutyPercent) issues.push({ kind: "decreasing", index: i });
    }
  });
  return issues;
}

export const isValidCurve = (points: readonly CoolingCurvePoint[]): boolean => validateCurve(points).length === 0;

/** Duty at `temperatureC`: linear between points, flat beyond the ends; null without points. */
export function dutyAt(points: readonly CoolingCurvePoint[], temperatureC: number): number | null {
  const first = points[0];
  const last = points[points.length - 1];
  if (!first || !last) return null;
  if (temperatureC <= first.temperatureC) return first.dutyPercent;
  if (temperatureC >= last.temperatureC) return last.dutyPercent;
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1]!;
    const b = points[i]!;
    if (temperatureC <= b.temperatureC) {
      const f = (temperatureC - a.temperatureC) / (b.temperatureC - a.temperatureC);
      return a.dutyPercent + f * (b.dutyPercent - a.dutyPercent);
    }
  }
  return last.dutyPercent;
}

export const snap = (value: number, step: number): number => Math.round(value / step) * step;
const clamp = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v));

/**
 * The range point `index` may take while staying valid: strictly between its neighbours'
 * temperatures (one step apart) and between their duties.
 */
export function pointBounds(points: readonly CoolingCurvePoint[], index: number) {
  const prev = points[index - 1];
  const next = points[index + 1];
  return {
    tMin: prev ? prev.temperatureC + TEMP_STEP : TEMP_MIN,
    tMax: next ? next.temperatureC - TEMP_STEP : TEMP_MAX,
    dMin: prev ? prev.dutyPercent : DUTY_MIN,
    dMax: next ? next.dutyPercent : DUTY_MAX,
  };
}

/** Moves point `index` to (t, d), snapped and kept inside its bounds, so the curve stays valid. */
export function movePoint(points: readonly CoolingCurvePoint[], index: number, temperatureC: number, dutyPercent: number): CoolingCurvePoint[] {
  if (!points[index]) return [...points];
  const b = pointBounds(points, index);
  const t = clamp(snap(temperatureC, TEMP_STEP), b.tMin, Math.max(b.tMin, b.tMax));
  const d = clamp(snap(dutyPercent, DUTY_STEP), b.dMin, Math.max(b.dMin, b.dMax));
  return points.map((p, i) => (i === index ? { temperatureC: t, dutyPercent: d } : p));
}

export type ArrowKey = "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown";

/** Keyboard move of the focused point: left/right change the temperature, up/down the duty. */
export function nudgePoint(points: readonly CoolingCurvePoint[], index: number, key: ArrowKey, big = false): CoolingCurvePoint[] {
  const p = points[index];
  if (!p) return [...points];
  const s = big ? BIG_STEP : 1;
  const dt = key === "ArrowRight" ? s * TEMP_STEP : key === "ArrowLeft" ? -s * TEMP_STEP : 0;
  const dd = key === "ArrowUp" ? s * DUTY_STEP : key === "ArrowDown" ? -s * DUTY_STEP : 0;
  return movePoint(points, index, p.temperatureC + dt, p.dutyPercent + dd);
}

/**
 * Adds a point in the widest temperature gap, on the line (so the shape does not change).
 * Returns the new points and the new point's index, or null when the curve is full or no
 * gap is wide enough.
 */
export function addPoint(points: readonly CoolingCurvePoint[]): { points: CoolingCurvePoint[]; index: number } | null {
  if (points.length >= MAX_POINTS) return null;
  let best = -1;
  let width = 0;
  const edges = [TEMP_MIN, ...points.map((p) => p.temperatureC), TEMP_MAX];
  for (let i = 0; i < edges.length - 1; i++) {
    const w = edges[i + 1]! - edges[i]!;
    if (w > width) {
      width = w;
      best = i;
    }
  }
  if (best < 0 || width < 2 * TEMP_STEP) return null;
  const t = snap((edges[best]! + edges[best + 1]!) / 2, TEMP_STEP);
  const d = snap(dutyAt(points, t) ?? 50, DUTY_STEP);
  const next = [...points.slice(0, best), { temperatureC: t, dutyPercent: d }, ...points.slice(best)];
  return { points: next, index: best };
}

/** Removes point `index` unless the curve would fall under the minimum. */
export function removePoint(points: readonly CoolingCurvePoint[], index: number): CoolingCurvePoint[] | null {
  if (points.length <= MIN_POINTS || !points[index]) return null;
  return points.filter((_, i) => i !== index);
}

/** Which preset the points equal, if any. */
export function matchingPreset(points: readonly CoolingCurvePoint[]): CurvePresetId | null {
  for (const id of Object.keys(PRESETS) as CurvePresetId[]) {
    const p = PRESETS[id];
    if (p.length === points.length && p.every((q, i) => q.temperatureC === points[i]?.temperatureC && q.dutyPercent === points[i]?.dutyPercent)) return id;
  }
  return null;
}

export const toCurve = (points: readonly CoolingCurvePoint[]): CoolingCurve => ({ points: points.map((p) => ({ ...p })) });

/** Chart geometry: plot area inside the SVG. */
export interface Plot {
  x: number;
  y: number;
  w: number;
  h: number;
}

export const tempToX = (plot: Plot, t: number): number => plot.x + ((t - TEMP_MIN) / (TEMP_MAX - TEMP_MIN)) * plot.w;
export const dutyToY = (plot: Plot, d: number): number => plot.y + plot.h - ((d - DUTY_MIN) / (DUTY_MAX - DUTY_MIN)) * plot.h;
export const xToTemp = (plot: Plot, x: number): number => TEMP_MIN + ((x - plot.x) / plot.w) * (TEMP_MAX - TEMP_MIN);
export const yToDuty = (plot: Plot, y: number): number => DUTY_MIN + ((plot.y + plot.h - y) / plot.h) * (DUTY_MAX - DUTY_MIN);

/** SVG path of the curve, flat to both chart edges. */
export function curvePath(plot: Plot, points: readonly CoolingCurvePoint[]): string {
  const first = points[0];
  const last = points[points.length - 1];
  if (!first || !last) return "";
  const xy = [
    [tempToX(plot, TEMP_MIN), dutyToY(plot, first.dutyPercent)],
    ...points.map((p) => [tempToX(plot, p.temperatureC), dutyToY(plot, p.dutyPercent)]),
    [tempToX(plot, TEMP_MAX), dutyToY(plot, last.dutyPercent)],
  ];
  return xy.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x!.toFixed(1)},${y!.toFixed(1)}`).join(" ");
}
