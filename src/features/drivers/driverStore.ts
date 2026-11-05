import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";
import { commands } from "@/services/commands";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { DriverInventory } from "@/types/generated/DriverInventory";

/** State of the online update search (Windows Update), independent of the inventory. */
export type UpdateCheckState =
  | { status: "idle" }
  | { status: "running"; startedAt: number }
  | { status: "error"; error: AppError };

export interface DriverState {
  status: "idle" | "loading" | "ready" | "error";
  data: DriverInventory | null;
  error: AppError | null;
  /** Re-enumeration in progress; the previous list stays visible meanwhile. */
  refreshing: boolean;
  refreshError: AppError | null;
  check: UpdateCheckState;
  /** Loads the (backend-cached) inventory once. Later calls are no-ops. */
  ensureLoaded: () => void;
  reload: () => void;
  /** "Aggiorna elenco": re-enumerates devices (`refresh: true`, no network). */
  refresh: () => void;
  /** Starts the online search. No-op while one is running. */
  checkUpdates: () => void;
}

export interface DriverDeps {
  getInventory: (refresh: boolean) => Promise<DriverInventory>;
  checkUpdates: () => Promise<DriverInventory>;
  now: () => number;
}

export function createDriverStore(deps: DriverDeps): StoreApi<DriverState> {
  return createStore<DriverState>()((set, get) => {
    const load = async () => {
      set({ status: "loading", error: null });
      try {
        const data = await deps.getInventory(false);
        set({ status: "ready", data, error: null });
      } catch (e) {
        set({ status: "error", error: toAppError(e) });
      }
    };

    return {
      status: "idle",
      data: null,
      error: null,
      refreshing: false,
      refreshError: null,
      check: { status: "idle" },
      ensureLoaded: () => {
        if (get().status === "idle") void load();
      },
      reload: () => {
        if (get().status !== "loading") void load();
      },
      refresh: () => {
        const s = get();
        if (s.refreshing || s.status === "loading" || s.check.status === "running") return;
        set({ refreshing: true, refreshError: null });
        deps.getInventory(true).then(
          (data) => set({ status: "ready", data, error: null, refreshing: false }),
          (e: unknown) => set({ refreshing: false, refreshError: toAppError(e) }),
        );
      },
      checkUpdates: () => {
        const s = get();
        if (s.check.status === "running" || s.refreshing) return;
        set({ check: { status: "running", startedAt: deps.now() } });
        deps.checkUpdates().then(
          (data) => set({ status: "ready", data, error: null, check: { status: "idle" } }),
          (e: unknown) => set({ check: { status: "error", error: toAppError(e) } }),
        );
      },
    };
  });
}

export const driverStore = createDriverStore({
  getInventory: commands.getDriverInventory,
  checkUpdates: commands.checkDriverUpdates,
  now: () => Date.now(),
});

export function useDrivers<T>(selector: (state: DriverState) => T): T {
  return useStore(driverStore, selector);
}
