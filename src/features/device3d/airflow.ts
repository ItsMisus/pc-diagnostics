import type { DeviceKind } from "./types";

/**
 * The air around the device model: where cool air goes in and warm air comes out, as streams
 * of small particles. Pure data plus the particle math, free of three.js (unit-tested).
 *
 * - `active`: the system reports fans. The paths are the usual ones for that kind of computer
 *   (a tower breathes in at the front and out at the back and top, a laptop in at the sides
 *   and out at the hinge), not a measurement: the stage says so in words.
 * - `passive`: no fans (e.g. a fanless laptop). No air is pushed through it; the heat leaves
 *   the case and rises with the air around it, which is what is drawn.
 *
 * Coordinates are model space (units of 10 cm, front towards +z, floor at y = 0), the same
 * as `presets.ts`, so the streams turn with the model.
 */
export type AirflowMode = "active" | "passive";

export type Vec3 = readonly [number, number, number];

export type StreamKind = "intake" | "exhaust" | "heat";

export interface AirStream {
  kind: StreamKind;
  /** Centre of the area where particles start. */
  from: Vec3;
  /** Half extents of that area (each particle keeps its own offset all the way). */
  spread: Vec3;
  /** Travel of a particle over one period. */
  travel: Vec3;
  count: number;
  /** Seconds for one particle to travel `travel`. */
  period: number;
  /** Particle radius, model units. */
  size: number;
  /** Sideways sway while travelling (heat shimmer), model units. */
  sway?: number;
}

const mirrorX = (s: AirStream): AirStream => ({
  ...s,
  from: [-s.from[0], s.from[1], s.from[2]],
  travel: [-s.travel[0], s.travel[1], s.travel[2]],
});

/** Laptop base: x ±1.52, z −0.73 … 1.43 (the model is shifted forward), top at 0.155. */
const LAPTOP_INTAKE: AirStream = { kind: "intake", from: [-2.05, 0.08, 0.25], spread: [0.04, 0.03, 0.5], travel: [0.5, 0, 0], count: 10, period: 1.7, size: 0.03 };

const STREAMS: Record<DeviceKind, Record<AirflowMode, readonly AirStream[]>> = {
  laptop: {
    active: [
      LAPTOP_INTAKE,
      mirrorX(LAPTOP_INTAKE),
      { kind: "exhaust", from: [0, 0.08, -0.8], spread: [1.0, 0.03, 0.02], travel: [0, 0.35, -1.0], count: 18, period: 1.5, size: 0.032 },
    ],
    passive: [{ kind: "heat", from: [0, 0.22, 0.05], spread: [1.15, 0.02, 0.45], travel: [0, 0.95, 0], count: 30, period: 4.2, size: 0.034, sway: 0.06 }],
  },
  tower: {
    active: [
      { kind: "intake", from: [0, 2.2, 3.15], spread: [0.6, 1.15, 0.04], travel: [0, 0, -0.95], count: 24, period: 1.6, size: 0.05 },
      { kind: "exhaust", from: [0, 3.35, -2.22], spread: [0.5, 0.45, 0.02], travel: [0, 0.1, -1.0], count: 12, period: 1.5, size: 0.05 },
      { kind: "exhaust", from: [0, 4.68, -0.55], spread: [0.6, 0.02, 1.0], travel: [0, 0.95, 0], count: 14, period: 1.8, size: 0.05 },
    ],
    passive: [{ kind: "heat", from: [0, 4.7, 0], spread: [0.8, 0.02, 1.7], travel: [0, 1.0, 0], count: 22, period: 4.2, size: 0.05, sway: 0.08 }],
  },
  mini: {
    active: [
      { kind: "intake", from: [-0.95, 0.2, 0.1], spread: [0.02, 0.07, 0.4], travel: [0.3, 0, 0], count: 7, period: 1.6, size: 0.016 },
      { kind: "intake", from: [0.95, 0.2, 0.1], spread: [0.02, 0.07, 0.4], travel: [-0.3, 0, 0], count: 7, period: 1.6, size: 0.016 },
      { kind: "exhaust", from: [0, 0.22, -0.68], spread: [0.45, 0.07, 0.02], travel: [0, 0.06, -0.55], count: 12, period: 1.5, size: 0.016 },
    ],
    passive: [{ kind: "heat", from: [0, 0.47, 0], spread: [0.5, 0.01, 0.5], travel: [0, 0.5, 0], count: 16, period: 4.2, size: 0.016, sway: 0.03 }],
  },
  allInOne: {
    active: [
      { kind: "intake", from: [0, 0.42, 0.28], spread: [1.9, 0.04, 0.04], travel: [0, 0.3, 0], count: 16, period: 1.7, size: 0.05 },
      { kind: "exhaust", from: [0, 4.2, 0.22], spread: [1.9, 0.02, 0.04], travel: [0, 0.8, -0.25], count: 16, period: 1.6, size: 0.05 },
    ],
    passive: [{ kind: "heat", from: [0, 4.2, 0.3], spread: [2.3, 0.02, 0.05], travel: [0, 0.9, 0], count: 22, period: 4.2, size: 0.05, sway: 0.08 }],
  },
};

export function airStreams(kind: DeviceKind, mode: AirflowMode): readonly AirStream[] {
  return STREAMS[kind][mode];
}

/** Smallest box around the model and its streams: [minX, minY, minZ, maxX, maxY, maxZ]. */
export function airflowBounds(kind: DeviceKind, mode: AirflowMode, size: readonly [number, number, number]): [number, number, number, number, number, number] {
  const b: [number, number, number, number, number, number] = [-size[0] / 2, 0, -size[2] / 2, size[0] / 2, size[1], size[2] / 2];
  for (const s of airStreams(kind, mode)) {
    for (const end of [0, 1]) {
      for (let a = 0; a < 3; a++) {
        const c = (s.from[a] ?? 0) + end * (s.travel[a] ?? 0);
        const r = (s.spread[a] ?? 0) + (s.sway ?? 0) + s.size;
        b[a] = Math.min(b[a] ?? 0, c - r);
        b[a + 3] = Math.max(b[a + 3] ?? 0, c + r);
      }
    }
  }
  return b;
}

/** Deterministic pseudo-random in [0, 1) for particle `i` of stream `s`, channel `c`. */
export function jitter(s: number, i: number, c: number): number {
  const x = Math.sin(s * 127.1 + i * 311.7 + c * 74.7) * 43758.5453;
  return x - Math.floor(x);
}

export interface Particle {
  position: [number, number, number];
  /** 0 … 1: the particle grows in, then fades out at the end of its path. */
  scale: number;
}

/**
 * Particle `i` of stream `stream` (index `si`) at time `t` seconds. Each particle starts at
 * its own phase so the stream looks continuous; it keeps its offset in the start area, so
 * the stream reads as parallel lines of air.
 */
export function particleAt(stream: AirStream, si: number, i: number, t: number): Particle {
  const phase = (((t / stream.period + i / stream.count + jitter(si, i, 3) * 0.35) % 1) + 1) % 1;
  const off = [0, 1, 2].map((c) => (jitter(si, i, c) * 2 - 1) * (stream.spread[c] ?? 0));
  const sway = stream.sway ? Math.sin(phase * Math.PI * 2 + jitter(si, i, 4) * 6.28) * stream.sway : 0;
  const position: [number, number, number] = [
    stream.from[0] + (off[0] ?? 0) + stream.travel[0] * phase + sway,
    stream.from[1] + (off[1] ?? 0) + stream.travel[1] * phase,
    stream.from[2] + (off[2] ?? 0) + stream.travel[2] * phase + sway * 0.5,
  ];
  // Grow in over the first 15 %, fade over the last 35 %.
  const scale = Math.min(1, phase / 0.15, (1 - phase) / 0.35);
  return { position, scale: Math.max(0, scale) };
}
