import { parseColor, type Rgba } from "./color";

/**
 * Theme helpers for code that cannot use CSS tokens directly (a WebGL scene, a canvas).
 * The source of truth stays `src/styles/index.css`.
 */

export type ThemePreferenceLike = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

/** The theme in effect: an explicit preference wins, "system" follows the OS. */
export function resolveTheme(preference: ThemePreferenceLike, systemPrefersDark: boolean): ResolvedTheme {
  if (preference === "light" || preference === "dark") return preference;
  return systemPrefersDark ? "dark" : "light";
}

/** The theme currently applied to the document (`data-theme` set by useThemeSync, else the OS). */
export function documentTheme(): ResolvedTheme {
  if (typeof document === "undefined") return "light";
  const explicit = document.documentElement.dataset.theme;
  if (explicit === "light" || explicit === "dark") return explicit;
  const dark = typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia("(prefers-color-scheme: dark)").matches;
  return dark ? "dark" : "light";
}

/**
 * Reads a color token (e.g. `"--accent"`) as resolved for the current theme. Returns null when
 * the token is missing or not a plain color (gradients, shadows).
 */
export function readColorToken(name: `--${string}`, root?: Element): Rgba | null {
  if (typeof document === "undefined" || typeof getComputedStyle !== "function") return null;
  const value = getComputedStyle(root ?? document.documentElement).getPropertyValue(name);
  return value ? parseColor(value) : null;
}
