import { describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import type { Metric } from "@/types/generated/Metric";
import { gpuPowerReading } from "./gpuPower";
import { missing, ok } from "./testing/fixtures";

const notImplemented: Metric<number> = { status: "unavailable", reason: { kind: "notImplemented" } };

describe("gpuPowerReading", () => {
  it("prefers watts when a provider measures them", () => {
    const r = gpuPowerReading({ powerW: ok(142.37, "watts"), powerPercent: ok(40) });
    expect(r.metric.status === "available" && r.metric.value).toBe(142.37);
    expect(r.format(142.37)).toBe("142,4 W");
  });

  it("falls back to the driver's share of the maximum, never converted to watts", () => {
    const r = gpuPowerReading({ powerW: notImplemented, powerPercent: ok(37.6) });
    expect(r.metric.status === "available" && r.metric.value).toBe(37.6);
    expect(r.format(37.6)).toBe("38 % del massimo");
  });

  it("describes the share of the maximum in English", () => {
    setLang("en");
    try {
      const r = gpuPowerReading({ powerW: notImplemented, powerPercent: ok(37.6) });
      expect(r.format(37.6)).toBe("38% of max");
    } finally {
      setLang("it");
    }
  });

  it("explains the absence with the most specific reason", () => {
    const both = gpuPowerReading({ powerW: notImplemented, powerPercent: missing });
    expect(both.metric.status === "unavailable" && both.metric.reason.kind).toBe("providerMissing");
    const watts = gpuPowerReading({ powerW: missing, powerPercent: notImplemented });
    expect(watts.metric.status === "unavailable" && watts.metric.reason.kind).toBe("providerMissing");
  });
});
