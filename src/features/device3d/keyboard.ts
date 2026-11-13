/**
 * Key layout of the laptop model: a generic 15-unit notebook keyboard (function row, number
 * row, three letter rows, bottom row with inverted-T arrows). Pure, so it is unit-tested and
 * the scene only turns it into instanced meshes.
 */

export interface KeyRect {
  /** Centre, model units (x right, z towards the front). */
  x: number;
  z: number;
  /** Cap size. */
  w: number;
  d: number;
}

/** Widths in key units, back row first. Every full row adds up to 15 units. */
const ROWS: readonly { depth: number; keys: readonly number[] }[] = [
  { depth: 0.55, keys: [1.5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1.5] },
  { depth: 1, keys: [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2] },
  { depth: 1, keys: [1.5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1.5] },
  { depth: 1, keys: [1.75, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2.25] },
  { depth: 1, keys: [2.25, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2.75] },
];
/** Bottom row before the arrow cluster (fn, ctrl, option, command, space, command, option). */
const BOTTOM = [1, 1, 1, 1.25, 5.5, 1.25, 1] as const;
export const KEYBOARD_UNITS = 15;

export function keyboardLayout(unit: number, gap: number): KeyRect[] {
  const keys: KeyRect[] = [];
  const width = KEYBOARD_UNITS * unit;
  const depth = (ROWS.reduce((s, r) => s + r.depth, 0) + 1) * unit;
  let z = -depth / 2;
  const row = (widths: readonly number[], rowDepth: number) => {
    let x = -width / 2;
    for (const w of widths) {
      keys.push({ x: x + (w * unit) / 2, z: z + (rowDepth * unit) / 2, w: w * unit - gap, d: rowDepth * unit - gap });
      x += w * unit;
    }
    return x;
  };
  for (const r of ROWS) {
    row(r.keys, r.depth);
    z += r.depth * unit;
  }
  // Bottom row, then the arrows: ← and → full height, ↑ ↓ stacked half keys.
  let x = row(BOTTOM, 1);
  const half = unit / 2;
  keys.push({ x: x + half, z: z + half + half / 2, w: unit - gap, d: half - gap / 2 });
  x += unit;
  keys.push({ x: x + half, z: z + half / 2, w: unit - gap, d: half - gap / 2 });
  keys.push({ x: x + half, z: z + half + half / 2, w: unit - gap, d: half - gap / 2 });
  x += unit;
  keys.push({ x: x + half, z: z + half + half / 2, w: unit - gap, d: half - gap / 2 });
  return keys;
}

/** Keys grouped by cap size (one instanced mesh per size keeps the corner radii exact). */
export function groupBySize(keys: readonly KeyRect[]): Map<string, KeyRect[]> {
  const groups = new Map<string, KeyRect[]>();
  for (const k of keys) {
    const id = `${k.w.toFixed(4)}x${k.d.toFixed(4)}`;
    const list = groups.get(id);
    if (list) list.push(k);
    else groups.set(id, [k]);
  }
  return groups;
}
