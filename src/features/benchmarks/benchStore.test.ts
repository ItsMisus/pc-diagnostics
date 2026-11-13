import { describe, expect, it, vi } from "vitest";
import { setLang } from "@/i18n";
import type { BenchmarkProgress } from "@/types/generated/BenchmarkProgress";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";
import { type BenchDeps, createBenchStore } from "./benchStore";
import { capability, progress, result } from "./testing/fixtures";

const flush = () => new Promise((r) => setTimeout(r, 0));

const request = {
  kind: "cpuSingleCore" as const,
  durationS: 10,
  threads: null,
  storageVolume: null,
  storageFileMib: null,
  storageWriteConsent: false,
  gpuId: null,
};

function setup(patch: Partial<BenchDeps> = {}) {
  let emitProgress: (p: BenchmarkProgress) => void = () => {};
  let emitFinished: (r: BenchmarkResult) => void = () => {};
  const deps: BenchDeps = {
    getStatus: async () => null,
    getCapabilities: async () => [capability("cpuSingleCore")],
    startBenchmark: async () => "run-1",
    cancelBenchmark: vi.fn(async () => {}),
    getLatestResult: async () => null,
    onProgress: async (h) => {
      emitProgress = h;
      return () => {
        emitProgress = () => {};
      };
    },
    onFinished: async (h) => {
      emitFinished = h;
      return () => {
        emitFinished = () => {};
      };
    },
    now: () => 0,
    watchdogMs: 0,
    ...patch,
  };
  const store = createBenchStore(deps);
  return {
    store,
    deps,
    progress: (p: BenchmarkProgress) => emitProgress(p),
    finished: (r: BenchmarkResult) => emitFinished(r),
  };
}

describe("bench store", () => {
  it("seeds the latest stored result of every kind once the tests are known", async () => {
    const stored = result("stored-1", { kind: "cpuSingleCore" });
    const t = setup({ getLatestResult: async (k) => (k === "cpuSingleCore" ? stored : null) });
    t.store.getState().ensureCapabilities();
    await flush();
    await flush();
    expect(t.store.getState().lastResultByKind.cpuSingleCore?.runId).toBe("stored-1");
  });

  it("seeds the active run from the backend status", async () => {
    const t = setup({ getStatus: async () => progress("seeded", { fraction: 0.3 }) });
    t.store.getState().start();
    await flush();
    expect(t.store.getState().activeRun?.runId).toBe("seeded");
    expect(t.store.getState().activeRun?.fraction).toBe(0.3);
  });

  it("ignores progress for other run ids and for finished runs", async () => {
    const t = setup();
    t.store.getState().start();
    await flush();
    await t.store.getState().startRun(request);
    expect(t.store.getState().activeRun).toMatchObject({ runId: "run-1", state: "preparing", fraction: 0 });

    t.progress(progress("old-run", { fraction: 0.9 }));
    expect(t.store.getState().activeRun?.runId).toBe("run-1");

    t.progress(progress("run-1", { fraction: 0.4 }));
    expect(t.store.getState().activeRun?.fraction).toBe(0.4);

    t.finished(result("run-1"));
    expect(t.store.getState().activeRun).toBeNull();
    t.progress(progress("run-1", { fraction: 1 })); // late event after finish
    expect(t.store.getState().activeRun).toBeNull();
  });

  it("finished clears the active run and records the result per kind", async () => {
    const t = setup();
    t.store.getState().start();
    await flush();
    await t.store.getState().startRun(request);
    await t.store.getState().cancelRun();
    expect(t.deps.cancelBenchmark).toHaveBeenCalledWith("run-1");
    expect(t.store.getState().cancelling).toBe(true);

    t.finished(result("run-1", { state: "cancelled", metrics: [] }));
    const s = t.store.getState();
    expect(s.activeRun).toBeNull();
    expect(s.cancelling).toBe(false);
    expect(s.lastResultByKind.cpuSingleCore?.state).toBe("cancelled");
    expect(s.lastFinished?.runId).toBe("run-1");
  });

  it("a finished event for an unknown run is recorded without clearing the active one", async () => {
    const t = setup({ getStatus: async () => progress("a") });
    t.store.getState().start();
    await flush();
    t.finished(result("other", { kind: "memoryLatency" }));
    expect(t.store.getState().activeRun?.runId).toBe("a");
    expect(t.store.getState().lastResultByKind.memoryLatency?.runId).toBe("other");
  });

  it("reports a busy start error and keeps no active run", async () => {
    const t = setup({
      startBenchmark: () => Promise.reject({ code: "busy", message: "busy", detail: null, retriable: true }),
    });
    t.store.getState().start();
    await flush();
    const id = await t.store.getState().startRun(request);
    expect(id).toBeNull();
    const s = t.store.getState();
    expect(s.starting).toBe(false);
    expect(s.startError?.kind).toBe("cpuSingleCore");
    expect(s.startError?.error.code).toBe("busy");
    expect(s.activeRun).toBeNull();
    t.store.getState().clearStartError();
    expect(t.store.getState().startError).toBeNull();
  });

  it("keeps a progress event that arrived before startBenchmark resolved", async () => {
    let resolveStart: (id: string) => void = () => {};
    const t = setup({ startBenchmark: () => new Promise((r) => (resolveStart = r)) });
    t.store.getState().start();
    await flush();
    const pending = t.store.getState().startRun(request);
    t.progress(progress("run-9", { fraction: 0.1 }));
    resolveStart("run-9");
    await pending;
    expect(t.store.getState().activeRun?.fraction).toBe(0.1);
  });

  it("shows a starting placeholder in the current language until the first progress event", async () => {
    const t = setup({ startBenchmark: async () => "run-5" });
    t.store.getState().start();
    await flush();
    setLang("en");
    try {
      await t.store.getState().startRun(request);
    } finally {
      setLang("it");
    }
    expect(t.store.getState().activeRun).toMatchObject({ runId: "run-5", state: "preparing", phase: "Starting…" });
  });

  it("resync clears a run whose finished event was missed and recovers the stored result", async () => {
    let status: BenchmarkProgress | null = progress("r");
    const t = setup({
      getStatus: async () => status,
      getLatestResult: async () => result("r"),
    });
    t.store.getState().start();
    await flush();
    expect(t.store.getState().activeRun?.runId).toBe("r");
    status = null;
    await t.store.getState().resync();
    expect(t.store.getState().activeRun).toBeNull();
    expect(t.store.getState().lastResultByKind.cpuSingleCore?.runId).toBe("r");
  });

  it("records cancel errors and allows retrying", async () => {
    const t = setup({
      getStatus: async () => progress("x"),
      cancelBenchmark: () => Promise.reject({ code: "notFound", message: "gone", detail: null, retriable: false }),
    });
    t.store.getState().start();
    await flush();
    await t.store.getState().cancelRun();
    expect(t.store.getState().cancelling).toBe(false);
    expect(t.store.getState().cancelError?.code).toBe("notFound");
  });

  it("stores the subscription failure and stops listening on cleanup", async () => {
    const t = setup({
      onProgress: () => Promise.reject({ code: "notSupported", message: "no tauri", detail: null, retriable: false }),
    });
    const stop = t.store.getState().start();
    await flush();
    expect(t.store.getState().connectError?.code).toBe("notSupported");
    stop();
  });

  it("loads capabilities once", async () => {
    const getCapabilities = vi.fn(async () => [capability("cpuSingleCore")]);
    const t = setup({ getCapabilities });
    t.store.getState().ensureCapabilities();
    t.store.getState().ensureCapabilities();
    await flush();
    expect(getCapabilities).toHaveBeenCalledTimes(1);
    expect(t.store.getState().capabilities.status).toBe("ready");
  });
});
