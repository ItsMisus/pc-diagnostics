import type { LightingColor } from "@/types/generated/LightingColor";
import { hsvRaw } from "./effect";

/** Hue, saturation and value, each 0–1. */
export interface Hsv {
  h: number;
  s: number;
  v: number;
}

const clamp01 = (x: number) => Math.min(1, Math.max(0, x));
const byte = (x: number) => Math.min(255, Math.max(0, Math.round(x)));

export function rgbToHsv({ r, g, b }: LightingColor): Hsv {
  const rf = r / 255;
  const gf = g / 255;
  const bf = b / 255;
  const max = Math.max(rf, gf, bf);
  const min = Math.min(rf, gf, bf);
  const d = max - min;
  let h = 0;
  if (d > 0) {
    if (max === rf) h = ((gf - bf) / d + 6) % 6;
    else if (max === gf) h = (bf - rf) / d + 2;
    else h = (rf - gf) / d + 4;
    h /= 6;
  }
  return { h, s: max === 0 ? 0 : d / max, v: max };
}

export function hsvToRgb({ h, s, v }: Hsv): LightingColor {
  const c = hsvRaw(h, clamp01(s), clamp01(v));
  return { r: byte(c.r), g: byte(c.g), b: byte(c.b) };
}

/** `#rrggbb` (lowercase). */
export function toHex({ r, g, b }: LightingColor): string {
  const h = (v: number) => byte(v).toString(16).padStart(2, "0");
  return `#${h(r)}${h(g)}${h(b)}`;
}

/** Parses `#rgb`, `#rrggbb`, with or without `#`, any case; null otherwise. */
export function parseHex(input: string): LightingColor | null {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(input.trim());
  if (!m) return null;
  let hex = m[1]!;
  if (hex.length === 3) hex = [...hex].map((c) => c + c).join("");
  const n = (i: number) => parseInt(hex.slice(i, i + 2), 16);
  return { r: n(0), g: n(2), b: n(4) };
}

/** A channel typed by the user: integer 0–255, or null when not a number. */
export function parseChannel(input: string): number | null {
  if (!/^\s*\d{1,3}\s*$/.test(input)) return null;
  const v = Number(input);
  return v >= 0 && v <= 255 ? v : null;
}

export function sameColor(a: LightingColor, b: LightingColor): boolean {
  return a.r === b.r && a.g === b.g && a.b === b.b;
}

export const rgbCss = ({ r, g, b }: LightingColor, alpha = 1): string => (alpha >= 1 ? `rgb(${r} ${g} ${b})` : `rgb(${r} ${g} ${b} / ${alpha})`);

/** Most recent colors kept by the picker. */
export const RECENT_MAX = 8;

/** Puts `color` first, without duplicates, at most `RECENT_MAX`. */
export function pushRecent(recent: readonly LightingColor[], color: LightingColor): LightingColor[] {
  return [color, ...recent.filter((c) => !sameColor(c, color))].slice(0, RECENT_MAX);
}

/** Preset swatches of the picker. */
export const SWATCHES: readonly LightingColor[] = [
  { r: 255, g: 255, b: 255 },
  { r: 255, g: 59, b: 48 },
  { r: 255, g: 149, b: 0 },
  { r: 255, g: 204, b: 0 },
  { r: 52, g: 199, b: 89 },
  { r: 0, g: 199, b: 190 },
  { r: 78, g: 165, b: 255 },
  { r: 88, g: 86, b: 214 },
  { r: 175, g: 82, b: 222 },
  { r: 255, g: 45, b: 85 },
];

/**
 * Keyboard step on the saturation/value square: arrows move by 1 %, with Shift by 10 %;
 * Home/End jump to the edges of saturation, PageUp/PageDown of value.
 */
export function stepSv(hsv: Hsv, key: string, shift: boolean): Hsv | null {
  const d = shift ? 0.1 : 0.01;
  switch (key) {
    case "ArrowLeft":
      return { ...hsv, s: clamp01(hsv.s - d) };
    case "ArrowRight":
      return { ...hsv, s: clamp01(hsv.s + d) };
    case "ArrowUp":
      return { ...hsv, v: clamp01(hsv.v + d) };
    case "ArrowDown":
      return { ...hsv, v: clamp01(hsv.v - d) };
    case "Home":
      return { ...hsv, s: 0 };
    case "End":
      return { ...hsv, s: 1 };
    case "PageUp":
      return { ...hsv, v: 1 };
    case "PageDown":
      return { ...hsv, v: 0 };
    default:
      return null;
  }
}

/** Keyboard step on the hue slider (degrees: 1, Shift 15), wrapping around. */
export function stepHue(h: number, key: string, shift: boolean): number | null {
  const d = (shift ? 15 : 1) / 360;
  if (key === "ArrowRight" || key === "ArrowUp") return (h + d) % 1;
  if (key === "ArrowLeft" || key === "ArrowDown") return (h - d + 1) % 1;
  if (key === "Home") return 0;
  if (key === "End") return 359 / 360;
  return null;
}

/** Point on the square (0–1 each, y down) to saturation/value. */
export function svFromPoint(x: number, y: number): { s: number; v: number } {
  return { s: clamp01(x), v: clamp01(1 - y) };
}

/** Average of colors (for the stage glow); black without colors. */
export function averageColor(colors: readonly LightingColor[]): LightingColor {
  if (colors.length === 0) return { r: 0, g: 0, b: 0 };
  const sum = colors.reduce((a, c) => ({ r: a.r + c.r, g: a.g + c.g, b: a.b + c.b }), { r: 0, g: 0, b: 0 });
  return { r: byte(sum.r / colors.length), g: byte(sum.g / colors.length), b: byte(sum.b / colors.length) };
}

/** Linear interpolation between two colors (smooth picker transitions). */
export function mixColor(a: LightingColor, b: LightingColor, t: number): LightingColor {
  const k = clamp01(t);
  return { r: byte(a.r + (b.r - a.r) * k), g: byte(a.g + (b.g - a.g) * k), b: byte(a.b + (b.b - a.b) * k) };
}
