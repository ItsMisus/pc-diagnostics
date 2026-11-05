import { describe, expect, it, vi } from "vitest";
import type { DriverInventory } from "@/types/generated/DriverInventory";
import { createDriverStore } from "./driverStore";
import { device, inventory } from "./testing/fixtures";

const flush = () => new Promise((r) => setTimeout(r, 0));

function deferred<T>() {
  let resolve: (v: T) => void = () => {};
  let reject: (e: unknown) => void = () => {};
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("driver store", () => {
  it("loads once and re-enumerates on refresh keeping the old list visible", async () => {
    const getInventory = vi.fn(async (refresh: boolean) => inventory([device(refresh ? "new" : "old")]));
    const store = createDriverStore({ getInventory, checkUpdates: vi.fn(), now: () => 0 });
    store.getState().ensureLoaded();
    store.getState().ensureLoaded();
    expect(store.getState().status).toBe("loading");
    await flush();
    expect(getInventory).toHaveBeenCalledTimes(1);
    expect(store.getState().data?.devices[0]?.name).toBe("old");

    store.getState().refresh();
    expect(store.getState().refreshing).toBe(true);
    expect(store.getState().data?.devices[0]?.name).toBe("old");
    await flush();
    expect(getInventory).toHaveBeenLastCalledWith(true);
    expect(store.getState().data?.devices[0]?.name).toBe("new");
    expect(store.getState().refreshing).toBe(false);
  });

  it("runs one update check at a time and replaces the inventory on success", async () => {
    const d = deferred<DriverInventory>();
    const checkUpdates = vi.fn(() => d.promise);
    const store = createDriverStore({ getInventory: async () => inventory(), checkUpdates, now: () => 42 });
    store.getState().ensureLoaded();
    await flush();

    store.getState().checkUpdates();
    store.getState().checkUpdates();
    expect(checkUpdates).toHaveBeenCalledTimes(1);
    expect(store.getState().check).toEqual({ status: "running", startedAt: 42 });

    store.getState().refresh(); // blocked while checking
    expect(store.getState().refreshing).toBe(false);

    d.resolve(inventory([device("gpu")], 99));
    await flush();
    expect(store.getState().check.status).toBe("idle");
    expect(store.getState().data?.collectedAt).toBe(99);
  });

  it("keeps the inventory when the check fails (consentRequired)", async () => {
    const store = createDriverStore({
      getInventory: async () => inventory([device("a")]),
      checkUpdates: () => Promise.reject({ code: "consentRequired", message: "off", detail: null, retriable: false }),
      now: () => 0,
    });
    store.getState().ensureLoaded();
    await flush();
    store.getState().checkUpdates();
    await flush();
    const s = store.getState();
    expect(s.check.status === "error" && s.check.error.code).toBe("consentRequired");
    expect(s.data?.devices).toHaveLength(1);
  });

  it("reports load errors and reloads", async () => {
    let fail = true;
    const store = createDriverStore({
      getInventory: async () => {
        if (fail) throw { code: "notSupported", message: "outside tauri", detail: null, retriable: false };
        return inventory();
      },
      checkUpdates: vi.fn(),
      now: () => 0,
    });
    store.getState().ensureLoaded();
    await flush();
    expect(store.getState().status).toBe("error");
    expect(store.getState().error?.code).toBe("notSupported");
    fail = false;
    store.getState().reload();
    await flush();
    expect(store.getState().status).toBe("ready");
  });
});
