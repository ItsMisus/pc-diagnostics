import type { ReactNode } from "react";

/**
 * The device families the stage can show.
 *
 * Adding a model (the list is meant to grow):
 * 1. its id here and in `DEVICE_KINDS`, and in the Rust `DeviceShape` (crates/models/src/device.rs)
 *    with the form factors that map to it (`FormFactor::shape`), then `npm run gen:types`;
 * 2. staging in `presets.ts` (size in 10 cm units, hero angle, lens, padding);
 * 3. the 3D model in `scene/<Name>.tsx` (procedural, materials from `useMaterials()`), listed
 *    in `scene/models.ts`;
 * 4. the static fallback in `illustration.ts` (`DRAW`), the i18n names (`model_*`,
 *    `illustration_*` in device3d, `shape_*` in overview and settings) and an icon;
 * 5. `device3d.test.ts` covers framing and the fallback for every kind automatically.
 */
export type DeviceKind = "tower" | "laptop" | "mini" | "allInOne";

export const DEVICE_KINDS: readonly DeviceKind[] = ["tower", "laptop", "mini", "allInOne"];

/** Resolved colour scheme of the surface behind the stage. */
export type StageTheme = "light" | "dark";

/** What the stage is currently drawing. */
export type StageMode = "loading" | "3d" | "fallback";

/**
 * Screen-space slots around the model for small indicators (CPU, GPU, RAM, Storage...).
 * They sit in the free space around the device and never point at a physical location:
 * the model is generic, so a label next to it must not suggest "the GPU is here".
 * `start` / `end` follow the reading direction.
 */
export type IndicatorSlot = "top-start" | "top-end" | "start" | "end" | "bottom-start" | "bottom-end";

export const INDICATOR_SLOTS: readonly IndicatorSlot[] = ["top-start", "top-end", "start", "end", "bottom-start", "bottom-end"];

export interface DeviceIndicator {
  /** Stable React key. */
  id: string;
  slot: IndicatorSlot;
  /** The indicator itself (the caller styles it; it stays interactive). */
  content: ReactNode;
}
