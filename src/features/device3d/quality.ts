/**
 * Adaptive resolution. The stage renders on demand, so frame times are only meaningful while
 * it renders continuously (dragging, settling, swapping). If those frames are slow, the
 * device-pixel ratio steps down; it never steps back up during a session, so the canvas does
 * not oscillate between sizes.
 */

export const DPR_RANGE: readonly [number, number] = [1, 1.75];

/** Starting DPR: the display's, clamped to the range. */
export function initialDpr(deviceRatio: number, range: readonly [number, number] = DPR_RANGE): number {
  const ratio = Number.isFinite(deviceRatio) && deviceRatio > 0 ? deviceRatio : 1;
  return Math.min(Math.max(ratio, range[0]), range[1]);
}

/** A frame slower than this (ms) counts as slow (a bit under 45 fps). */
export const SLOW_FRAME_MS = 22;
const WINDOW = 36;
const STEP = 0.25;

export class QualityMonitor {
  private samples: number[] = [];
  dpr: number;
  private readonly min: number;

  constructor(dpr: number, min = DPR_RANGE[0]) {
    this.dpr = dpr;
    this.min = min;
  }

  /**
   * Record the duration of a continuously rendered frame (ms). Returns the new DPR when it
   * should change, else null.
   */
  push(frameMs: number): number | null {
    // Pauses between on-demand frames are not frame times.
    if (!(frameMs > 0) || frameMs > 250) return null;
    this.samples.push(frameMs);
    if (this.samples.length < WINDOW) return null;
    const sorted = [...this.samples].sort((a, b) => a - b);
    this.samples = [];
    const median = sorted[sorted.length >> 1] ?? 0;
    if (median > SLOW_FRAME_MS && this.dpr > this.min) {
      this.dpr = Math.max(this.min, Math.round((this.dpr - STEP) * 100) / 100);
      return this.dpr;
    }
    return null;
  }

  /** Forget samples (the stream was interrupted). */
  reset(): void {
    this.samples = [];
  }
}
