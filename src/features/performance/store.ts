import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { PerformanceProfileInput } from "@/types/generated/PerformanceProfileInput";
import type { PerformanceState } from "@/types/generated/PerformanceState";
import { performanceApi, type PerformanceApi } from "./api";

export type PerformanceAction = "apply" | "save" | "delete" | "restore" | "openSettings";

export interface PerformanceStore {
  status: "idle" | "loading" | "ready" | "error";
  data: PerformanceState | null;
  /** First-load failure (the page shows it instead of the content). */
  loadError: AppError | null;
  /** The action in flight and, for apply/delete, the profile it targets. */
  busy: { action: PerformanceAction; profileId?: string } | null;
  /** Last action failure, shown inline next to what failed. */
  actionError: { action: PerformanceAction; error: AppError } | null;

  load: () => Promise<void>;
  /** Re-reads quietly (supply or System Settings changes); keeps the last data on failure. */
  refresh: () => Promise<void>;
  apply: (profileId: string) => Promise<boolean>;
  save: (profile: PerformanceProfileInput) => Promise<PerformanceState | null>;
  remove: (profileId: string) => Promise<boolean>;
  restore: () => Promise<boolean>;
  openSettings: () => Promise<void>;
  clearActionError: () => void;
}

export function createPerformanceStore(api: PerformanceApi = performanceApi): StoreApi<PerformanceStore> {
  return createStore<PerformanceStore>((set, get) => {
    /** Runs one action; the returned state replaces the page's data. */
    async function act(action: PerformanceAction, run: () => Promise<PerformanceState>, profileId?: string): Promise<PerformanceState | null> {
      if (get().busy) return null;
      set({ busy: profileId === undefined ? { action } : { action, profileId }, actionError: null });
      try {
        const data = await run();
        set({ data, status: "ready", busy: null });
        return data;
      } catch (e) {
        set({ busy: null, actionError: { action, error: toAppError(e) } });
        return null;
      }
    }

    return {
      status: "idle",
      data: null,
      loadError: null,
      busy: null,
      actionError: null,

      load: async () => {
        set({ status: get().data ? "ready" : "loading", loadError: null });
        try {
          set({ data: await api.getState(), status: "ready" });
        } catch (e) {
          set({ status: "error", loadError: toAppError(e) });
        }
      },
      refresh: async () => {
        if (get().busy || get().status !== "ready") return;
        try {
          const data = await api.getState();
          if (!get().busy) set({ data });
        } catch {
          // Keep the last state; the next action or load reports the error.
        }
      },
      apply: async (profileId) => (await act("apply", () => api.applyProfile(profileId), profileId)) !== null,
      save: (profile) => act("save", () => api.saveProfile(profile)),
      remove: async (profileId) => (await act("delete", () => api.deleteProfile(profileId), profileId)) !== null,
      restore: async () => (await act("restore", () => api.restoreOriginal())) !== null,
      openSettings: async () => {
        set({ actionError: null });
        try {
          await api.openPowerSettings();
        } catch (e) {
          set({ actionError: { action: "openSettings", error: toAppError(e) } });
        }
      },
      clearActionError: () => set({ actionError: null }),
    };
  });
}

export const performanceStore = createPerformanceStore();

export function usePerformance<T>(selector: (s: PerformanceStore) => T): T {
  return useStore(performanceStore, selector);
}
