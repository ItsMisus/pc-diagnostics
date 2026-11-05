import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";
import { commands } from "@/services/commands";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { HardwareInventory } from "@/types/generated/HardwareInventory";

export interface InventoryState {
  status: "idle" | "loading" | "ready" | "error";
  data: HardwareInventory | null;
  error: AppError | null;
  /** A forced re-collection is running; the previous data stays visible meanwhile. */
  refreshing: boolean;
  refreshError: AppError | null;
  /** Loads the (backend-cached) inventory once. Later calls are no-ops. */
  ensureLoaded: () => void;
  /** Retries the initial load after an error. */
  reload: () => void;
  /** Forces a new, slow collection (`refresh: true`). */
  refresh: () => void;
}

export function createInventoryStore(
  fetch: (refresh: boolean) => Promise<HardwareInventory>,
): StoreApi<InventoryState> {
  return createStore<InventoryState>()((set, get) => {
    const load = async () => {
      set({ status: "loading", error: null });
      try {
        const data = await fetch(false);
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
      ensureLoaded: () => {
        if (get().status === "idle") void load();
      },
      reload: () => {
        if (get().status !== "loading") void load();
      },
      refresh: () => {
        if (get().refreshing || get().status === "loading") return;
        set({ refreshing: true, refreshError: null });
        fetch(true).then(
          (data) => set({ status: "ready", data, error: null, refreshing: false }),
          (e: unknown) => set({ refreshing: false, refreshError: toAppError(e) }),
        );
      },
    };
  });
}

export const inventoryStore = createInventoryStore(commands.getHardwareInventory);

export function useInventory<T>(selector: (state: InventoryState) => T): T {
  return useStore(inventoryStore, selector);
}
