import { describe, expect, it } from "vitest";
import { finding, report } from "@/features/diagnostics/testing/fixtures";
import { gpuInfo, gpuSample, missing, ok, sample } from "@/features/monitoring/testing/fixtures";
import type { AppError } from "@/types/generated/AppError";
import type { HardwareInventory } from "@/types/generated/HardwareInventory";
import type { SystemInfo } from "@/types/generated/SystemInfo";
import {
  deviceIdentity,
  heroChips,
  isRecent,
  overallState,
  RECENT_REPORT_MS,
  recentWindow,
  shapeCaptionKey,
} from "./model";

const error: AppError = { code: "internal", message: "boom" } as AppError;

describe("overallState", () => {
  it("is loading until the latest report is known", () => {
    expect(overallState({ status: "idle", report: null, error: null })).toEqual({ kind: "loading" });
    expect(overallState({ status: "loading", report: null, error: null })).toEqual({ kind: "loading" });
  });

  it("says the diagnosis never ran when there is no report", () => {
    expect(overallState({ status: "ready", report: null, error: null })).toEqual({ kind: "neverRun" });
  });

  it("reports a load error", () => {
    expect(overallState({ status: "error", report: null, error })).toEqual({ kind: "error", error });
  });

  it("is clear only when the report has no findings", () => {
    expect(overallState({ status: "ready", report: report({ generatedAt: 5 }), error: null })).toEqual({ kind: "clear", at: 5 });
  });

  it("takes the tone of the most severe finding and counts each severity", () => {
    const r = report({
      findings: [finding("a", { severity: "warning" }), finding("b", { severity: "info" }), finding("c", { severity: "warning" })],
    });
    const s = overallState({ status: "ready", report: r, error: null });
    expect(s).toMatchObject({ kind: "findings", highest: "warning", tone: "warn", counts: { critical: 0, warning: 2, info: 1 } });
    const bad = overallState({ status: "ready", report: report({ findings: [finding("x", { severity: "critical" })] }), error: null });
    expect(bad).toMatchObject({ highest: "critical", tone: "bad" });
    const info = overallState({ status: "ready", report: report({ findings: [finding("x", { severity: "info" })] }), error: null });
    expect(info).toMatchObject({ highest: "info", tone: "neutral" });
  });

  it("keeps a report on screen while a reload fails", () => {
    expect(overallState({ status: "error", report: report(), error })).toMatchObject({ kind: "clear" });
  });
});

describe("isRecent", () => {
  it("is true only for a report younger than a day", () => {
    const s = overallState({ status: "ready", report: report({ generatedAt: 0 }), error: null });
    expect(isRecent(s, RECENT_REPORT_MS - 1)).toBe(true);
    expect(isRecent(s, RECENT_REPORT_MS)).toBe(false);
    expect(isRecent({ kind: "neverRun" }, 0)).toBe(false);
  });
});

const sys = (patch: Partial<SystemInfo> = {}): SystemInfo => ({
  osName: "macOS",
  osVersion: "26.1",
  osLongVersion: null,
  kernelVersion: null,
  hostname: null,
  cpuArch: "aarch64",
  buildArch: "aarch64",
  bootTimeUnixS: null,
  uptimeS: null,
  ...patch,
});

const inventory = (patch: { productName?: string | null; brand?: string | null; gpuName?: string } = {}): HardwareInventory => ({
  collectedAt: 0,
  cpu: { vendor: null, brand: patch.brand === undefined ? "Apple M5" : patch.brand, arch: "aarch64", physicalCores: 10, logicalCores: 10, maxMhz: missing },
  gpus: [{ ...gpuInfo("g"), name: patch.gpuName ?? "Apple M5" }],
  memory: { totalBytes: 16 * 1024 ** 3, modules: missing },
  physicalDisks: missing,
  volumes: [],
  formFactor: {
    formFactor: missing,
    basis: null,
    internalBattery: missing,
    productName: patch.productName === undefined ? "MacBook Air (13-inch, M5)" : patch.productName,
    modelIdentifier: null,
    chassisTypes: [],
  },
});

describe("deviceIdentity", () => {
  it("names the product, with the OS as a separate line", () => {
    const id = deviceIdentity(inventory(), sys());
    expect(id).toMatchObject({ name: "MacBook Air (13-inch, M5)", os: "macOS 26.1", cpu: "Apple M5", logicalCores: 10 });
  });

  it("drops a GPU name that repeats the CPU (Apple silicon)", () => {
    expect(deviceIdentity(inventory(), sys()).gpu).toBeNull();
    expect(deviceIdentity(inventory({ gpuName: "NVIDIA RTX 4070" }), sys()).gpu).toBe("NVIDIA RTX 4070");
  });

  it("falls back to the OS name, and omits what is not reported", () => {
    const id = deviceIdentity(inventory({ productName: null, brand: "  Intel(R)   Core(TM) i7 " }), sys({ osName: "Windows 11 Pro", osVersion: null }));
    expect(id.name).toBe("Windows 11 Pro");
    expect(id.os).toBeNull();
    expect(id.cpu).toBe("Intel(R) Core(TM) i7");
    const none = deviceIdentity(null, null);
    expect(none).toMatchObject({ name: null, os: null, cpu: null, gpu: null, memoryBytes: null });
  });
});

describe("shapeCaptionKey", () => {
  it("says where the model comes from", () => {
    expect(shapeCaptionKey({ origin: "override", shape: "tower" })).toBe("shapeOverride");
    expect(shapeCaptionKey({ origin: "pending", shape: null })).toBe("shapePending");
    expect(shapeCaptionKey({ origin: "unknown", shape: null, why: { kind: "noModel", formFactor: "allInOne" } })).toBe("shapeUnknown");
    expect(shapeCaptionKey({ origin: "detected", shape: "laptop", formFactor: "laptop", basis: "productName" })).toBe("shapeDetected_productName");
    expect(shapeCaptionKey({ origin: "detected", shape: "laptop", formFactor: "laptop", basis: "modelIdentifier" })).toBe(
      "shapeDetected_modelIdentifier",
    );
    expect(shapeCaptionKey({ origin: "detected", shape: "tower", formFactor: "desktop", basis: "chassisType" })).toBe("shapeDetected_chassisType");
    expect(shapeCaptionKey({ origin: "detected", shape: "tower", formFactor: "desktop", basis: null })).toBe("shapeDetected_unknownBasis");
  });
});

describe("recentWindow", () => {
  it("keeps the last minute and fixes the domain to it", () => {
    const points = Array.from({ length: 100 }, (_, i) => ({ t: i * 1000, v: i }));
    const w = recentWindow(points, 60_000);
    expect(w.domain).toEqual([39_000, 99_000]);
    expect(w.points[0]?.t).toBe(39_000);
    expect(w.points).toHaveLength(61);
  });

  it("keeps a short buffer whole, with a full-minute domain", () => {
    const w = recentWindow([{ t: 5000, v: 1 }, { t: 6000, v: null }], 60_000);
    expect(w.points).toHaveLength(2);
    expect(w.domain).toEqual([-54_000, 6000]);
  });

  it("handles an empty series", () => {
    expect(recentWindow([], 60_000)).toEqual({ points: [], domain: [0, 60_000] });
  });
});

describe("heroChips", () => {
  it("shows only readings the sample carries", () => {
    expect(heroChips(null, null)).toEqual([]);
    expect(heroChips(sample(1), null)).toEqual(["cpu", "ram"]);
    const s = sample(1, { gpus: [gpuSample("g", ok(40))] });
    expect(heroChips(s, "g")).toEqual(["cpu", "ram", "gpu"]);
    expect(heroChips(s, "other")).toEqual(["cpu", "ram"]);
    const hot = sample(1, { cpu: { ...sample(1).cpu, temperatureC: ok(60, "celsius") } });
    expect(heroChips(hot, null)).toEqual(["cpu", "ram", "cpuTemp"]);
  });
});
