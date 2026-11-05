import { describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import {
  ALL_KINDS,
  buildRequest,
  componentOfKind,
  defaultDraft,
  defaultGpuTarget,
  defaultVolume,
  durationBounds,
  gpuStopCopy,
  headlineMetric,
  isDebugBuild,
  kindLabel,
  powerSourceLabel,
  presets,
  resolveGpuTarget,
  resolvedFileMib,
  stateLabel,
  storageConsentCopy,
  threadLimit,
} from "./model";
import { capability, gpuTarget, result, volume } from "./testing/fixtures";

const ctx = { logicalCores: 8, volumes: [volume("C:\\"), volume("D:\\", { availableBytes: 10 * 1024 ** 2 })] };

describe("kind → component", () => {
  it("maps every kind", () => {
    expect(ALL_KINDS).toHaveLength(8);
    expect(componentOfKind("cpuSustained")).toBe("cpu");
    expect(componentOfKind("memoryLatency")).toBe("memory");
    expect(componentOfKind("storageRandom")).toBe("storage");
    expect(componentOfKind("gpuCompute")).toBe("gpu");
  });
});

describe("buildRequest", () => {
  it("rejects unavailable capabilities", () => {
    const cap = capability("gpuCompute", { availability: { status: "unavailable", reason: { kind: "notImplemented" } } });
    expect(buildRequest(cap, defaultDraft(cap, []), ctx).ok).toBe(false);
  });

  it("clamps the duration into bounds and applies the default", () => {
    const cap = capability("cpuSingleCore");
    const draft = defaultDraft(cap, []);
    const r1 = buildRequest(cap, { ...draft, durationS: 999 }, ctx);
    expect(r1.ok && r1.request.durationS).toBe(60);
    const r2 = buildRequest(cap, { ...draft, durationS: 0 }, ctx);
    expect(r2.ok && r2.request.durationS).toBe(1);
    const r3 = buildRequest(cap, { ...draft, durationS: null }, ctx);
    expect(r3.ok && r3.request.durationS).toBe(10);
    expect(r3.ok && r3.request.threads).toBeNull();
  });

  it("uses null duration for non-timed tests", () => {
    const cap = capability("memoryLatency", { defaultDurationS: null, maxDurationS: null });
    expect(durationBounds(cap)).toBeNull();
    const r = buildRequest(cap, { ...defaultDraft(cap, []), durationS: 30 }, ctx);
    expect(r.ok && r.request.durationS).toBeNull();
  });

  it("clamps threads to maxThreads or the logical core count, and drops them when unsupported", () => {
    const multi = capability("cpuMultiCore", { maxThreads: 4 });
    const r1 = buildRequest(multi, { ...defaultDraft(multi, []), threads: 16 }, ctx);
    expect(r1.ok && r1.request.threads).toBe(4);
    const r2 = buildRequest(capability("cpuMultiCore"), { ...defaultDraft(multi, []), threads: 0 }, ctx);
    expect(r2.ok && r2.request.threads).toBe(1);
    expect(threadLimit(capability("cpuMultiCore"), 8)).toBe(8);
    const single = capability("cpuSingleCore");
    const r3 = buildRequest(single, { ...defaultDraft(single, []), threads: 4 }, ctx);
    expect(r3.ok && r3.request.threads).toBeNull();
  });

  it("gates storage tests on explicit consent", () => {
    const cap = capability("storageSequential");
    const draft = defaultDraft(cap, ctx.volumes);
    expect(draft.storageVolume).toBe("C:\\");
    expect(draft.consent).toBe(false);
    const denied = buildRequest(cap, draft, ctx);
    expect(denied.ok).toBe(false);
    const granted = buildRequest(cap, { ...draft, consent: true }, ctx);
    expect(granted.ok).toBe(true);
    if (granted.ok) {
      expect(granted.request.storageWriteConsent).toBe(true);
      expect(granted.request.storageVolume).toBe("C:\\");
      expect(granted.request.storageFileMib).toBe(256);
    }
  });

  it("never sends storage consent for non-storage tests", () => {
    const cap = capability("memoryBandwidth");
    const r = buildRequest(cap, { ...defaultDraft(cap, []), consent: true, storageVolume: "C:\\" }, ctx);
    expect(r.ok && r.request.storageWriteConsent).toBe(false);
    expect(r.ok && r.request.storageVolume).toBeNull();
  });

  it("clamps file size, requires a known volume and enough free space", () => {
    const cap = capability("storageRandom");
    const draft = { ...defaultDraft(cap, ctx.volumes), consent: true };
    expect(resolvedFileMib(cap, { ...draft, storageFileMib: 99_999 })).toBe(1024);
    const big = buildRequest(cap, { ...draft, storageFileMib: 99_999 }, ctx);
    expect(big.ok && big.request.storageFileMib).toBe(1024);
    expect(buildRequest(cap, { ...draft, storageVolume: null }, ctx).ok).toBe(false);
    expect(buildRequest(cap, { ...draft, storageVolume: "Z:\\" }, ctx).ok).toBe(false);
    const full = buildRequest(cap, { ...draft, storageVolume: "D:\\" }, ctx);
    expect(full.ok).toBe(false);
    if (!full.ok) expect(full.reason).toMatch(/Spazio libero/);
  });

  it("states the file size in the consent copy", () => {
    expect(storageConsentCopy(256)).toContain("file temporaneo di 256 MiB");
    expect(storageConsentCopy(256)).toContain("Nessun dato personale viene letto o modificato.");
  });
});

describe("helpers", () => {
  it("builds presets inside bounds including default and max", () => {
    expect(presets({ min: 1, max: 45, def: 12 }, [5, 10, 30, 60])).toEqual([5, 10, 12, 30, 45]);
  });

  it("prefers the system volume", () => {
    expect(defaultVolume([volume("E:\\", { removable: true }), volume("D:\\"), volume("C:\\")])).toBe("C:\\");
    expect(defaultVolume([volume("E:\\", { removable: true }), volume("D:\\")])).toBe("D:\\");
    expect(defaultVolume([])).toBeNull();
  });

  it("has no headline for cancelled runs and flags debug builds", () => {
    expect(headlineMetric(result("a"))?.id).toBe("score");
    expect(headlineMetric(result("a", { state: "cancelled" }))).toBeNull();
    expect(isDebugBuild(result("a", { buildProfile: "debug" }))).toBe(true);
    expect(isDebugBuild(result("a"))).toBe(false);
  });
});

describe("GPU targets", () => {
  const unusable = { status: "unavailable", reason: { kind: "notSupported" } } as const;

  it("preselects the default target, or the first usable one when the default is not usable", () => {
    const targets = [gpuTarget("a"), gpuTarget("b", { isDefault: true })];
    expect(defaultGpuTarget(targets)?.id).toBe("b");
    expect(defaultGpuTarget([gpuTarget("a", { isDefault: true, availability: unusable }), gpuTarget("b")])?.id).toBe("b");
    expect(defaultGpuTarget([gpuTarget("a", { isDefault: true, availability: unusable })])).toBeNull();
    expect(defaultGpuTarget([])).toBeNull();
  });

  it("falls back to the default when the picked id is no longer listed", () => {
    const targets = [gpuTarget("a", { isDefault: true }), gpuTarget("b")];
    expect(resolveGpuTarget(targets, "b")?.id).toBe("b");
    expect(resolveGpuTarget(targets, "gone")?.id).toBe("a");
    expect(resolveGpuTarget(targets, null)?.id).toBe("a");
  });

  it("puts the selected target in the request and the default in the initial draft", () => {
    const cap = capability("gpuCompute", { gpuTargets: [gpuTarget("a", { isDefault: true }), gpuTarget("b")] });
    const draft = defaultDraft(cap, []);
    expect(draft.gpuId).toBe("a");
    const r1 = buildRequest(cap, draft, ctx);
    expect(r1.ok && r1.request.gpuId).toBe("a");
    const r2 = buildRequest(cap, { ...draft, gpuId: "b" }, ctx);
    expect(r2.ok && r2.request.gpuId).toBe("b");
    const r3 = buildRequest(cap, { ...draft, gpuId: null }, ctx);
    expect(r3.ok && r3.request.gpuId).toBe("a");
  });

  it("refuses an unusable target with its reason", () => {
    const cap = capability("gpuCompute", {
      gpuTargets: [gpuTarget("a", { isDefault: true }), gpuTarget("b", { availability: unusable })],
    });
    const r = buildRequest(cap, { ...defaultDraft(cap, []), gpuId: "b" }, ctx);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toBe("GPU selezionata non utilizzabile: non esposto da hardware o sistema operativo");
    const none = capability("gpuCompute", { gpuTargets: [gpuTarget("b", { availability: unusable })] });
    expect(buildRequest(none, defaultDraft(none, []), ctx).ok).toBe(false);
  });

  it("sends a null gpuId when no targets are listed", () => {
    const gpu = capability("gpuCompute");
    const r1 = buildRequest(gpu, { ...defaultDraft(gpu, []), gpuId: "stale" }, ctx);
    expect(r1.ok && r1.request.gpuId).toBeNull();
    const storage = capability("storageSequential");
    const r2 = buildRequest(storage, { ...defaultDraft(storage, ctx.volumes), consent: true }, ctx);
    expect(r2.ok && r2.request.gpuId).toBeNull();
  });

  it("describes the thermal stop of the selected target", () => {
    expect(gpuStopCopy({ stopTemperatureC: 87 })).toBe("Arresto automatico a 87 °C (soglia del driver)");
    expect(gpuStopCopy({ stopTemperatureC: null })).toBe(
      "Il driver non fornisce una soglia termica: durata massima ridotta",
    );
  });

  it("caps the duration by the selected GPU target", () => {
    const cap = capability("gpuCompute", { defaultDurationS: 20, maxDurationS: 60 });
    expect(durationBounds(cap, { maxDurationS: 30 })).toEqual({ min: expect.any(Number), max: 30, def: 20 });
    expect(durationBounds(cap, { maxDurationS: 60 })?.max).toBe(60);
    expect(durationBounds(cap)?.max).toBe(60);
  });
});

describe("texts in English", () => {
  it("labels kinds, states and power sources and explains refusals", () => {
    const cap = capability("storageSequential");
    const draft = { ...defaultDraft(cap, ctx.volumes), consent: true };
    setLang("en");
    try {
      expect(kindLabel("cpuSustained")).toBe("CPU sustained load");
      expect(stateLabel("cancelling")).toBe("Stopping");
      expect(powerSourceLabel("ac")).toBe("AC power");
      expect(gpuStopCopy({ stopTemperatureC: 87 })).toBe("Automatic stop at 87 °C (driver threshold)");
      const full = buildRequest(cap, { ...draft, storageVolume: "D:\\" }, ctx);
      expect(!full.ok && full.reason).toBe("Not enough free space on the selected volume");
      expect(storageConsentCopy(256)).toContain("temporary file of 256 MiB");
    } finally {
      setLang("it");
    }
    expect(kindLabel("cpuSustained")).toBe("CPU carico prolungato");
  });
});
