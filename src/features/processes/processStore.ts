import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";
import { stateKindForError } from "@/lib/errors";
import { commands } from "@/services/commands";
import { onProcessSnapshot } from "@/services/events";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { ProcessSnapshot } from "@/types/generated/ProcessSnapshot";

/**
 * `idle`: page not mounted. `connecting`: listener attached, seed request pending.
 * `waiting`: the backend has no snapshot yet (first ~4 s after start); resolves by itself
 * on the first event. `live`: receiving snapshots. The rest mirror `StateView` kinds.
 */
export type ProcessStatus = "idle" | "connecting" | "waiting" | "live" | "unsupported" | "permission" | "consent" | "error";

export interface ProcessState {
  status: ProcessStatus;
  error: AppError | null;
  /** Latest accepted snapshot. Kept across unmounts; replaced by the next newer one. */
  snapshot: ProcessSnapshot | null;
  /** Attaches the page-scoped subscription and seeds it. Returns the cleanup. */
  start: () => () => void;
  retry: () => void;
}

export interface ProcessDeps {
  getSnapshot: () => Promise<ProcessSnapshot>;
  subscribe: (handler: (snapshot: ProcessSnapshot) => void) => Promise<() => void>;
  /** While waiting, the seed is re-requested at this interval in case an event is missed. */
  waitRetryMs: number;
}

export function createProcessStore(deps: ProcessDeps): StoreApi<ProcessState> {
  return createStore<ProcessState>()((set, get) => {
    // Bumped on every connect/stop so late results of an older connection are dropped.
    let generation = 0;
    let unlisten: (() => void) | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const stop = () => {
      generation += 1;
      if (timer !== undefined) clearTimeout(timer);
      timer = undefined;
      const un = unlisten;
      unlisten = null;
      un?.();
    };

    /** Accepts only snapshots newer than the current one (events and seed can race). */
    const accept = (snapshot: ProcessSnapshot) => {
      const last = get().snapshot;
      if (last && snapshot.at <= last.at) {
        if (get().status !== "live") set({ status: "live", error: null });
        return;
      }
      set({ snapshot, status: "live", error: null });
    };

    const seed = async (gen: number) => {
      try {
        const snapshot = await deps.getSnapshot();
        if (gen !== generation) return;
        accept(snapshot);
      } catch (raw) {
        if (gen !== generation) return;
        const error = toAppError(raw);
        if (get().status === "live") return; // events are already flowing
        if (error.code === "notFound") {
          set({ status: "waiting", error: null });
          timer = setTimeout(() => {
            timer = undefined;
            if (gen === generation && get().status === "waiting") void seed(gen);
          }, deps.waitRetryMs);
          return;
        }
        stop();
        set({ status: stateKindForError(error), error });
      }
    };

    const connect = async () => {
      stop();
      const gen = generation;
      set({ status: "connecting", error: null });
      const onSnapshot = (snapshot: ProcessSnapshot) => {
        if (gen === generation) accept(snapshot);
      };
      try {
        const un = await deps.subscribe(onSnapshot);
        if (gen !== generation) {
          un();
          return;
        }
        unlisten = un;
      } catch (raw) {
        if (gen !== generation) return;
        stop();
        const error = toAppError(raw);
        set({ status: stateKindForError(error), error });
        return;
      }
      await seed(gen);
    };

    return {
      status: "idle",
      error: null,
      snapshot: null,
      start: () => {
        void connect();
        return () => {
          stop();
          set({ status: "idle" });
        };
      },
      retry: () => {
        void connect();
      },
    };
  });
}

export const processStore = createProcessStore({
  getSnapshot: commands.getProcessSnapshot,
  subscribe: onProcessSnapshot,
  waitRetryMs: 2000,
});

/** Narrow subscription: components re-render only when the selected slice changes. */
export function useProcesses<T>(selector: (state: ProcessState) => T): T {
  return useStore(processStore, selector);
}
