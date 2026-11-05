import { useEffect, useMemo } from "react";
import type { Domain, Point } from "@/components/charts/scale";
import type { HardwareInventory } from "@/types/generated/HardwareInventory";
import { useInventory } from "./inventoryStore";
import { useLive } from "./liveStore";
import { type Pick, seriesFrom, timeDomain } from "./series";

/**
 * Derived series for one chart. Recomputed only when the sample buffer is replaced
 * (once per accepted event). `pick` must be referentially stable.
 */
export function useSeries(pick: Pick): Point[] {
  const samples = useLive((s) => s.samples);
  return useMemo(() => seriesFrom(samples, pick), [samples, pick]);
}

export function useTimeDomain(): Domain {
  const samples = useLive((s) => s.samples);
  return useMemo(() => timeDomain(samples), [samples]);
}

/** Inventory slice for a component, triggering the one-time load on first use. */
export function useInventoryData(): {
  status: "idle" | "loading" | "ready" | "error";
  data: HardwareInventory | null;
} {
  const status = useInventory((s) => s.status);
  const data = useInventory((s) => s.data);
  const ensureLoaded = useInventory((s) => s.ensureLoaded);
  useEffect(() => ensureLoaded(), [ensureLoaded]);
  return { status, data };
}
