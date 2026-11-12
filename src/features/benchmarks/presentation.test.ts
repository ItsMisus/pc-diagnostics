import { describe, expect, it } from "vitest";
import {
  componentAvailability,
  isRunOnScreen,
  latestComponentResult,
  splitUnitValue,
  stateKindForReason,
} from "./model";
import { capability, result } from "./testing/fixtures";

const unavailable = (kind: "notSupported" | "notImplemented") =>
  ({ status: "unavailable", reason: { kind } }) as const;

describe("splitUnitValue", () => {
  it("separates the number from a trailing unit", () => {
    expect(splitUnitValue("12,3 GB/s")).toEqual({ value: "12,3", unit: "GB/s" });
    expect(splitUnitValue("1.20 Gop/s")).toEqual({ value: "1.20", unit: "Gop/s" });
    expect(splitUnitValue("4,56 M")).toEqual({ value: "4,56", unit: "M" });
    expect(splitUnitValue("87 ns")).toEqual({ value: "87", unit: "ns" });
  });

  it("handles percentages with and without a space", () => {
    expect(splitUnitValue("12,5 %")).toEqual({ value: "12,5", unit: "%" });
    expect(splitUnitValue("12.5%")).toEqual({ value: "12.5", unit: "%" });
  });

  it("keeps text without a unit whole", () => {
    expect(splitUnitValue("950")).toEqual({ value: "950", unit: null });
    expect(splitUnitValue("—")).toEqual({ value: "—", unit: null });
    expect(splitUnitValue("")).toEqual({ value: "", unit: null });
  });
});

describe("componentAvailability", () => {
  it("is none without capabilities", () => {
    expect(componentAvailability([])).toEqual({ status: "none" });
  });

  it("lists the available kinds", () => {
    const caps = [capability("cpuSingleCore"), capability("cpuMultiCore", { availability: unavailable("notSupported") })];
    expect(componentAvailability(caps)).toEqual({ status: "available", kinds: ["cpuSingleCore"] });
  });

  it("gives the first reason when nothing can run", () => {
    const caps = [capability("gpuCompute", { availability: unavailable("notSupported") })];
    expect(componentAvailability(caps)).toEqual({ status: "unavailable", reason: { kind: "notSupported" } });
  });
});

describe("stateKindForReason", () => {
  it("maps each reason to a state view", () => {
    expect(stateKindForReason({ kind: "notSupported" })).toBe("unsupported");
    expect(stateKindForReason({ kind: "permissionDenied" })).toBe("permission");
    expect(stateKindForReason({ kind: "notImplemented" })).toBe("notImplemented");
    expect(stateKindForReason({ kind: "providerMissing", detail: "x" })).toBe("unavailable");
    expect(stateKindForReason({ kind: "inconclusive", detail: "x" })).toBe("unavailable");
    expect(
      stateKindForReason({ kind: "failed", detail: { code: "internal", message: "x", detail: null, retriable: false } }),
    ).toBe("error");
  });
});

describe("latestComponentResult", () => {
  it("picks the most recent result among the component's kinds", () => {
    const older = result("a", { kind: "cpuSingleCore", finishedAt: 10 });
    const newer = result("b", { kind: "cpuMultiCore", finishedAt: 20 });
    const other = result("c", { kind: "gpuCompute", finishedAt: 30 });
    const byKind = { cpuSingleCore: older, cpuMultiCore: newer, gpuCompute: other };
    expect(latestComponentResult(byKind, "cpu")?.runId).toBe("b");
    expect(latestComponentResult(byKind, "memory")).toBeNull();
  });
});

describe("isRunOnScreen", () => {
  it("is true only on the running component's screen", () => {
    const run = { kind: "storageRandom" } as const;
    expect(isRunOnScreen(run, "benchmarks", "storage")).toBe(true);
    expect(isRunOnScreen(run, "benchmarks", "cpu")).toBe(false);
    expect(isRunOnScreen(run, "benchmarks", null)).toBe(false);
    expect(isRunOnScreen(run, "overview", "storage")).toBe(false);
    expect(isRunOnScreen(null, "benchmarks", "storage")).toBe(false);
  });
});
