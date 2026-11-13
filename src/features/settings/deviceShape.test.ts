import { describe, expect, it } from "vitest";
import type { AppError } from "@/types/generated/AppError";
import type { FormFactor } from "@/types/generated/FormFactor";
import type { FormFactorInfo } from "@/types/generated/FormFactorInfo";
import type { Metric } from "@/types/generated/Metric";
import {
  SHAPE_CHOICES,
  choiceForOverride,
  detectionEvidence,
  detectionResult,
  overrideForChoice,
} from "./deviceShape";

const ok = <T>(value: T): Metric<T> => ({ status: "available", value, unit: "none", source: "ioKit", at: 1 });

const macBookAir: FormFactorInfo = {
  formFactor: ok<FormFactor>("laptop"),
  basis: "productName",
  internalBattery: ok(true),
  productName: "MacBook Air (13-inch, M5)",
  modelIdentifier: "Mac17,3",
  chassisTypes: [],
};

const windowsTower: FormFactorInfo = {
  formFactor: ok<FormFactor>("desktop"),
  basis: "chassisType",
  internalBattery: ok(false),
  productName: null,
  modelIdentifier: null,
  chassisTypes: [3],
};

describe("shape choice", () => {
  it("lists Automatico first, then the models", () => {
    expect(SHAPE_CHOICES).toEqual(["auto", "tower", "laptop", "mini", "allInOne"]);
  });

  it("maps no stored choice to Automatico and back", () => {
    expect(choiceForOverride(null)).toBe("auto");
    expect(choiceForOverride("mini")).toBe("mini");
    expect(overrideForChoice("auto")).toBeNull();
    expect(overrideForChoice("tower")).toBe("tower");
  });
});

describe("detectionResult", () => {
  it("reports what was detected and from what", () => {
    expect(detectionResult(macBookAir, null)).toEqual({
      origin: "detected",
      shape: "laptop",
      formFactor: "laptop",
      basis: "productName",
    });
  });

  it("is pending while the inventory loads", () => {
    expect(detectionResult(null, null).origin).toBe("pending");
  });

  it("gives the reason when loading the inventory failed", () => {
    const error: AppError = { code: "internal", message: "boom", detail: null, retriable: true };
    const r = detectionResult(null, error);
    expect(r.origin).toBe("unknown");
    if (r.origin === "unknown") expect(r.why.kind).toBe("unavailable");
  });

  it("shows the all-in-one model and asks the user for \"other\" (no 3D model)", () => {
    const aio = detectionResult({ ...macBookAir, formFactor: ok<FormFactor>("allInOne") }, null);
    expect(aio.shape).toBe("allInOne");
    const r = detectionResult({ ...macBookAir, formFactor: ok<FormFactor>("other") }, null);
    expect(r).toEqual({ origin: "unknown", shape: null, why: { kind: "noModel", formFactor: "other" } });
  });
});

describe("detectionEvidence", () => {
  it("shows the value the basis names", () => {
    expect(detectionEvidence(macBookAir)).toEqual({ kind: "productName", value: "MacBook Air (13-inch, M5)" });
    expect(detectionEvidence({ ...macBookAir, basis: "modelIdentifier" })).toEqual({
      kind: "modelIdentifier",
      value: "Mac17,3",
    });
    expect(detectionEvidence(windowsTower)).toEqual({ kind: "chassisType", value: "3" });
  });

  it("falls back to the first value present when there is no basis", () => {
    const inconclusive: FormFactorInfo = {
      ...windowsTower,
      basis: null,
      formFactor: { status: "unavailable", reason: { kind: "inconclusive", detail: "battery" } },
      chassisTypes: [3, 9],
    };
    expect(detectionEvidence(inconclusive)).toEqual({ kind: "chassisType", value: "3, 9" });
  });

  it("returns nothing without evidence (never an invented value)", () => {
    expect(detectionEvidence(null)).toBeNull();
    expect(detectionEvidence({ ...windowsTower, chassisTypes: [] })).toBeNull();
    expect(detectionEvidence({ ...macBookAir, productName: "  " , modelIdentifier: null, basis: null })).toBeNull();
  });
});
