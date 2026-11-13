/**
 * Rotation of the model: drag, keyboard, limits and the soft return to the hero angle.
 * Pure and frame-rate independent (unit-tested); the scene calls `step()` once per frame
 * and asks `needsFrame()` / `wakeAt()` whether it must keep rendering.
 *
 * Offsets are relative to the preset's hero pose: yaw (turntable) and camera elevation.
 */

export const clamp = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v));

/**
 * Rubber band past a limit: free inside [-limit, limit], then a resistance that tends to
 * `limit + limit * stretch` however far the pointer goes (the iOS scroll-view feel).
 */
export function rubberBand(value: number, lo: number, hi: number, stretch = 0.18): number {
  if (value >= lo && value <= hi) return value;
  const range = Math.max(hi - lo, 1e-6);
  const max = (range / 2) * stretch;
  const over = value > hi ? value - hi : lo - value;
  const eased = (1 - 1 / ((over * 0.55) / max + 1)) * max;
  return value > hi ? hi + eased : lo - eased;
}

export interface SpringParams {
  stiffness: number;
  damping: number;
}

/** Drag following: fast, no overshoot. */
export const FOLLOW_SPRING: SpringParams = { stiffness: 420, damping: 42 };
/** Return to the hero angle and release past a limit: soft, a whisper of overshoot. */
export const RETURN_SPRING: SpringParams = { stiffness: 46, damping: 12.5 };

/** One spring integration step (semi-implicit Euler, sub-stepped for stability). */
export function springStep(x: number, v: number, target: number, dt: number, p: SpringParams): [number, number] {
  let pos = x;
  let vel = v;
  let left = Math.min(Math.max(dt, 0), 0.1);
  while (left > 1e-6) {
    const h = Math.min(left, 1 / 240);
    vel += (-p.stiffness * (pos - target) - p.damping * vel) * h;
    pos += vel * h;
    left -= h;
  }
  return [pos, vel];
}

/** Keyboard control: arrows rotate by a step, Home (Fn+← on a Mac) restores the hero view. */
export type KeyAction = { kind: "nudge"; yaw: number; elevation: number } | { kind: "reset" };

export const KEY_YAW_STEP = (12 * Math.PI) / 180;
export const KEY_ELEVATION_STEP = (4 * Math.PI) / 180;

export function keyAction(key: string): KeyAction | null {
  switch (key) {
    case "ArrowLeft":
      return { kind: "nudge", yaw: -KEY_YAW_STEP, elevation: 0 };
    case "ArrowRight":
      return { kind: "nudge", yaw: KEY_YAW_STEP, elevation: 0 };
    case "ArrowUp":
      return { kind: "nudge", yaw: 0, elevation: KEY_ELEVATION_STEP };
    case "ArrowDown":
      return { kind: "nudge", yaw: 0, elevation: -KEY_ELEVATION_STEP };
    case "Home":
      return { kind: "reset" };
    default:
      return null;
  }
}

export interface OrbitLimits {
  /** ± yaw offset, radians. */
  yaw: number;
  /** Elevation offset range, radians. */
  elevation: readonly [number, number];
}

export interface OrbitOptions {
  limits: OrbitLimits;
  /** Reduced motion: the model follows input directly, no springs, no automatic return. */
  reducedMotion?: boolean;
  /** Idle time after the last input before the model drifts back to the hero angle, ms. */
  returnDelay?: number;
}

/** Radians of yaw per viewport width dragged. */
export const DRAG_YAW_GAIN = Math.PI * 0.9;
/** Radians of elevation per viewport height dragged. */
export const DRAG_ELEVATION_GAIN = Math.PI * 0.35;

const EPS = 1e-4;

export class OrbitController {
  /** Current offsets from the hero pose. */
  yaw = 0;
  elevation = 0;
  private vYaw = 0;
  private vElevation = 0;
  private targetYaw = 0;
  private targetElevation = 0;
  /** Unbounded drag accumulators (the rubber band is applied on top). */
  private rawYaw = 0;
  private rawElevation = 0;
  private dragging = false;
  private lastInput = -Infinity;
  private limits: OrbitLimits;
  private reduced: boolean;
  private readonly returnDelay: number;

  constructor(options: OrbitOptions) {
    this.limits = options.limits;
    this.reduced = options.reducedMotion ?? false;
    this.returnDelay = options.returnDelay ?? 1800;
  }

  setLimits(limits: OrbitLimits): void {
    this.limits = limits;
    this.targetYaw = clamp(this.targetYaw, -limits.yaw, limits.yaw);
    this.targetElevation = clamp(this.targetElevation, limits.elevation[0], limits.elevation[1]);
  }

  setReducedMotion(reduced: boolean): void {
    this.reduced = reduced;
  }

  get isDragging(): boolean {
    return this.dragging;
  }

  dragStart(now: number): void {
    this.dragging = true;
    this.rawYaw = this.targetYaw;
    this.rawElevation = this.targetElevation;
    this.lastInput = now;
  }

  /** Pointer moved by (dx, dy) CSS pixels over a viewport of (width, height). */
  dragBy(dx: number, dy: number, width: number, height: number, now: number): void {
    if (!this.dragging) return;
    this.rawYaw += (dx / Math.max(width, 1)) * DRAG_YAW_GAIN;
    // Dragging down looks from higher up, like grabbing the scene.
    this.rawElevation += (dy / Math.max(height, 1)) * DRAG_ELEVATION_GAIN;
    const { yaw, elevation } = this.limits;
    if (this.reduced) {
      this.targetYaw = clamp(this.rawYaw, -yaw, yaw);
      this.targetElevation = clamp(this.rawElevation, elevation[0], elevation[1]);
    } else {
      this.targetYaw = rubberBand(this.rawYaw, -yaw, yaw);
      this.targetElevation = rubberBand(this.rawElevation, elevation[0], elevation[1]);
    }
    this.lastInput = now;
  }

  dragEnd(now: number): void {
    if (!this.dragging) return;
    this.dragging = false;
    const { yaw, elevation } = this.limits;
    this.targetYaw = clamp(this.targetYaw, -yaw, yaw);
    this.targetElevation = clamp(this.targetElevation, elevation[0], elevation[1]);
    this.lastInput = now;
  }

  /** Keyboard (or programmatic) step, clamped to the limits. */
  nudge(dYaw: number, dElevation: number, now: number): void {
    const { yaw, elevation } = this.limits;
    this.targetYaw = clamp(this.targetYaw + dYaw, -yaw, yaw);
    this.targetElevation = clamp(this.targetElevation + dElevation, elevation[0], elevation[1]);
    this.lastInput = now;
  }

  /** Back to the hero pose. */
  reset(now: number): void {
    this.targetYaw = 0;
    this.targetElevation = 0;
    this.lastInput = now;
    if (this.reduced) this.snap();
  }

  /** Jump to the targets with no motion (kind swaps, reduced motion). */
  snap(): void {
    this.yaw = this.targetYaw;
    this.elevation = this.targetElevation;
    this.vYaw = 0;
    this.vElevation = 0;
  }

  /** Back to the hero pose immediately (e.g. a new model). */
  home(): void {
    this.targetYaw = 0;
    this.targetElevation = 0;
    this.rawYaw = 0;
    this.rawElevation = 0;
    this.snap();
  }

  /** Advance by dt seconds at time `now` (ms). */
  step(dt: number, now: number): void {
    if (!this.dragging && !this.reduced && now - this.lastInput >= this.returnDelay) {
      this.targetYaw = 0;
      this.targetElevation = 0;
    }
    if (this.reduced) {
      this.snap();
      return;
    }
    const p = this.dragging ? FOLLOW_SPRING : RETURN_SPRING;
    [this.yaw, this.vYaw] = springStep(this.yaw, this.vYaw, this.targetYaw, dt, p);
    [this.elevation, this.vElevation] = springStep(this.elevation, this.vElevation, this.targetElevation, dt, p);
    if (this.atRest()) this.snap();
  }

  private atRest(): boolean {
    return (
      Math.abs(this.yaw - this.targetYaw) < EPS &&
      Math.abs(this.elevation - this.targetElevation) < EPS &&
      Math.abs(this.vYaw) < EPS &&
      Math.abs(this.vElevation) < EPS
    );
  }

  /** True while the pose is still changing (or the pointer is down). */
  needsFrame(): boolean {
    return this.dragging || !this.atRest();
  }

  /** At rest exactly on the hero pose (ambient motion may play). */
  atHero(): boolean {
    return !this.dragging && this.atRest() && Math.abs(this.yaw) < EPS && Math.abs(this.elevation) < EPS;
  }

  /**
   * When the controller will need a frame again with no new input (the return to the hero
   * angle), in ms; null if never.
   */
  wakeAt(): number | null {
    if (this.dragging || this.reduced) return null;
    if (this.targetYaw === 0 && this.targetElevation === 0) return null;
    return this.lastInput + this.returnDelay;
  }
}
