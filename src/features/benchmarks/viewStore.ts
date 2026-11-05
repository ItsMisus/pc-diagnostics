import { create } from "zustand";
import type { BenchComponent } from "./model";

interface BenchViewState {
  /** Open component screen, or null for the card grid. Kept across page switches. */
  component: BenchComponent | null;
  open: (component: BenchComponent | null) => void;
}

export const useBenchView = create<BenchViewState>((set) => ({
  component: null,
  open: (component) => set({ component }),
}));
