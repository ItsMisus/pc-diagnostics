import { afterEach, describe, expect, it, vi } from "vitest";
import type { ProcessSnapshot } from "@/types/generated/ProcessSnapshot";
import { createProcessStore, type ProcessDeps } from "./processStore";
import { snapshot } from "./testing/fixtures";

const notFound = { code: "notFound", message: "no snapshot yet", detail: null, retriable: true };

interface Harness {
  deps: ProcessDeps;
  emit: (s: ProcessSnapshot) => void;
  unlisten: ReturnType<typeof vi.fn>;
  getSnapshot: ReturnType<typeof vi.fn>;
}

function harness(results: (() => Promise<ProcessSnapshot>)[]): Harness {
  let handler: ((s: ProcessSnapshot) => void) | null = null;
  const unlisten = vi.fn(() => {
    handler = null;
  });
  let call = 0;
  const getSnapshot = vi.fn(() => {
    const next = results[Math.min(call, results.length - 1)];
    call += 1;
    return next ? next() : Promise.reject(notFound);
  });
  return {
    deps: {
      waitRetryMs: 2000,
      subscribe: async (h) => {
        handler = h;
        return unlisten;
      },
      getSnapshot,
    },
    emit: (s) => handler?.(s),
    unlisten,
    getSnapshot,
  };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

afterEach(() => {
  vi.useRealTimers();
});

describe("process store", () => {
  it("seeds from the latest snapshot", async () => {
    const h = harness([async () => snapshot(100)]);
    const store = createProcessStore(h.deps);
    store.getState().start();
    expect(store.getState().status).toBe("connecting");
    await flush();
    expect(store.getState().status).toBe("live");
    expect(store.getState().snapshot?.at).toBe(100);
  });

  it("treats notFound as waiting and resolves on the first event", async () => {
    const h = harness([() => Promise.reject(notFound)]);
    const store = createProcessStore(h.deps);
    store.getState().start();
    await flush();
    expect(store.getState().status).toBe("waiting");
    expect(store.getState().error).toBeNull();

    h.emit(snapshot(200));
    expect(store.getState().status).toBe("live");
    expect(store.getState().snapshot?.at).toBe(200);
  });

  it("re-requests the seed while waiting", async () => {
    vi.useFakeTimers();
    const h = harness([() => Promise.reject(notFound), async () => snapshot(300)]);
    const store = createProcessStore(h.deps);
    store.getState().start();
    await vi.advanceTimersByTimeAsync(0);
    expect(store.getState().status).toBe("waiting");
    await vi.advanceTimersByTimeAsync(2000);
    expect(h.getSnapshot).toHaveBeenCalledTimes(2);
    expect(store.getState().status).toBe("live");
  });

  it("ignores snapshots that are not newer than the last one", async () => {
    const h = harness([async () => snapshot(100)]);
    const store = createProcessStore(h.deps);
    store.getState().start();
    await flush();
    const first = store.getState().snapshot;

    h.emit(snapshot(100));
    expect(store.getState().snapshot).toBe(first); // same reference: no re-render
    h.emit(snapshot(50));
    expect(store.getState().snapshot).toBe(first);
    h.emit(snapshot(102));
    expect(store.getState().snapshot?.at).toBe(102);
  });

  it("keeps an event that arrived before a stale seed", async () => {
    let resolveSeed: (s: ProcessSnapshot) => void = () => {};
    const h = harness([() => new Promise((r) => (resolveSeed = r))]);
    const store = createProcessStore(h.deps);
    store.getState().start();
    await flush();
    h.emit(snapshot(500));
    resolveSeed(snapshot(400));
    await flush();
    expect(store.getState().snapshot?.at).toBe(500);
    expect(store.getState().status).toBe("live");
  });

  it("maps errors onto state kinds and retries", async () => {
    const h = harness([
      () => Promise.reject({ code: "notSupported", message: "outside tauri", detail: null, retriable: false }),
      () => Promise.reject({ code: "permissionDenied", message: "no", detail: null, retriable: false }),
      () => Promise.reject({ code: "platform", message: "x", detail: null, retriable: true }),
      async () => snapshot(1),
    ]);
    const store = createProcessStore(h.deps);
    store.getState().start();
    await flush();
    expect(store.getState().status).toBe("unsupported");
    expect(h.unlisten).toHaveBeenCalledTimes(1);

    store.getState().retry();
    await flush();
    expect(store.getState().status).toBe("permission");

    store.getState().retry();
    await flush();
    expect(store.getState().status).toBe("error");
    expect(store.getState().error?.code).toBe("platform");

    store.getState().retry();
    await flush();
    expect(store.getState().status).toBe("live");
  });

  it("cleanup detaches the listener and ignores late results", async () => {
    let resolveSeed: (s: ProcessSnapshot) => void = () => {};
    const h = harness([() => new Promise((r) => (resolveSeed = r))]);
    const store = createProcessStore(h.deps);
    const stop = store.getState().start();
    await flush();
    stop();
    expect(h.unlisten).toHaveBeenCalledTimes(1);
    resolveSeed(snapshot(1));
    h.emit(snapshot(2));
    await flush();
    expect(store.getState().snapshot).toBeNull();
    expect(store.getState().status).toBe("idle");
  });

  it("stops the waiting retry on cleanup", async () => {
    vi.useFakeTimers();
    const h = harness([() => Promise.reject(notFound)]);
    const store = createProcessStore(h.deps);
    const stop = store.getState().start();
    await vi.advanceTimersByTimeAsync(0);
    stop();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(h.getSnapshot).toHaveBeenCalledTimes(1);
  });
});
