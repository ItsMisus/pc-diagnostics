import type { LightingColor } from "@/types/generated/LightingColor";
import type { LightingEffect } from "@/types/generated/LightingEffect";

/**
 * Effect math, a line-by-line port of `crates/control/src/lighting/effect.rs`, so the preview
 * shows what the devices show. Both test suites check `effect-vectors.json`.
 *
 * `position` is the lamp's place along the device, 0 (left) to 1 (right); without one, lamps
 * are spread evenly by index. One cycle of a dynamic effect lasts `12 000 / speed` ms.
 */

export const BLACK: LightingColor = { r: 0, g: 0, b: 0 };

export function cycleMs(speed: number): number {
  return 12_000 / Math.min(10, Math.max(1, Math.round(speed)));
}

export function defaultPosition(index: number, count: number): number {
  if (count <= 1) return 0.5;
  return Math.min(index, count - 1) / (count - 1);
}

const fract = (x: number) => x - Math.floor(x);

interface Rgbf {
  r: number;
  g: number;
  b: number;
}

/** Rust's `f64::round` (half away from zero) for the non-negative values used here. */
const roundHalfAway = (v: number) => (v < 0 ? -Math.round(-v) : Math.round(v));

function toColor(c: Rgbf): LightingColor {
  const ch = (v: number) => Math.min(255, Math.max(0, roundHalfAway(v)));
  return { r: ch(c.r), g: ch(c.g), b: ch(c.b) };
}

const scale = (c: Rgbf, k: number): Rgbf => ({ r: c.r * k, g: c.g * k, b: c.b * k });
const lerp = (a: Rgbf, b: Rgbf, t: number): Rgbf => ({ r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t });

/** HSV (all 0–1) to sRGB 0–255, unrounded. */
export function hsvRaw(h: number, s: number, v: number): Rgbf {
  const h6 = fract(h) * 6;
  const sector = Math.floor(h6);
  const f = h6 - sector;
  const p = v * (1 - s);
  const q = v * (1 - s * f);
  const t = v * (1 - s * (1 - f));
  let rgb: [number, number, number];
  switch (sector) {
    case 0:
      rgb = [v, t, p];
      break;
    case 1:
      rgb = [q, v, p];
      break;
    case 2:
      rgb = [p, v, t];
      break;
    case 3:
      rgb = [p, q, v];
      break;
    case 4:
      rgb = [t, p, v];
      break;
    default:
      rgb = [v, p, q];
  }
  return { r: rgb[0] * 255, g: rgb[1] * 255, b: rgb[2] * 255 };
}

function palette(colors: readonly LightingColor[], u: number, cyclic: boolean): Rgbf {
  if (colors.length === 0) return BLACK;
  if (colors.length === 1) return colors[0]!;
  const n = colors.length;
  const segments = cyclic ? n : n - 1;
  const x = cyclic ? fract(u) * segments : Math.min(1, Math.max(0, u)) * segments;
  const i = Math.min(Math.floor(x), segments - 1);
  const t = x - i;
  return lerp(colors[i]!, colors[(i + 1) % n]!, t);
}

/** The color of lamp `index` (of `count`) at `tMs` after the effect started. */
export function colorAt(effect: LightingEffect, tMs: number, index: number, count: number, position?: number | null): LightingColor {
  const x = position ?? defaultPosition(index, count);
  const t = tMs;
  switch (effect.kind) {
    case "off":
      return BLACK;
    case "static":
      return effect.color;
    case "breathing": {
      const period = Math.max(1, effect.period);
      const phase = fract(t / period);
      const k = (1 - Math.cos(2 * Math.PI * phase)) / 2;
      return toColor(scale(effect.color, k));
    }
    case "rainbow":
      return toColor(hsvRaw(t / cycleMs(effect.speed) + x, 1, 1));
    case "wave": {
      const shift = t / cycleMs(effect.speed);
      const u = effect.direction === "leftToRight" ? x - shift : x + shift;
      return toColor(palette(effect.colors, u, true));
    }
    case "gradient":
      return toColor(palette(effect.colors, x, false));
    case "colorCycle":
      return toColor(palette(effect.colors, t / cycleMs(effect.speed), true));
  }
}

/** Level 0–1 of a single-color light (color ignored: off 0, steady 1, breathing the breath). */
export function levelAt(effect: LightingEffect, tMs: number): number {
  if (effect.kind === "off") return 0;
  if (effect.kind !== "breathing") return 1;
  return colorAt({ kind: "breathing", color: { r: 255, g: 255, b: 255 }, period: effect.period }, tMs, 0, 1).r / 255;
}

/** A color dimmed by a brightness of 0–100. */
export function dim(c: LightingColor, brightness: number): LightingColor {
  if (brightness >= 100) return c;
  return toColor(scale(c, Math.max(0, brightness) / 100));
}

/** The effect changes over time. */
export function isDynamic(effect: LightingEffect): boolean {
  return effect.kind === "breathing" || effect.kind === "rainbow" || effect.kind === "wave" || effect.kind === "colorCycle";
}

/** A single-color (white) light can show it. */
export function monochromeCompatible(effect: LightingEffect): boolean {
  return effect.kind === "off" || effect.kind === "static" || effect.kind === "breathing";
}

/** The colors an effect uses, for swatches and the stage glow (rainbow: a fixed spectrum). */
export function effectColors(effect: LightingEffect): LightingColor[] {
  switch (effect.kind) {
    case "off":
      return [];
    case "static":
    case "breathing":
      return [effect.color];
    case "rainbow":
      return [0, 1 / 6, 2 / 6, 3 / 6, 4 / 6, 5 / 6].map((h) => toColor(hsvRaw(h, 1, 1)));
    case "wave":
    case "gradient":
    case "colorCycle":
      return effect.colors;
  }
}
