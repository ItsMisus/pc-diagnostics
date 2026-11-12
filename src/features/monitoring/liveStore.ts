import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";
import { stateKindForError } from "@/lib/errors";
import { commands } from "@/services/commands";
import { onLiveSample } from "@/services/events";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { LiveSample } from "@/types/generated/LiveSample";
import { appendSample, LIVE_CAPACITY, mergeSamples } from "./liveBuffer";

/**
 * `idle`: not started. `connecting`: listener attached, history loading. `live`: seeded
 * and receiving events. The rest mirror `StateView` kinds.
 */
export type LiveStatus = "idle" | "connecting" | "live" | "unsupported" | "permission" | "error";

export interface LiveState {
  status: LiveStatus;
  error: AppError | null;
  /** Oldest first, sorted by `seq`, at most `capacity` entries. Replaced, never mutated. */
  samples: readonly LiveSample[];
  latest: LiveSample | null;
  /**
   * True while `live` but no sample has arrived for `stallMs`: the connection is lost (the
   * backend samples every 0.5–5 s and never pauses). The last values are then stale and must
   * not look live. Cleared by the next sample.
   */
  stalled: boolean;
  /** Starts the single app-level subscription. Returns the cleanup (unlisten). */
  start: () => () => void;
  /** Re-runs the connection after a failure. */
  retry: () => void;
}

export interface LiveDeps {
  getHistory: () => Promise<LiveSample[]>;
  subscribe: (handler: (sample: LiveSample) => void) => Promise<() => void>;
  capacity: number;
  /** No sample for this long while live = connection lost. Default 15 s (3× the slowest interval). */
  stallMs?: number;
}

export const LIVE_STALL_MS = 15_000;

export function createLiveStore(deps: LiveDeps): StoreApi<LiveState> {
  const { capacity } = deps;
  return createStore<LiveState>()((set, get) => {
    // Every connect/stop bumps the generation, so late async results of an older
    // connection (StrictMode double mount, retry) are discarded.
    let generation = 0;
    let unlisten: (() => void) | null = null;
    let watchdog: ReturnType<typeof setTimeout> | null = null;
    const stallMs = deps.stallMs ?? LIVE_STALL_MS;

    const clearWatchdog = () => {
      if (watchdog !== null) clearTimeout(watchdog);
      watchdog = null;
    };
    /** (Re)arms the stall timer for the current connection. */
    const armWatchdog = (gen: number) => {
      clearWatchdog();
      watchdog = setTimeout(() => {
        if (gen === generation && get().status === "live") set({ stalled: true });
      }, stallMs);
    };

    const stop = () => {
      clearWatchdog();
      generation += 1;
      const un = unlisten;
      unlisten = null;
      un?.();
    };

    const connect = async () => {
      stop();
      const gen = generation;
      set({ status: "connecting", error: null, stalled: false });

      // Events that arrive while the history is loading are kept and merged afterwards.
      let pending: LiveSample[] | null = [];
      const onSample = (sample: LiveSample) => {
        if (gen !== generation) return;
        if (pending) {
          pending.push(sample);
          if (pending.length > capacity) pending.shift();
          return;
        }
        armWatchdog(gen);
        const next = appendSample(get().samples, sample, capacity);
        if (next) set({ samples: next, latest: next[next.length - 1] ?? null, stalled: false });
        else if (get().stalled) set({ stalled: false });
      };

      try {
        const un = await deps.subscribe(onSample);
        if (gen !== generation) {
          un();
          return;
        }
        unlisten = un;
        const history = await deps.getHistory();
        if (gen !== generation) return;
        const merged = mergeSamples([get().samples, history, pending], capacity);
        pending = null;
        set({ status: "live", samples: merged, latest: merged[merged.length - 1] ?? null });
        armWatchdog(gen);
      } catch (raw) {
        if (gen !== generation) return;
        stop();
        const error = toAppError(raw);
        const kind = stateKindForError(error);
        set({ status: kind === "consent" ? "error" : kind, error });
      }
    };

    return {
      status: "idle",
      error: null,
      samples: [],
      latest: null,
      stalled: false,
      start: () => {
        void connect();
        return () => {
          stop();
          set({ status: "idle", stalled: false });
        };
      },
      retry: () => {
        void connect();
      },
    };
  });
}

export const liveStore = createLiveStore({
  getHistory: commands.getLiveHistory,
  subscribe: onLiveSample,
  capacity: LIVE_CAPACITY,
});

/** Narrow subscription: components re-render only when the selected slice changes. */
export function useLive<T>(selector: (state: LiveState) => T): T {
  return useStore(liveStore, selector);
}
