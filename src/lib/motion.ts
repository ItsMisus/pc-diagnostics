import type { Transition, Variants } from "motion/react";

/**
 * Motion design system: the single source of durations, easings and springs.
 * CSS mirrors these as `--dur-*` and `--ease-*` in `src/styles/index.css`
 * (docs/design-system.md, "Motion").
 *
 * Principles:
 * - Every animation answers a user action or a navigation. Live data never animates.
 * - Animate only `transform` and `opacity` (plus colors); never layout properties per frame.
 * - Control feedback 100–180 ms, panels and overlays 200–350 ms, exits faster than entrances.
 * - Springs settle without visible overshoot (indicators) or with a hint of it (toggles).
 * - Reduced motion is honored globally by `<MotionProvider>` (`reducedMotion="user"`):
 *   transforms are dropped, opacity stays. Code outside Motion (CSS, WebGL) reads
 *   `useReducedMotion()` from `motion/react` or `prefersReducedMotion()` below.
 */

/** Seconds, as Motion expects. Mirrors `--dur-*` (ms) in index.css. */
export const duration = {
  /** Press feedback, checkbox ticks. */
  instant: 0.1,
  /** Hover, toggles, color changes. */
  micro: 0.14,
  /** Tooltips, menus, small popovers. */
  fast: 0.18,
  /** Disclosure, toasts, phase changes inside a panel. */
  component: 0.24,
  /** Modals, sheets, large panels. */
  panel: 0.3,
  /** Page transitions, sidebar width. */
  nav: 0.3,
  /** Exits: always shorter than the matching entrance. */
  exit: 0.12,
} as const;

export const ease = {
  /** Strong ease-out: fast start, gentle settle. Entrances and responses. */
  out: [0.23, 1, 0.32, 1],
  inOut: [0.77, 0, 0.175, 1],
  /** Exits: accelerate away. */
  in: [0.5, 0, 0.75, 0],
  /** Material-like standard curve for color/opacity cross-fades. */
  standard: [0.2, 0, 0, 1],
} as const satisfies Record<string, readonly [number, number, number, number]>;

export const spring = {
  /** Selection indicators (sidebar, segmented controls): quick, no visible overshoot. */
  indicator: { type: "spring", stiffness: 520, damping: 42, mass: 0.9 },
  /** Switch thumbs and small toggles: a hint of bounce. */
  snappy: { type: "spring", stiffness: 640, damping: 34 },
  /** Toasts and small floating panels. */
  gentle: { type: "spring", visualDuration: 0.3, bounce: 0.08 },
  /** Modals and sheets: settles in ~300 ms, no bounce. */
  panel: { type: "spring", visualDuration: 0.32, bounce: 0 },
} as const satisfies Record<string, Transition>;

/** Page enter/exit. Exit is short so navigation never feels blocked; no layout shift (opacity + 4 px). */
export const pageVariants: Variants = {
  initial: { opacity: 0, y: 4 },
  enter: { opacity: 1, y: 0, transition: { duration: duration.nav, ease: ease.out } },
  exit: { opacity: 0, transition: { duration: 0.09, ease: ease.in } },
};

/** Small floating surfaces (tooltips, menus). */
export const popoverTransition: Transition = { duration: duration.fast, ease: ease.out };

/** Backdrop of a modal or sheet. */
export const backdropVariants: Variants = {
  hidden: { opacity: 0, transition: { duration: duration.exit, ease: ease.in } },
  shown: { opacity: 1, transition: { duration: duration.component, ease: ease.out } },
};

/** Centered dialog: rises 8 px and scales from 97 %. */
export const dialogVariants: Variants = {
  hidden: { opacity: 0, y: 8, scale: 0.97, transition: { duration: duration.exit, ease: ease.in } },
  shown: { opacity: 1, y: 0, scale: 1, transition: spring.panel },
};

/** Trailing side sheet: slides 24 px from the right edge. */
export const sheetVariants: Variants = {
  hidden: { opacity: 0, x: 24, transition: { duration: duration.exit, ease: ease.in } },
  shown: { opacity: 1, x: 0, transition: spring.panel },
};

/** Whether the OS asks for reduced motion (for code outside React/Motion, e.g. a render loop). */
export function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-reduced-motion: reduce)").matches
    : false;
}

/** Milliseconds of a duration token, for `setTimeout` and CSS-in-JS. */
export const ms = (seconds: number): number => Math.round(seconds * 1000);
