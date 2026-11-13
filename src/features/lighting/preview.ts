import { useEffect, useRef } from "react";
import type { LightingEffect } from "@/types/generated/LightingEffect";

/** The instant a still preview shows (reduced motion, paused): a representative frame. */
export function stillTime(effect: LightingEffect): number {
  return effect.kind === "breathing" ? effect.period * 0.42 : 0;
}

/**
 * Calls `draw(tMs)` every animation frame while `active`, with the time since the effect
 * started; when not active, draws the still frame once (and again whenever `deps` change).
 * The loop stops as soon as `active` turns false or the component unmounts.
 */
export function useAnimationClock(active: boolean, still: number, draw: (tMs: number) => void, deps: readonly unknown[]): void {
  const drawRef = useRef(draw);
  drawRef.current = draw;
  useEffect(() => {
    if (!active) {
      drawRef.current(still);
      return;
    }
    let id = 0;
    const start = performance.now();
    const tick = (now: number) => {
      drawRef.current(now - start);
      id = requestAnimationFrame(tick);
    };
    id = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(id);
  }, [active, still, ...deps]);
}
