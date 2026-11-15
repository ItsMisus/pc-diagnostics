import { describe, expect, it, vi } from "vitest";
import type { AppError } from "@/types/generated/AppError";
import type { PerformanceState } from "@/types/generated/PerformanceState";
import type { PerformanceApi } from "./api";
import { createPerformanceStore } from "./store";

const state = (selected: string | null): PerformanceState =>
  ({
    capabilities: {},
    os: { ac: {}, battery: null, schemeName: null, keepAwake: "off" },
    supply: "ac",
    hasBattery: false,
    profiles: [],
    selectedProfileId: selected,
    gameOverride: null,
    canRestore: selected !== null,
    issues: [],
  }) as unknown as PerformanceState;

const denied: AppError = { code: "permissionDenied", message: "denied", detail: null, retriable: false };

function api(over: Partial<PerformanceApi> = {}): PerformanceApi {
  return {
    getState: vi.fn(async () => state(null)),
    applyProfile: vi.fn(async (id: string) => state(id)),
    saveProfile: vi.fn(async () => state(null)),
    deleteProfile: vi.fn(async () => state(null)),
    restoreOriginal: vi.fn(async () => state(null)),
    openPowerSettings: vi.fn(async () => undefined),
    ...over,
  };
}

describe("performance store", () => {
  it("loads, applies and restores", async () => {
    const s = createPerformanceStore(api());
    await s.getState().load();
    expect(s.getState().status).toBe("ready");
    expect(await s.getState().apply("silent")).toBe(true);
    expect(s.getState().data?.selectedProfileId).toBe("silent");
    expect(s.getState().busy).toBeNull();
    expect(await s.getState().restore()).toBe(true);
    expect(s.getState().data?.canRestore).toBe(false);
  });

  it("keeps the data and reports a failed action inline", async () => {
    const s = createPerformanceStore(api({ applyProfile: vi.fn(async () => Promise.reject(denied)) }));
    await s.getState().load();
    expect(await s.getState().apply("silent")).toBe(false);
    expect(s.getState().actionError).toEqual({ action: "apply", error: denied });
    expect(s.getState().data).not.toBeNull();
  });

  it("shows a first-load failure", async () => {
    const s = createPerformanceStore(api({ getState: vi.fn(async () => Promise.reject(denied)) }));
    await s.getState().load();
    expect(s.getState().status).toBe("error");
    expect(s.getState().loadError?.code).toBe("permissionDenied");
  });

  it("runs one action at a time", async () => {
    let release: (v: PerformanceState) => void = () => {};
    const a = api({ applyProfile: vi.fn(() => new Promise<PerformanceState>((r) => (release = r))) });
    const s = createPerformanceStore(a);
    await s.getState().load();
    const first = s.getState().apply("silent");
    expect(s.getState().busy).toEqual({ action: "apply", profileId: "silent" });
    expect(await s.getState().apply("balanced")).toBe(false);
    release(state("silent"));
    expect(await first).toBe(true);
    expect(a.applyProfile).toHaveBeenCalledTimes(1);
  });
});
