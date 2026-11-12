/**
 * Motion rules of the stage, pure (unit-tested): when to render at all, the ambient float,
 * and the swap between two models.
 */

export interface MotionPolicy {
  /** Slow float / yaw while idle on the hero angle. */
  ambient: boolean;
  /** Model swap: scale + turn + fade, or a plain short cross-fade. */
  swap: "animated" | "fade";
}

/** Reduced motion turns off everything that moves on its own; direct manipulation stays. */
export function motionPolicy(opts: { reducedMotion: boolean; ambient: boolean }): MotionPolicy {
  return {
    ambient: opts.ambient && !opts.reducedMotion,
    swap: opts.reducedMotion ? "fade" : "animated",
  };
}

/**
 * Whether the scene may render: not paused by the caller (benchmark running, overview not
 * shown), the document visible and the stage on screen. When false nothing renders at all.
 */
export function isStageActive(opts: { paused: boolean; documentHidden: boolean; inView: boolean }): boolean {
  return !opts.paused && !opts.documentHidden && opts.inView;
}

/** Frame interval used while only the ambient motion plays (it is slow: 30 fps suffice). */
export const AMBIENT_FRAME_MS = 1000 / 30;

const TAU = Math.PI * 2;
const FLOAT_PERIOD = 7.5;
const YAW_PERIOD = 17;
const YAW_AMPLITUDE = (1.6 * Math.PI) / 180;

/** Ambient offset at ambient time `t` (seconds): a slow float and a slower yaw sway. */
export function ambientPose(t: number, floatAmplitude: number): { y: number; yaw: number } {
  return {
    y: floatAmplitude * (0.5 - 0.5 * Math.cos((TAU * t) / FLOAT_PERIOD)),
    yaw: YAW_AMPLITUDE * Math.sin((TAU * t) / YAW_PERIOD),
  };
}

/** Exponential approach, frame-rate independent (three's MathUtils.damp). */
export function damp(current: number, target: number, lambda: number, dt: number): number {
  return current + (target - current) * (1 - Math.exp(-lambda * dt));
}

// ---------------------------------------------------------------------------------------
// Swap timeline
// ---------------------------------------------------------------------------------------

export interface SwapSample {
  phase: "out" | "in" | "idle";
  /** Canvas opacity. */
  opacity: number;
  /** Model scale. */
  scale: number;
  /** Extra yaw on top of the pose, radians. */
  yaw: number;
  /** True exactly once: the out phase is over, mount the next model now. */
  switchNow: boolean;
}

/** Durations, ms. The whole swap stays within ~550 ms. */
export const SWAP_TIMING = {
  animated: { out: 190, in: 360 },
  fade: { out: 110, in: 150 },
} as const;

const SWAP_SCALE = 0.94;
const SWAP_YAW = (16 * Math.PI) / 180;

const easeIn = (x: number) => x * x * x;
const easeOut = (x: number) => 1 - (1 - x) ** 3;

export class SwapAnimator {
  private phase: "out" | "in" | "idle" = "idle";
  private start = 0;
  private style: MotionPolicy["swap"] = "animated";
  private switched = false;

  get busy(): boolean {
    return this.phase !== "idle";
  }

  /** Leave the current model, then `switchNow` once, then enter the next. */
  swap(now: number, style: MotionPolicy["swap"]): void {
    if (this.phase === "out") return;
    this.phase = "out";
    this.start = now;
    this.style = style;
    this.switched = false;
  }

  /** Only the entrance (first appearance). */
  enter(now: number, style: MotionPolicy["swap"]): void {
    this.phase = "in";
    this.start = now;
    this.style = style;
    this.switched = true;
  }

  /** Stop wherever it is and show the model at rest. */
  finish(): void {
    this.phase = "idle";
  }

  sample(now: number): SwapSample {
    const timing = SWAP_TIMING[this.style];
    const animated = this.style === "animated";
    if (this.phase === "out") {
      const x = Math.min((now - this.start) / timing.out, 1);
      if (x >= 1) {
        this.phase = "in";
        this.start = now;
        const first = !this.switched;
        this.switched = true;
        return { phase: "in", opacity: 0, scale: animated ? SWAP_SCALE : 1, yaw: animated ? -SWAP_YAW : 0, switchNow: first };
      }
      const e = easeIn(x);
      return {
        phase: "out",
        opacity: 1 - e,
        scale: animated ? 1 - (1 - SWAP_SCALE) * e : 1,
        yaw: animated ? SWAP_YAW * e : 0,
        switchNow: false,
      };
    }
    if (this.phase === "in") {
      const x = Math.min((now - this.start) / timing.in, 1);
      if (x >= 1) this.phase = "idle";
      const e = easeOut(x);
      return {
        phase: x >= 1 ? "idle" : "in",
        opacity: e,
        scale: animated ? SWAP_SCALE + (1 - SWAP_SCALE) * e : 1,
        yaw: animated && x < 1 ? -SWAP_YAW * (1 - e) : 0,
        switchNow: false,
      };
    }
    return { phase: "idle", opacity: 1, scale: 1, yaw: 0, switchNow: false };
  }
}
