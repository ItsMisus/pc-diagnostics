/**
 * The app's one icon family (inline SVG, no icon font, no network): 16-unit grid, 1.5 stroke,
 * round caps and joins, ~1.75-unit optical margin, rounded rectangles with 1.25–2 unit
 * corners. Rendered by `<Icon>`; pure data so tests can check that every name used in the
 * app resolves (`icons.test.ts`).
 *
 * Adding an icon: draw it on the same grid with the helpers below, stroke only (no fills),
 * and give it a name that says what it depicts, not where it is used. Aliases (e.g.
 * `warning` → `alert`) are allowed when two names read naturally in code.
 */

const n = (v: number) => Number(v.toFixed(3)).toString();

/** Rounded rectangle as a closed subpath. */
function rect(x: number, y: number, w: number, h: number, r: number): string {
  const rr = Math.min(r, w / 2, h / 2);
  return (
    `M${n(x + rr)} ${n(y)}h${n(w - 2 * rr)}a${n(rr)} ${n(rr)} 0 0 1 ${n(rr)} ${n(rr)}v${n(h - 2 * rr)}` +
    `a${n(rr)} ${n(rr)} 0 0 1 ${n(-rr)} ${n(rr)}h${n(-(w - 2 * rr))}a${n(rr)} ${n(rr)} 0 0 1 ${n(-rr)} ${n(-rr)}` +
    `v${n(-(h - 2 * rr))}a${n(rr)} ${n(rr)} 0 0 1 ${n(rr)} ${n(-rr)}z`
  );
}

/** Full circle as a subpath. */
function circle(cx: number, cy: number, r: number): string {
  return `M${n(cx - r)} ${n(cy)}a${n(r)} ${n(r)} 0 1 0 ${n(2 * r)} 0a${n(r)} ${n(r)} 0 1 0 ${n(-2 * r)} 0`;
}

/** Gear outline: `teeth` flat-topped teeth between radii `inner` and `outer`. */
function gear(cx: number, cy: number, teeth: number, inner: number, outer: number): string {
  const step = (2 * Math.PI) / teeth;
  const top = step * 0.22;
  const base = step * 0.32;
  const pt = (a: number, r: number) => `${n(cx + r * Math.cos(a))} ${n(cy + r * Math.sin(a))}`;
  const parts: string[] = [];
  for (let i = 0; i < teeth; i++) {
    const a = i * step - Math.PI / 2;
    parts.push(`${i === 0 ? "M" : "L"}${pt(a - base, inner)}`, `L${pt(a - top, outer)}`, `L${pt(a + top, outer)}`, `L${pt(a + base, inner)}`);
  }
  return `${parts.join("")}z`;
}

const sunRays = [0, 45, 90, 135, 180, 225, 270, 315]
  .map((deg) => {
    const a = (deg * Math.PI) / 180;
    return `M${n(8 + 4.6 * Math.cos(a))} ${n(8 + 4.6 * Math.sin(a))}L${n(8 + 6.1 * Math.cos(a))} ${n(8 + 6.1 * Math.sin(a))}`;
  })
  .join("");

const alertTriangle = "M6.7 3.05a1.5 1.5 0 0 1 2.6 0l5.05 8.85a1.5 1.5 0 0 1-1.3 2.25H2.95a1.5 1.5 0 0 1-1.3-2.25z";

const chip = `${rect(4, 4, 8, 8, 1.5)}${rect(6.25, 6.25, 3.5, 3.5, 0.5)}M6.25 1.75V4M9.75 1.75V4M6.25 12v2.25M9.75 12v2.25M1.75 6.25H4M1.75 9.75H4M12 6.25h2.25M12 9.75h2.25`;
const gauge = "M2.25 11.75a5.75 5.75 0 1 1 11.5 0M8 11.75l2.6-3.6M4.4 13.75h7.2";
const sidebar = `${rect(1.75, 2.75, 12.5, 10.5, 2)}M6 2.75v10.5`;

export const ICON_PATHS = {
  // Navigation
  overview: `${rect(2, 2, 5.25, 5.25, 1.25)}${rect(8.75, 2, 5.25, 5.25, 1.25)}${rect(2, 8.75, 5.25, 5.25, 1.25)}${rect(8.75, 8.75, 5.25, 5.25, 1.25)}`,
  hardware: chip,
  benchmarks: gauge,
  processes: "M5.75 4h7.5M5.75 8h7.5M5.75 12h7.5M2.75 4h.01M2.75 8h.01M2.75 12h.01",
  drivers: "M5.75 2v3M10.25 2v3M4 5h8v2.5a4 4 0 0 1-8 0zM8 11.5V14",
  diagnostics: "M1.75 8.25h2.5l1.75-4.5 3.5 8.5 1.75-4h3",
  history: "M8 4.75V8l2.25 1.5M2.6 8a5.4 5.4 0 1 0 1.58-3.82M2.5 2.5v2.75h2.75",
  settings: `${gear(8, 8, 8, 4.75, 6.4)}${circle(8, 8, 2)}`,
  // Shell
  collapse: `${sidebar}M10.75 6.5L9.25 8l1.5 1.5`,
  expand: `${sidebar}M9.25 6.5L10.75 8l-1.5 1.5`,
  sidebar,
  /** View modes: "Semplice" (two lines, less detail) and "Avanzata" (sliders, all the detail). */
  viewSimple: "M3.25 5.75h9.5M3.25 10.25h6",
  viewAdvanced: "M2.75 4.5h10.5M2.75 8h10.5M2.75 11.5h10.5M5.5 3v3M10.5 6.5v3M7 10v3",
  // Status
  info: `${circle(8, 8, 6.25)}M8 7.25v4M8 4.85v.01`,
  alert: `${alertTriangle}M8 6.5v3M8 11.6v.01`,
  warning: `${alertTriangle}M8 6.5v3M8 11.6v.01`,
  error: `${circle(8, 8, 6.25)}M5.75 5.75l4.5 4.5M10.25 5.75l-4.5 4.5`,
  success: `${circle(8, 8, 6.25)}M5.25 8.25l1.9 1.9 3.6-3.9`,
  lock: `${rect(3.25, 7, 9.5, 7, 1.5)}M5.25 7V5a2.75 2.75 0 0 1 5.5 0v2`,
  shield: "M8 1.75l5.25 2v4c0 3.1-2.2 5.4-5.25 6.5-3.05-1.1-5.25-3.4-5.25-6.5v-4z",
  // Actions
  refresh: "M13.25 8a5.25 5.25 0 1 1-1.54-3.71M13.25 2.5v2.75H10.5",
  check: "M3.25 8.25l3 3 6.5-6.5",
  close: "M4.25 4.25l7.5 7.5M11.75 4.25l-7.5 7.5",
  plus: "M8 3.25v9.5M3.25 8h9.5",
  minus: "M3.25 8h9.5",
  more: "M3.5 8h.01M8 8h.01M12.5 8h.01",
  search: "M7.25 12.25a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM13.75 13.75l-3-3",
  filter: "M2.25 3.75h11.5M4.25 8h7.5M6.5 12.25h3",
  external: "M9.5 2.75h3.75V6.5M13.25 2.75L7.5 8.5M11.25 9.5v2.25a1.5 1.5 0 0 1-1.5 1.5h-5.5a1.5 1.5 0 0 1-1.5-1.5v-5.5a1.5 1.5 0 0 1 1.5-1.5H6.5",
  download: "M8 2.5v8M4.75 7.25L8 10.5l3.25-3.25M2.75 13.25h10.5",
  share: "M8 1.75v8M5 4.5l3-3 3 3M5 7.25H4a1.5 1.5 0 0 0-1.5 1.5v4A1.5 1.5 0 0 0 4 14.25h8a1.5 1.5 0 0 0 1.5-1.5v-4A1.5 1.5 0 0 0 12 7.25h-1",
  copy: `${rect(5.25, 5.25, 8.5, 8.5, 1.5)}M10.75 5.25v-1.5a1.5 1.5 0 0 0-1.5-1.5h-5.5a1.5 1.5 0 0 0-1.5 1.5v5.5a1.5 1.5 0 0 0 1.5 1.5h1.5`,
  stop: rect(4, 4, 8, 8, 1.5),
  play: "M5.25 3.6a.75.75 0 0 1 1.13-.65l6.4 4.4a.75.75 0 0 1 0 1.3l-6.4 4.4a.75.75 0 0 1-1.13-.65z",
  key: "M5.5 13a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM7.6 7.9l5.15-5.15M10.75 4.75l1.75 1.75",
  sparkle: "M8 1.75l1.4 3.85 3.85 1.4-3.85 1.4L8 12.25l-1.4-3.85L2.75 7l3.85-1.4zM12.75 11.25v2.5M11.5 12.5H14",
  // Arrows and chevrons
  chevronDown: "M4.5 6.25L8 9.75l3.5-3.5",
  chevronUp: "M4.5 9.75L8 6.25l3.5 3.5",
  chevronRight: "M6.25 4.5L9.75 8l-3.5 3.5",
  chevronLeft: "M9.75 4.5L6.25 8l3.5 3.5",
  arrowRight: "M3 8h10M9 4l4 4-4 4",
  arrowUp: "M8 13V3M4 7l4-4 4 4",
  arrowDown: "M8 3v10M4 9l4 4 4-4",
  // Components and readings
  cpu: chip,
  gpu: `${rect(1.75, 3.75, 12.5, 8, 1.5)}${circle(6, 7.75, 2)}M10 6.5h2M10 9h2M4 11.75v1.75`,
  memory: `${rect(1.75, 4.25, 12.5, 6.25, 1.25)}M4.5 6.75v1.75M7 6.75v1.75M9.5 6.75v1.75M4 10.5v1.75M6.5 10.5v1.75M9.5 10.5v1.75M12 10.5v1.75`,
  storage: `${rect(2, 3.5, 12, 9, 2)}M2 9h12M11 10.75v.01`,
  network: "M5.25 13V3.25M2.75 5.75l2.5-2.5 2.5 2.5M10.75 3v9.75M8.25 10.25l2.5 2.5 2.5-2.5",
  temperature: "M6.5 9.4V3.75a1.5 1.5 0 0 1 3 0V9.4a2.75 2.75 0 1 1-3 0zM8 7v3.5",
  fan: `${circle(8, 8, 6.25)}${circle(8, 8, 1.25)}M8 6.75C7.25 5 7.75 3.75 9.5 3.6M9.1 8.6c1.85.25 2.7 1.35 2.1 3M6.9 8.6c-1.1 1.5-2.5 1.75-3.6.5`,
  power: "M9 1.75L3.75 9h4.25l-1 5.25L12.25 7H8z",
  bolt: "M9 1.75L3.75 9h4.25l-1 5.25L12.25 7H8z",
  battery: `${rect(1.75, 4.75, 11, 6.5, 1.5)}M14.25 7v2M4.25 7.25v1.5`,
  gauge,
  clock: `${circle(8, 8, 6.25)}M8 4.75V8l2.25 1.5`,
  display: `${rect(1.75, 2.5, 12.5, 8.5, 1.25)}M5.75 13.75h4.5M8 11v2.75`,
  // Device types (form factors)
  desktop: `${rect(4.5, 1.75, 7, 12.5, 1.5)}M6.75 4.5h2.5M6.75 6.5h2.5M8 11.25v.01`,
  laptop: `${rect(3, 3, 10, 7.25, 1.25)}M1.5 12.75h13`,
  allInOne: `${rect(1.75, 1.75, 12.5, 9.5, 1.5)}M1.75 8.75h12.5M6.5 11.25l-.75 3h4.5l-.75-3`,
  miniPc: `${rect(2, 5.5, 12, 5, 1.75)}M4.5 8h.01M4.25 12.75h7.5`,
  // Appearance and language
  sun: `${circle(8, 8, 2.75)}${sunRays}`,
  moon: "M13.25 9.6A5.5 5.5 0 1 1 6.4 2.75a4.4 4.4 0 0 0 6.85 6.85z",
  globe: `${circle(8, 8, 6.25)}M1.75 8h12.5M8 1.75C6.25 3.5 5.5 5.65 5.5 8s.75 4.5 2.5 6.25C9.75 12.5 10.5 10.35 10.5 8S9.75 3.5 8 1.75z`,
  pulse: circle(8, 8, 2.25),
  // Control centers
  bulb: "M6 12.25h4M6.5 14.25h3M8 1.75a4.25 4.25 0 0 0-2.5 7.7c.4.3.75.85.75 1.35v.45h3.5v-.45c0-.5.35-1.05.75-1.35A4.25 4.25 0 0 0 8 1.75z",
  gamepad: "M5 4.75h6a3.25 3.25 0 0 1 3.2 2.7l.55 3.3a1.9 1.9 0 0 1-3.3 1.55L10 10.75H6l-1.45 1.55a1.9 1.9 0 0 1-3.3-1.55l.55-3.3A3.25 3.25 0 0 1 5 4.75zM4.75 7.25v2M3.75 8.25h2M10.75 7.75h.01M12 9.25h.01",
  sliders: `${circle(10, 4.5, 1.25)}${circle(5.5, 8, 1.25)}${circle(9, 11.5, 1.25)}M2.5 4.5h6.25M11.25 4.5h2.25M2.5 8h1.75M6.75 8h6.75M2.5 11.5h5.25M10.25 11.5h3.25`,
  snowflake: "M8 1.75v12.5M2.6 4.9l10.8 6.2M2.6 11.1l10.8-6.2M6.25 2.75L8 4.25l1.75-1.5M6.25 13.25L8 11.75l1.75 1.5",
  undo: "M3.75 6.25h6.5a3.25 3.25 0 0 1 0 6.5H6.5M6.25 3.5L3.5 6.25 6.25 9",
  // Gaming center: hide / show library entries.
  eye: `M1.75 8S4 3.75 8 3.75 14.25 8 14.25 8 12 12.25 8 12.25 1.75 8 1.75 8z${circle(8, 8, 2)}`,
  eyeOff: "M6.6 4a6 6 0 0 1 1.4-.25C12 3.75 14.25 8 14.25 8a11 11 0 0 1-1.7 2.2M4.2 5.1A10.6 10.6 0 0 0 1.75 8S4 12.25 8 12.25a5.9 5.9 0 0 0 3.1-.9M6.6 6.6a2 2 0 0 0 2.8 2.8M2.25 2.25l11.5 11.5",
} as const;

export type IconName = keyof typeof ICON_PATHS;

export const ICON_NAMES = Object.keys(ICON_PATHS) as IconName[];

export function isIconName(name: string): name is IconName {
  return Object.prototype.hasOwnProperty.call(ICON_PATHS, name);
}
