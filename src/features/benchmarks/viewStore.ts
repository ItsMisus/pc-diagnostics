import { create } from "zustand";
import type { BenchComponent } from "./model";

interface BenchViewState {
  /** Open component screen, or null for the card grid. Kept across page switches. */
  component: BenchComponent | null;
  open: (component: BenchComponent | null) => void;
  /**
   * The run block (phase, progress, Stop) of the open component screen is actually visible in
   * the scroll area. Only then does the floating strip step aside; when the block is scrolled
   * out of view (or not mounted), the strip comes back so Stop is always reachable.
   */
  runInView: boolean;
  setRunInView: (inView: boolean) => void;
}

export const useBenchView = create<BenchViewState>((set) => ({
  component: null,
  open: (component) => set({ component }),
  runInView: false,
  setRunInView: (runInView) => set({ runInView }),
}));
