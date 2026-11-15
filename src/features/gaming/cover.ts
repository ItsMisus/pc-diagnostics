/**
 * Typographic placeholder for a game without artwork: a two-tone gradient whose hues come
 * from a hash of the title (stable across runs) and the title set in the display face. It is
 * meant to read as a generated label, not as fake box art.
 *
 * The colors are computed, not theme tokens: they identify a title, like an avatar color.
 * Lightness and chroma are fixed so white text on them stays legible in both themes.
 */

/** FNV-1a 32-bit hash. */
export function hashTitle(title: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < title.length; i++) {
    h ^= title.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h >>> 0;
}

export interface CoverPalette {
  /** CSS colors. */
  from: string;
  to: string;
  /** Gradient angle, degrees. */
  angle: number;
  hue: number;
}

export function coverPalette(title: string): CoverPalette {
  const h = hashTitle(title.trim().toLowerCase());
  const hue = h % 360;
  // Second hue 30–80° away, direction from another part of the hash.
  const shift = 30 + ((h >>> 9) % 51);
  const hue2 = (hue + ((h >>> 17) & 1 ? shift : 360 - shift)) % 360;
  const angle = 120 + ((h >>> 20) % 100);
  return {
    hue,
    from: `oklch(0.52 0.13 ${hue})`,
    to: `oklch(0.34 0.11 ${hue2})`,
    angle,
  };
}

export function coverBackground(title: string): string {
  const p = coverPalette(title);
  return `linear-gradient(${p.angle}deg, ${p.from}, ${p.to})`;
}

/** Up to two initials (letters or digits of the first two words), for very small covers. */
export function coverMonogram(title: string): string {
  const words = title
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .split(/\s+/)
    .filter(Boolean);
  const first = words[0];
  if (!first) return "?";
  const second = words[1];
  const a = [...first][0] ?? "";
  const b = second ? ([...second][0] ?? "") : "";
  return (a + b).toUpperCase();
}
