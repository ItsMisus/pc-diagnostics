import type { ReactNode } from "react";

/** The device families the stage can show. */
export type DeviceKind = "tower" | "laptop" | "mini";

export const DEVICE_KINDS: readonly DeviceKind[] = ["tower", "laptop", "mini"];

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
