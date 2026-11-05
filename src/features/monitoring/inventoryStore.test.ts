import { describe, expect, it, vi } from "vitest";
import type { HardwareInventory } from "@/types/generated/HardwareInventory";
import { createInventoryStore } from "./inventoryStore";

const flush = () => new Promise((r) => setTimeout(r, 0));

const inventory = (collectedAt: number) => ({ collectedAt }) as unknown as HardwareInventory;

describe("inventory store", () => {
  it("loads once; later ensureLoaded calls are no-ops", async () => {
    const fetch = vi.fn(async (refresh: boolean) => inventory(refresh ? 2 : 1));
    const store = createInventoryStore(fetch);
    store.getState().ensureLoaded();
    store.getState().ensureLoaded();
    await flush();
    store.getState().ensureLoaded();
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch).toHaveBeenCalledWith(false);
    expect(store.getState().status).toBe("ready");
  });

  it("refresh forces a new collection and keeps old data on failure", async () => {
    let fail = false;
    const fetch = vi.fn(async (refresh: boolean) => {
      if (fail) throw { code: "platform", message: "wmi", detail: null, retriable: true };
      return inventory(refresh ? 2 : 1);
    });
    const store = createInventoryStore(fetch);
    store.getState().ensureLoaded();
    await flush();

    store.getState().refresh();
    expect(store.getState().refreshing).toBe(true);
    store.getState().refresh(); // ignored while running
    await flush();
    expect(fetch).toHaveBeenLastCalledWith(true);
    expect(fetch).toHaveBeenCalledTimes(2);
    expect(store.getState().data?.collectedAt).toBe(2);

    fail = true;
    store.getState().refresh();
    await flush();
    expect(store.getState().refreshing).toBe(false);
    expect(store.getState().refreshError?.code).toBe("platform");
    expect(store.getState().data?.collectedAt).toBe(2);
  });

  it("exposes load errors and reload retries", async () => {
    let fail = true;
    const store = createInventoryStore(async () => {
      if (fail) throw { code: "notSupported", message: "x", detail: null, retriable: false };
      return inventory(1);
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
