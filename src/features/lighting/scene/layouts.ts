import { keyboardLayout } from "@/features/device3d/keyboard";
import type { LightingColor } from "@/types/generated/LightingColor";
import type { LightingDeviceKind } from "@/types/generated/LightingDeviceKind";
import type { LightingEffect } from "@/types/generated/LightingEffect";
import { colorAt, dim, levelAt } from "../effect";

/** The procedural models of the lighting stage. */
export type StageModel = "keyboard" | "mouse" | "fan" | "strip" | "backlight";

export const STAGE_MODELS: readonly StageModel[] = ["keyboard", "mouse", "fan", "strip", "backlight"];

/** Which model stands for a detected device. */
export function modelForKind(kind: LightingDeviceKind): StageModel {
  switch (kind) {
    case "keyboard":
      return "keyboard";
    case "mouse":
      return "mouse";
    case "fan":
      return "fan";
    case "keyboardBacklight":
      return "backlight";
    default:
      return "strip";
  }
}

/** One lamp of a model: centre (x right, z towards the viewer, y up), footprint, effect position. */
export interface Lamp {
  x: number;
  y: number;
  z: number;
  w: number;
  d: number;
  /** Place along the device for the effect math, 0 (left / start) to 1. */
  u: number;
}

export interface ModelLayout {
  lamps: Lamp[];
  /** Footprint of the body (top view), for the SVG fallback. */
  width: number;
  depth: number;
  /** The lamps sit on a ring (fan): drawn as circles. */
  ring: boolean;
}

const normalize = (lamps: Omit<Lamp, "u">[]): Lamp[] => {
  const xs = lamps.map((l) => l.x);
  const min = Math.min(...xs);
  const max = Math.max(...xs);
  return lamps.map((l) => ({ ...l, u: max - min > 1e-6 ? (l.x - min) / (max - min) : 0.5 }));
};

export const KEY_UNIT = 0.2;
export const KEY_GAP = 0.028;
export const LAPTOP_KEY_UNIT = 0.175;

/** Lamps of each model, built once per model (pure; also drawn by the SVG fallback). */
export function modelLayout(model: StageModel): ModelLayout {
  switch (model) {
    case "keyboard": {
      const keys = keyboardLayout(KEY_UNIT, KEY_GAP);
      return { lamps: normalize(keys.map((k) => ({ x: k.x, y: 0.2, z: k.z, w: k.w, d: k.d }))), width: 15 * KEY_UNIT + 0.3, depth: 6.55 * KEY_UNIT + 0.3, ring: false };
    }
    case "backlight": {
      const keys = keyboardLayout(LAPTOP_KEY_UNIT, 0.03);
      return { lamps: normalize(keys.map((k) => ({ x: k.x, y: 0.17, z: k.z - 0.35, w: k.w, d: k.d }))), width: 3.04, depth: 2.15, ring: false };
    }
    case "mouse": {
      // A light strip around the lower rim (open at the front, like most gaming mice) + logo.
      const n = 18;
      const lamps: Omit<Lamp, "u">[] = [];
      for (let i = 0; i < n; i++) {
        const a = Math.PI * 0.18 + (i / (n - 1)) * Math.PI * 1.64;
        lamps.push({ x: Math.sin(a) * 0.62, y: 0.065, z: -Math.cos(a) * 1.02, w: 0.1, d: 0.1 });
      }
      lamps.push({ x: 0, y: 0.545, z: 0.45, w: 0.18, d: 0.18 });
      return { lamps: normalize(lamps), width: 1.3, depth: 2.1, ring: false };
    }
    case "fan": {
      const n = 24;
      const lamps: Lamp[] = [];
      for (let i = 0; i < n; i++) {
        const a = (i / n) * Math.PI * 2;
        lamps.push({ x: Math.cos(a) * 1.02, y: Math.sin(a) * 1.02, z: 0.2, w: 0.12, d: 0.12, u: i / n });
      }
      return { lamps, width: 2.4, depth: 2.4, ring: true };
    }
    case "strip": {
      const n = 30;
      const lamps: Omit<Lamp, "u">[] = [];
      for (let i = 0; i < n; i++) lamps.push({ x: -1.74 + (i / (n - 1)) * 3.48, y: 0.06, z: 0, w: 0.07, d: 0.07 });
      return { lamps: normalize(lamps), width: 3.8, depth: 0.3, ring: false };
    }
  }
}

/**
 * Colors of a model's lamps at `tMs`, with the shared effect math. The MacBook backlight is a
 * single white light: it shows the effect's level only (as the real one does).
 */
export function lampColors(model: StageModel, layout: ModelLayout, effect: LightingEffect, brightness: number, tMs: number): LightingColor[] {
  const n = layout.lamps.length;
  if (model === "backlight") {
    const v = Math.round(255 * levelAt(effect, tMs) * (Math.max(0, Math.min(100, brightness)) / 100));
    const white: LightingColor = { r: v, g: v, b: Math.min(255, Math.round(v * 1.02)) };
    return layout.lamps.map(() => white);
  }
  return layout.lamps.map((l, i) => dim(colorAt(effect, tMs, i, n, l.u), brightness));
}
