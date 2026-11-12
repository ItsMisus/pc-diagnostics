/**
 * Small color helpers for the design tokens: parse the formats `src/styles/index.css` uses
 * (`#rgb`, `#rrggbb`, `rgb(r g b / a)`, `rgb(r, g, b)`), composite a translucent color over an
 * opaque one and compute WCAG contrast. Used by the token tests (text pairs must stay AA) and
 * by code that needs a token as numbers (e.g. a WebGL scene matching the theme).
 */

export interface Rgba {
  /** 0–255. */
  r: number;
  g: number;
  b: number;
  /** 0–1. */
  a: number;
}

/** Parses a hex or `rgb()` / `rgba()` color. Returns null for anything else (gradients, `var()`). */
export function parseColor(input: string): Rgba | null {
  const s = input.trim().toLowerCase();
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/.exec(s);
  if (hex) {
    let h = hex[1]!;
    if (h.length === 3) h = [...h].map((c) => c + c).join("");
    const n = (i: number) => parseInt(h.slice(i, i + 2), 16);
    return { r: n(0), g: n(2), b: n(4), a: h.length === 8 ? n(6) / 255 : 1 };
  }
  const fn = /^rgba?\(([^)]+)\)$/.exec(s);
  if (!fn) return null;
  const [channels, alpha] = fn[1]!.split("/").map((p) => p.trim());
  const parts = channels!.split(/[\s,]+/).filter(Boolean);
  // Legacy comma syntax may carry the alpha as a fourth value.
  const alphaText = alpha ?? (parts.length === 4 ? parts[3] : undefined);
  if (parts.length < 3) return null;
  const nums = parts.slice(0, 3).map(Number);
  if (nums.some((v) => !Number.isFinite(v))) return null;
  let a = 1;
  if (alphaText !== undefined) {
    a = alphaText.endsWith("%") ? Number(alphaText.slice(0, -1)) / 100 : Number(alphaText);
    if (!Number.isFinite(a)) return null;
  }
  return { r: nums[0]!, g: nums[1]!, b: nums[2]!, a: Math.min(1, Math.max(0, a)) };
}

/** `top` drawn over the opaque `bottom` (source-over), result opaque. */
export function composite(top: Rgba, bottom: Rgba): Rgba {
  const mix = (t: number, b: number) => t * top.a + b * (1 - top.a);
  return { r: mix(top.r, bottom.r), g: mix(top.g, bottom.g), b: mix(top.b, bottom.b), a: 1 };
}

/** WCAG 2.x relative luminance of an opaque color. */
export function luminance({ r, g, b }: Rgba): number {
  const lin = (c: number) => {
    const v = c / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/**
 * WCAG contrast ratio (1–21) of `fg` on `bg`. A translucent foreground is first composited
 * over the background; the background must be opaque.
 */
export function contrastRatio(fg: Rgba, bg: Rgba): number {
  const f = fg.a < 1 ? composite(fg, bg) : fg;
  const l1 = luminance(f);
  const l2 = luminance(bg);
  const [hi, lo] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (hi + 0.05) / (lo + 0.05);
}

/** `#rrggbb` of an opaque color (channels rounded). For WebGL / canvas consumers. */
export function toHex({ r, g, b }: Rgba): string {
  const h = (v: number) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0");
  return `#${h(r)}${h(g)}${h(b)}`;
}
