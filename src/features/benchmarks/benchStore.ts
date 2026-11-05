import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";
import { translatorFor } from "@/i18n";
import { commands } from "@/services/commands";
import { onBenchmarkFinished, onBenchmarkProgress } from "@/services/events";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { BenchmarkCapability } from "@/types/generated/BenchmarkCapability";
import type { BenchmarkKind } from "@/types/generated/BenchmarkKind";
import type { BenchmarkProgress } from "@/types/generated/BenchmarkProgress";
import type { BenchmarkRequest } from "@/types/generated/BenchmarkRequest";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";

export type CapabilitiesState =
  | { status: "idle" | "loading" }
  | { status: "ready"; data: BenchmarkCapability[] }
  | { status: "error"; error: AppError };

export interface BenchState {
  /** Progress of the run in flight, or null when idle. Updated ~4×/s: subscribe narrowly. */
  activeRun: BenchmarkProgress | null;
  /** A start request is in flight. */
  starting: boolean;
  /** Last start failure (busy, invalidInput, …) and the kind it refers to. */
  startError: { kind: BenchmarkKind; error: AppError } | null;
  cancelling: boolean;
  cancelError: AppError | null;
  /** Final results received in this session, per kind. */
  lastResultByKind: Partial<Record<BenchmarkKind, BenchmarkResult>>;
  /** Most recent final result of any kind (History refreshes on it). */
  lastFinished: BenchmarkResult | null;
  /** Event subscription failure (e.g. outside the desktop app). */
  connectError: AppError | null;
  capabilities: CapabilitiesState;

  /** Starts the single app-level subscription. Returns the cleanup. */
  start: () => () => void;
  startRun: (request: BenchmarkRequest) => Promise<string | null>;
  cancelRun: () => Promise<void>;
  /** Re-reads the backend status; clears a run whose finished event was missed. */
  resync: () => Promise<void>;
  ensureCapabilities: () => void;
  reloadCapabilities: () => void;
  clearStartError: () => void;
}

export interface BenchDeps {
  getStatus: () => Promise<BenchmarkProgress | null>;
  getCapabilities: () => Promise<BenchmarkCapability[]>;
  startBenchmark: (request: BenchmarkRequest) => Promise<string>;
  cancelBenchmark: (runId: string) => Promise<void>;
  /** Latest stored result of a kind; used to recover a missed finished event. */
  getLatestResult: (kind: BenchmarkKind) => Promise<BenchmarkResult | null>;
  onProgress: (handler: (p: BenchmarkProgress) => void) => Promise<() => void>;
  onFinished: (handler: (r: BenchmarkResult) => void) => Promise<() => void>;
  now: () => number;
  /** Watchdog period (ms); 0 disables it (tests). */
  watchdogMs: number;
}

/** Without progress for this long, the backend status is re-read. */
export const STALE_PROGRESS_MS = 3000;

/**
 * Placeholder until the first progress event of a run we just started arrives. Its phase is
 * the only one written by the frontend (in the current language); real phases come from Rust.
 */
function startingProgress(runId: string, kind: BenchmarkKind): BenchmarkProgress {
  const phase = translatorFor("benchmarks")("starting");
  return { runId, kind, state: "preparing", fraction: 0, elapsedMs: 0, phase, interim: [] };
}

export function createBenchStore(deps: BenchDeps): StoreApi<BenchState> {
  return createStore<BenchState>()((set, get) => {
    let generation = 0;
    let unlisteners: (() => void)[] = [];
    let watchdog: ReturnType<typeof setInterval> | null = null;
    /** Run ids that already finished: late progress for them is dropped. */
    const finished = new Set<string>();
    let lastProgressAt = 0;

    const acceptProgress = (p: BenchmarkProgress) => {
      if (finished.has(p.runId)) return;
      const active = get().activeRun;
      // Only one run can be active; progress for a different id is stale.
      if (active && active.runId !== p.runId) return;
      lastProgressAt = deps.now();
      set({ activeRun: p });
    };

    const acceptFinished = (r: BenchmarkResult) => {
      if (finished.has(r.runId)) return;
      finished.add(r.runId);
      const active = get().activeRun;
      const clears = active?.runId === r.runId;
      set({
        lastResultByKind: { ...get().lastResultByKind, [r.kind]: r },
        lastFinished: r,
        ...(clears ? { activeRun: null, cancelling: false, cancelError: null } : {}),
      });
    };

    const stop = () => {
      generation += 1;
      for (const un of unlisteners) un();
      unlisteners = [];
      if (watchdog !== null) clearInterval(watchdog);
      watchdog = null;
    };

    const connect = async () => {
      stop();
      const gen = generation;
      set({ connectError: null });
      try {
        const unP = await deps.onProgress((p) => gen === generation && acceptProgress(p));
        const unF = await deps.onFinished((r) => gen === generation && acceptFinished(r));
        if (gen !== generation) {
          unP();
          unF();
          return;
        }
        unlisteners = [unP, unF];
        const status = await deps.getStatus();
        if (gen !== generation) return;
        if (status) acceptProgress(status);
        if (deps.watchdogMs > 0) {
          watchdog = setInterval(() => {
            if (get().activeRun && deps.now() - lastProgressAt > STALE_PROGRESS_MS) void get().resync();
          }, deps.watchdogMs);
        }
      } catch (e) {
        if (gen !== generation) return;
        set({ connectError: toAppError(e) });
      }
    };

    const loadCapabilities = async () => {
      set({ capabilities: { status: "loading" } });
      try {
        const data = await deps.getCapabilities();
        set({ capabilities: { status: "ready", data } });
      } catch (e) {
        set({ capabilities: { status: "error", error: toAppError(e) } });
      }
    };

    return {
      activeRun: null,
      starting: false,
      startError: null,
      cancelling: false,
      cancelError: null,
      lastResultByKind: {},
      lastFinished: null,
      connectError: null,
      capabilities: { status: "idle" },

      start: () => {
        void connect();
        return stop;
      },

      startRun: async (request) => {
        if (get().starting) return null;
        set({ starting: true, startError: null });
        try {
          const runId = await deps.startBenchmark(request);
          lastProgressAt = deps.now();
          const active = get().activeRun;
          set({
            starting: false,
            cancelError: null,
            ...(finished.has(runId)
              ? {}
              : { activeRun: active?.runId === runId ? active : startingProgress(runId, request.kind) }),
          });
          return runId;
        } catch (e) {
          set({ starting: false, startError: { kind: request.kind, error: toAppError(e) } });
          return null;
        }
      },

      cancelRun: async () => {
        const active = get().activeRun;
        if (!active || get().cancelling) return;
        set({ cancelling: true, cancelError: null });
        try {
          await deps.cancelBenchmark(active.runId);
          // `cancelling` stays true until the finished event clears the run.
        } catch (e) {
          if (get().activeRun?.runId === active.runId) set({ cancelling: false, cancelError: toAppError(e) });
        }
      },

      resync: async () => {
        const active = get().activeRun;
        if (!active) return;
        let status: BenchmarkProgress | null;
        try {
          status = await deps.getStatus();
        } catch {
          return;
        }
        lastProgressAt = deps.now();
        if (get().activeRun?.runId !== active.runId) return;
        if (status && status.runId === active.runId) {
          set({ activeRun: status });
          return;
        }
        // The run ended and we missed its finished event: recover the stored result.
        finished.add(active.runId);
        set({ activeRun: null, cancelling: false });
        try {
          const latest = await deps.getLatestResult(active.kind);
          if (latest && latest.runId === active.runId) {
            set({ lastResultByKind: { ...get().lastResultByKind, [latest.kind]: latest }, lastFinished: latest });
          }
        } catch {
          // Nothing to recover; History still lists the run once stored.
        }
        if (status) acceptProgress(status);
      },

      ensureCapabilities: () => {
        if (get().capabilities.status === "idle") void loadCapabilities();
      },
      reloadCapabilities: () => {
        if (get().capabilities.status !== "loading") void loadCapabilities();
      },
      clearStartError: () => set({ startError: null }),
    };
  });
}

export const benchStore = createBenchStore({
  getStatus: commands.getBenchmarkStatus,
  getCapabilities: commands.getBenchmarkCapabilities,
  startBenchmark: commands.startBenchmark,
  cancelBenchmark: commands.cancelBenchmark,
  getLatestResult: async (kind) => (await commands.getBenchmarkHistory(kind, 1))[0] ?? null,
  onProgress: onBenchmarkProgress,
  onFinished: onBenchmarkFinished,
  now: () => Date.now(),
  watchdogMs: 1000,
});

/** Narrow subscription: select primitives or stable references only. */
export function useBench<T>(selector: (state: BenchState) => T): T {
  return useStore(benchStore, selector);
}
