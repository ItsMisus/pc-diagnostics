import type { Transition, Variants } from "motion/react";

/**
 * Motion design system: the single source of durations, easings and springs.
 * CSS mirrors these as `--dur-*` and `--ease-*` in `src/styles/index.css`.
 *
 * Principles:
 * - Animate only `transform` and `opacity` (plus colors); never layout properties per frame.
 * - Ease-out for anything that enters or responds to the user; nothing slower than 350 ms.
 * - Live data never triggers an entrance animation: only mount, navigation and user actions do.
 * - Reduced motion is honored globally by `<MotionProvider>` (`reducedMotion="user"`).
 */

/** Seconds, as Motion expects. */
export const duration = {
  /** Hover, press, toggles, checkboxes. */
  micro: 0.14,
  /** Popovers, tooltips, toasts, disclosure. */
  component: 0.22,
  /** Page and large surface transitions. */
  nav: 0.28,
} as const;

export const ease = {
  /** Strong ease-out: fast start, gentle settle. */
  out: [0.23, 1, 0.32, 1],
  inOut: [0.77, 0, 0.175, 1],
  /** For exits: accelerate away. */
  in: [0.5, 0, 0.75, 0],
} as const satisfies Record<string, readonly [number, number, number, number]>;

export const spring = {
  /** Selection indicators (sidebar, segmented controls): quick, no visible overshoot. */
  indicator: { type: "spring", stiffness: 520, damping: 42, mass: 0.9 },
  /** Switch thumbs and small toggles: a hint of bounce. */
  snappy: { type: "spring", stiffness: 640, damping: 34 },
  /** Larger surfaces (toasts, panels). */
  gentle: { type: "spring", visualDuration: 0.3, bounce: 0.08 },
} as const satisfies Record<string, Transition>;

/** Page enter/exit. Exit is short so navigation never feels blocked. */
export const pageVariants: Variants = {
  initial: { opacity: 0, y: 6 },
  enter: { opacity: 1, y: 0, transition: { duration: duration.nav, ease: ease.out } },
  exit: { opacity: 0, transition: { duration: 0.09, ease: ease.in } },
};

/** Small floating surfaces (tooltips, menus). */
export const popoverTransition: Transition = { duration: duration.micro, ease: ease.out };
