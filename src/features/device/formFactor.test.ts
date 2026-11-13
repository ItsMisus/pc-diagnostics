import { describe, expect, it } from "vitest";
import type { FormFactor } from "@/types/generated/FormFactor";
import type { FormFactorInfo } from "@/types/generated/FormFactorInfo";
import type { Metric } from "@/types/generated/Metric";
import type { AppError } from "@/types/generated/AppError";
import type { DeviceShape } from "@/types/generated/DeviceShape";
import { DEVICE_SHAPES, isDeviceShape, resolveDeviceShape as resolve, shapeForFormFactor } from "./formFactor";

const resolveDeviceShape = (override: DeviceShape | null | "loading", detected: FormFactorInfo | null | undefined, inventoryError: AppError | null = null) =>
  resolve({ override, detected, inventoryError });

const ok = <T>(value: T): Metric<T> => ({ status: "available", value, unit: "none", source: "ioKit", at: 1 });

/** This Mac, as measured: MacBook Air (13-inch, M5), Mac17,3, battery installed. */
const macBookAir: FormFactorInfo = {
  formFactor: ok<FormFactor>("laptop"),
  basis: "productName",
  internalBattery: ok(true),
  productName: "MacBook Air (13-inch, M5)",
  modelIdentifier: "Mac17,3",
  chassisTypes: [],
};

const detected = (formFactor: Metric<FormFactor>, patch: Partial<FormFactorInfo> = {}): FormFactorInfo => ({
  ...macBookAir,
  formFactor,
  ...patch,
});

describe("shapeForFormFactor", () => {
  it("maps the three modelled form factors and nothing else", () => {
    expect(shapeForFormFactor("desktop")).toBe("tower");
    expect(shapeForFormFactor("laptop")).toBe("laptop");
    expect(shapeForFormFactor("mini")).toBe("mini");
    expect(shapeForFormFactor("allInOne")).toBe("allInOne");
    expect(shapeForFormFactor("other")).toBeNull();
    expect(shapeForFormFactor("other")).toBeNull();
  });
});

describe("resolveDeviceShape", () => {
  it("prefers the user's choice over detection", () => {
    expect(resolveDeviceShape("tower", macBookAir)).toEqual({ origin: "override", shape: "tower" });
    expect(resolveDeviceShape("mini", null)).toEqual({ origin: "override", shape: "mini" });
  });

  it("uses the detected form factor and keeps its basis", () => {
    expect(resolveDeviceShape(null, macBookAir)).toEqual({
      origin: "detected",
      shape: "laptop",
      formFactor: "laptop",
      basis: "productName",
    });
    const pc = detected(ok<FormFactor>("desktop"), { basis: "chassisType", chassisTypes: [3] });
    expect(resolveDeviceShape(null, pc)).toMatchObject({ origin: "detected", shape: "tower", basis: "chassisType" });
  });

  it("is pending while the inventory is not loaded", () => {
    expect(resolveDeviceShape(null, null)).toEqual({ origin: "pending", shape: null });
    expect(resolveDeviceShape(null, undefined)).toEqual({ origin: "pending", shape: null });
  });

  it("is pending while the settings are loading, so a saved choice is not swapped in later", () => {
    expect(resolveDeviceShape("loading", macBookAir)).toEqual({ origin: "pending", shape: null });
    const inconclusive = detected({ status: "unavailable", reason: { kind: "notSupported" } });
    expect(resolveDeviceShape("loading", inconclusive).origin).toBe("pending");
  });

  it("asks the user when the inventory failed to load", () => {
    const error: AppError = { code: "internal", message: "boom", detail: null, retriable: true };
    expect(resolveDeviceShape(null, null, error)).toEqual({
      origin: "unknown",
      shape: null,
      why: { kind: "unavailable", reason: { kind: "failed", detail: error } },
    });
    // A saved choice still wins; loading settings still waits.
    expect(resolveDeviceShape("laptop", null, error)).toEqual({ origin: "override", shape: "laptop" });
    expect(resolveDeviceShape("loading", null, error).origin).toBe("pending");
  });

  it("asks the user when detection is inconclusive or failed", () => {
    const reason = { kind: "inconclusive", detail: "the chassis type indicates desktop but an internal battery is present" } as const;
    const r = resolveDeviceShape(null, detected({ status: "unavailable", reason }, { basis: null }));
    expect(r).toEqual({ origin: "unknown", shape: null, why: { kind: "unavailable", reason } });
    const denied = resolveDeviceShape(null, detected({ status: "unavailable", reason: { kind: "permissionDenied" } }));
    expect(denied.origin).toBe("unknown");
  });

  it("asks the user for form factors without a model", () => {
    const rack = detected(ok<FormFactor>("other"), { productName: null, internalBattery: ok(false) });
    expect(resolveDeviceShape(null, rack)).toEqual({
      origin: "unknown",
      shape: null,
      why: { kind: "noModel", formFactor: "other" },
    });
    // The user's choice still wins.
    expect(resolveDeviceShape("tower", rack).shape).toBe("tower");
    const imac = detected(ok<FormFactor>("allInOne"), { productName: "iMac (24-inch, 2024)", internalBattery: ok(false) });
    expect(resolveDeviceShape(null, imac).shape).toBe("allInOne");
  });

  it("ignores an unknown stored value", () => {
    expect(resolveDeviceShape("phone" as never, macBookAir).origin).toBe("detected");
  });
});

describe("isDeviceShape", () => {
  it("accepts exactly the listed shapes", () => {
    for (const s of DEVICE_SHAPES) expect(isDeviceShape(s)).toBe(true);
    expect(isDeviceShape("desktop")).toBe(false);
    expect(isDeviceShape(null)).toBe(false);
  });
});
