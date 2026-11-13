import type { DeviceKind, IndicatorSlot } from "./types";

/**
 * Per-model staging: size, hero angle, rotation limits and lens. Pure data plus the framing
 * math, free of three.js so the main bundle and the unit tests can use it.
 *
 * Units: 1 = 10 cm. Every model is built with its bounding box centred on x/z and standing on
 * the floor (y = 0 … height), front facing +z. Yaw turns the model on its "turntable"
 * (positive = front turns to the viewer's right, the left side comes into view); elevation is
 * the camera's angle above the horizon.
 */
export interface DevicePreset {
  /** Bounding box [width, height, depth]. */
  size: readonly [number, number, number];
  /** Hero yaw, radians. */
  heroYaw: number;
  /** Hero camera elevation, radians. */
  heroElevation: number;
  /** Allowed yaw offset from the hero angle, ± radians. */
  yawLimit: number;
  /** Allowed elevation offsets from the hero elevation, radians [min, max]. */
  elevationRange: readonly [number, number];
  /** Vertical field of view, degrees (long lens: little perspective distortion). */
  fov: number;
  /** Fraction of the frame kept free around the model at the hero pose. */
  padding: number;
  /** Height of the aim point, as a fraction of the model height. */
  aimHeight: number;
  /** Soft floor shadow footprint (model units) and its z offset. */
  shadow: { width: number; depth: number; z: number };
  /** Amplitude of the ambient float, model units. */
  floatAmplitude: number;
}

const deg = (d: number) => (d * Math.PI) / 180;

export const PRESETS: Record<DeviceKind, DevicePreset> = {
  tower: {
    size: [2.1, 4.6, 4.3],
    heroYaw: deg(36),
    heroElevation: deg(12),
    yawLimit: deg(50),
    elevationRange: [deg(-6), deg(12)],
    fov: 26,
    // Tall model: 0.1 let its base reach the bottom indicator chips.
    padding: 0.15,
    aimHeight: 0.5,
    shadow: { width: 2.1, depth: 4.3, z: 0 },
    floatAmplitude: 0.035,
  },
  laptop: {
    size: [3.04, 2.08, 2.92],
    heroYaw: deg(30),
    heroElevation: deg(20),
    yawLimit: deg(50),
    elevationRange: [deg(-8), deg(12)],
    fov: 26,
    padding: 0.08,
    aimHeight: 0.42,
    shadow: { width: 3.04, depth: 2.15, z: 0.35 },
    floatAmplitude: 0.02,
  },
  mini: {
    size: [1.27, 0.43, 1.27],
    heroYaw: deg(34),
    heroElevation: deg(22),
    yawLimit: deg(50),
    elevationRange: [deg(-8), deg(12)],
    fov: 24,
    padding: 0.2,
    aimHeight: 0.45,
    shadow: { width: 1.27, depth: 1.27, z: 0 },
    floatAmplitude: 0.012,
  },
  allInOne: {
    size: [5.4, 4.15, 1.4],
    heroYaw: deg(28),
    heroElevation: deg(10),
    yawLimit: deg(50),
    elevationRange: [deg(-6), deg(12)],
    fov: 26,
    padding: 0.08,
    aimHeight: 0.5,
    shadow: { width: 2.2, depth: 1.5, z: 0 },
    floatAmplitude: 0.02,
  },
};

export function presetFor(kind: DeviceKind): DevicePreset {
  return PRESETS[kind];
}

export interface Frame {
  /** Camera distance from the aim point. */
  distance: number;
  /** Aim point (model space, before yaw; on the turntable axis). */
  target: readonly [number, number, number];
  fov: number;
}

export interface FrameOptions {
  /** Extra horizontal padding (fraction of the half width), e.g. for side indicators. */
  extraPadX?: number;
}

/**
 * Camera distance needed so that the box, turned by `yaw` and seen from `elevation`, fits
 * the viewport with the given padding on each axis. Exact for the 8 corners of the box.
 */
export function fitDistance(
  size: readonly [number, number, number],
  aimY: number,
  yaw: number,
  elevation: number,
  fovDeg: number,
  aspect: number,
  padX: number,
  padY: number,
): number {
  const [w, h, d] = size;
  const tanV = Math.tan((fovDeg * Math.PI) / 360);
  const tanH = tanV * Math.max(aspect, 0.05);
  const limX = tanH * Math.max(1 - padX, 0.05);
  const limY = tanV * Math.max(1 - padY, 0.05);
  const cy = Math.cos(yaw);
  const sy = Math.sin(yaw);
  const ce = Math.cos(elevation);
  const se = Math.sin(elevation);
  let distance = 0;
  for (const x of [-w / 2, w / 2]) {
    for (const y of [0, h]) {
      for (const z of [-d / 2, d / 2]) {
        // Turntable yaw (rotation about +y).
        const rx = x * cy + z * sy;
        const rz = -x * sy + z * cy;
        const ry = y - aimY;
        // View space: right = +x, up = (0, ce, -se), towards camera = (0, se, ce).
        const vx = rx;
        const vy = ry * ce - rz * se;
        const vz = ry * se + rz * ce;
        distance = Math.max(distance, vz + Math.abs(vx) / limX, vz + Math.abs(vy) / limY);
      }
    }
  }
  return distance;
}

/**
 * The framing for a model in a viewport of the given aspect ratio. The hero pose keeps its
 * padding; any pose within the rotation limits still fits (with a hairline margin, plus the
 * room kept for side indicators), so a drag never clips the model and the camera never
 * "zooms" while rotating.
 */
export function frameFor(kind: DeviceKind, aspect: number, options: FrameOptions = {}): Frame {
  const p = PRESETS[kind];
  const aimY = p.size[1] * p.aimHeight;
  const padX = Math.min(p.padding + (options.extraPadX ?? 0), 0.6);
  let distance = fitDistance(p.size, aimY, p.heroYaw, p.heroElevation, p.fov, aspect, padX, p.padding);
  const [eMin, eMax] = p.elevationRange;
  for (const dy of [-1, -0.5, 0, 0.5, 1]) {
    for (const e of [eMin, 0, eMax]) {
      const pose = fitDistance(p.size, aimY, p.heroYaw + dy * p.yawLimit, p.heroElevation + e, p.fov, aspect, 0.02 + (options.extraPadX ?? 0), 0.02);
      distance = Math.max(distance, pose);
    }
  }
  return { distance, target: [0, aimY, 0], fov: p.fov };
}

/** Side slots take horizontal room: the model is framed narrower when any of them is used. */
export function extraPaddingFor(slots: readonly IndicatorSlot[]): number {
  return slots.length > 0 ? 0.22 : 0;
}

/** Camera position for an elevation and distance around the aim point (camera on +z). */
export function cameraPosition(target: readonly [number, number, number], distance: number, elevation: number): [number, number, number] {
  return [target[0], target[1] + distance * Math.sin(elevation), target[2] + distance * Math.cos(elevation)];
}
