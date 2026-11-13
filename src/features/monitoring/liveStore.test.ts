import { describe, expect, it, vi } from "vitest";
import type { LiveSample } from "@/types/generated/LiveSample";
import { createLiveStore, type LiveDeps } from "./liveStore";
import { sample } from "./testing/fixtures";

interface Harness {
  deps: LiveDeps;
  emit: (s: LiveSample) => void;
  resolveHistory: (h: LiveSample[]) => void;
  rejectHistory: (e: unknown) => void;
  unlisten: ReturnType<typeof vi.fn>;
}

function harness(capacity = 5): Harness {
  let handler: ((s: LiveSample) => void) | null = null;
  let resolveHistory: (h: LiveSample[]) => void = () => {};
  let rejectHistory: (e: unknown) => void = () => {};
  const unlisten = vi.fn(() => {
    handler = null;
  });
  return {
    deps: {
      capacity,
      subscribe: async (h) => {
        handler = h;
        return unlisten;
      },
      getHistory: () =>
        new Promise<LiveSample[]>((res, rej) => {
          resolveHistory = res;
          rejectHistory = rej;
        }),
    },
    emit: (s) => handler?.(s),
    resolveHistory: (h) => resolveHistory(h),
    rejectHistory: (e) => rejectHistory(e),
    unlisten,
  };
}

const flush = () => new Promise((r) => setTimeout(r, 0));
const seqs = (xs: readonly LiveSample[]) => xs.map((x) => x.seq);

describe("live store", () => {
  it("seeds from history and keeps events received while loading", async () => {
    const h = harness();
    const store = createLiveStore(h.deps);
    store.getState().start();
    await flush();
    expect(store.getState().status).toBe("connecting");

    h.emit(sample(4)); // arrives before the history resolves
    h.resolveHistory([sample(1), sample(2), sample(3), sample(4)]);
    await flush();

    const s = store.getState();
    expect(s.status).toBe("live");
    expect(seqs(s.samples)).toEqual([1, 2, 3, 4]);
    expect(s.latest?.seq).toBe(4);
  });

  it("appends events, drops duplicates and out-of-order, stays bounded", async () => {
    const h = harness(3);
    const store = createLiveStore(h.deps);
    store.getState().start();
    await flush();
    h.resolveHistory([sample(1)]);
    await flush();

    const before = store.getState().samples;
    h.emit(sample(1));
    expect(store.getState().samples).toBe(before); // unchanged reference: no re-render

    for (const n of [2, 3, 2, 4, 5]) h.emit(sample(n));
    expect(seqs(store.getState().samples)).toEqual([3, 4, 5]);
    expect(store.getState().latest?.seq).toBe(5);
  });

  it("flags a stalled stream as lost and clears it with the next sample", async () => {
    const h = harness();
    const store = createLiveStore({ ...h.deps, stallMs: 40 });
    const stopLive = store.getState().start();
    await flush();
    h.resolveHistory([sample(1)]);
    await flush();
    expect(store.getState().stalled).toBe(false);

    await new Promise((r) => setTimeout(r, 60));
    expect(store.getState()).toMatchObject({ status: "live", stalled: true });

    h.emit(sample(2));
    expect(store.getState().stalled).toBe(false);

    stopLive();
    await new Promise((r) => setTimeout(r, 60));
    expect(store.getState()).toMatchObject({ status: "idle", stalled: false });
  });

  it("maps notSupported to the unsupported state and shows no data", async () => {
    const h = harness();
    const store = createLiveStore(h.deps);
    store.getState().start();
    await flush();
    h.rejectHistory({ code: "notSupported", message: "outside tauri", detail: null, retriable: false });
    await flush();

    const s = store.getState();
    expect(s.status).toBe("unsupported");
    expect(s.samples).toEqual([]);
    expect(s.error?.code).toBe("notSupported");
    expect(h.unlisten).toHaveBeenCalled();
  });

  it("maps permissionDenied and other errors; retry reconnects", async () => {
    const h = harness();
    const store = createLiveStore(h.deps);
    store.getState().start();
    await flush();
    h.rejectHistory({ code: "platform", message: "pdh", detail: null, retriable: true });
    await flush();
    expect(store.getState().status).toBe("error");

    store.getState().retry();
    await flush();
    h.rejectHistory({ code: "permissionDenied", message: "no", detail: null, retriable: false });
    await flush();
    expect(store.getState().status).toBe("permission");

    store.getState().retry();
    await flush();
    h.resolveHistory([sample(7)]);
    await flush();
    expect(store.getState().status).toBe("live");
  });

  it("cleanup detaches the listener and ignores late results", async () => {
    const h = harness();
    const store = createLiveStore(h.deps);
    const stop = store.getState().start();
    await flush();
    stop();
    expect(h.unlisten).toHaveBeenCalledTimes(1);

    h.resolveHistory([sample(1)]);
    h.emit(sample(2));
    await flush();
    expect(store.getState().samples).toEqual([]);
    expect(store.getState().status).toBe("idle");
  });

  it("unlistens a subscription that resolves after cleanup", async () => {
    const unlisten = vi.fn();
    let resolveSub: (un: () => void) => void = () => {};
    const store = createLiveStore({
      capacity: 5,
      subscribe: () => new Promise((r) => (resolveSub = r)),
      getHistory: async () => [],
    });
    const stop = store.getState().start();
    stop();
    resolveSub(unlisten);
    await flush();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
