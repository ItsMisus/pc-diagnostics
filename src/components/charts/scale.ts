/** Time-series point. `v: null` marks a gap (sample unavailable) and breaks the line. */
export interface Point {
  t: number;
  v: number | null;
}

export type Domain = readonly [number, number];

/** Maps `domain` linearly onto `range`. A degenerate domain maps to the range midpoint. */
export function linear(domain: Domain, range: Domain): (x: number) => number {
  const [d0, d1] = domain;
  const [r0, r1] = range;
  if (d0 === d1) return () => (r0 + r1) / 2;
  const k = (r1 - r0) / (d1 - d0);
  return (x) => r0 + (x - d0) * k;
}

export function clamp(x: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, x));
}

/**
 * Builds an SVG path. The y domain is fixed by the caller (e.g. 0–100 for percent) so
 * charts never auto-zoom small variations into dramatic swings. Out-of-domain values are
 * clamped to the plot edge; gaps start a new subpath instead of interpolating.
 */
export function linePath(
  points: readonly Point[],
  xDomain: Domain,
  yDomain: Domain,
  width: number,
  height: number,
): string {
  const x = linear(xDomain, [0, width]);
  const y = linear(yDomain, [height, 0]);
  const parts: string[] = [];
  let penDown = false;
  for (const p of points) {
    if (p.v === null || !Number.isFinite(p.v)) {
      penDown = false;
      continue;
    }
    const px = clamp(x(p.t), 0, width).toFixed(1);
    const py = clamp(y(p.v), 0, height).toFixed(1);
    parts.push(`${penDown ? "L" : "M"}${px} ${py}`);
    penDown = true;
  }
  return parts.join(" ");
}

/**
 * Closed area under each subpath of a `linePath` output, down to the baseline `height`.
 * Gaps stay gaps: every subpath gets its own closed shape.
 */
export function areaPath(line: string, height: number): string {
  if (!line) return "";
  const base = height.toFixed(1);
  return line
    .split("M")
    .filter((seg) => seg.trim() !== "")
    .map((seg) => {
      const coords = seg.trim().split(" L");
      const first = coords[0]?.split(" ")[0];
      const last = coords[coords.length - 1]?.split(" ")[0];
      return `M${seg.trim()} L${last} ${base} L${first} ${base} Z`;
    })
    .join(" ");
}

/** Index of the point closest in time to `t` (points sorted by `t`). -1 if empty. */
export function nearestIndex(points: readonly Point[], t: number): number {
  if (points.length === 0) return -1;
  let lo = 0;
  let hi = points.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if ((points[mid]?.t ?? 0) < t) lo = mid + 1;
    else hi = mid;
  }
  const prev = points[lo - 1];
  const cur = points[lo];
  if (prev && cur && Math.abs(prev.t - t) <= Math.abs(cur.t - t)) return lo - 1;
  return lo;
}

/** `count` evenly spaced ticks across the domain, inclusive. */
export function ticks(domain: Domain, count: number): number[] {
  const [a, b] = domain;
  if (count < 2) return [a];
  const step = (b - a) / (count - 1);
  return Array.from({ length: count }, (_, i) => a + step * i);
}

/** Coordinates of the last point of a `linePath` output, or null when empty. */
export function lastPoint(path: string): [number, number] | null {
  const m = /(-?[\d.]+) (-?[\d.]+)\s*$/.exec(path);
  return m ? [Number(m[1]), Number(m[2])] : null;
}
