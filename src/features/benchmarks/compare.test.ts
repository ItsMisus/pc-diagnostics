import { describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import { formatNanoseconds, formatOpsPerSec, formatSignedPercent, formatUnitValue } from "@/lib/format";
import { comparisonSummary, deltaView } from "./compare";
import { delta } from "./testing/fixtures";

describe("deltaView", () => {
  it("colours improvements green when higher is better", () => {
    expect(deltaView(delta({ changePercent: 10 }), true)).toMatchObject({ text: "+10,0 %", tone: "ok", direction: "better" });
    expect(deltaView(delta({ changePercent: -10 }), true)).toMatchObject({ text: "−10,0 %", tone: "bad", direction: "worse" });
  });

  it("inverts the meaning when lower is better (latency)", () => {
    expect(deltaView(delta({ changePercent: -8, higherIsBetter: false }), true).tone).toBe("ok");
    expect(deltaView(delta({ changePercent: 8, higherIsBetter: false }), true).tone).toBe("bad");
  });

  it("keeps small changes neutral as noise", () => {
    expect(deltaView(delta({ changePercent: 2.9 }), true)).toMatchObject({ tone: "neutral", direction: "noise" });
    expect(deltaView(delta({ changePercent: -1 }), true).tone).toBe("neutral");
  });

  it("never colours non-comparable or undefined deltas", () => {
    expect(deltaView(delta({ changePercent: 50 }), false)).toMatchObject({ tone: "neutral", direction: "unknown" });
    expect(deltaView(delta({ changePercent: null }), true)).toMatchObject({ text: "n/d", tone: "neutral" });
  });

  it("summarises comparability", () => {
    const base = { currentRunId: "a", previousRunId: "b", caveats: ["Batteria"], deltas: [delta()] };
    expect(comparisonSummary({ ...base, comparable: true }).kind).toBe("comparable");
    expect(comparisonSummary({ ...base, comparable: false, deltas: [] })).toEqual({
      kind: "notComparable",
      caveats: ["Batteria"],
    });
  });
});

describe("result formatters", () => {
  it("formats signed percentages", () => {
    expect(formatSignedPercent(4.24)).toBe("+4,2 %");
    expect(formatSignedPercent(-0.04)).toBe("0,0 %");
    expect(formatSignedPercent(Number.NaN)).toBe("—");
  });

  it("formats latency and throughput units", () => {
    expect(formatNanoseconds(85.34)).toBe("85,3 ns");
    expect(formatNanoseconds(1500)).toBe("1,50 µs");
    expect(formatUnitValue(250, "microseconds")).toBe("250 µs");
    expect(formatUnitValue(2500, "microseconds")).toBe("2,50 ms");
    expect(formatOpsPerSec(4_560_000)).toBe("4,56 Mop/s");
    expect(formatOpsPerSec(950)).toBe("950 op/s");
    expect(formatOpsPerSec(21_770_330_229_964.8)).toBe("21,8 Top/s");
    expect(formatUnitValue(3 * 1024 ** 3, "bytesPerSecond")).toBe("3,0 GB/s");
    expect(formatUnitValue(12_345, "none")).toBe("12,3 k");
    expect(formatUnitValue(42, "count")).toBe("42");
  });
});

describe("deltaView in English", () => {
  it("describes the change in the current language", () => {
    setLang("en");
    try {
      expect(deltaView(delta({ changePercent: 10 }), true).description).toBe("Better than the previous run");
      expect(deltaView(delta({ changePercent: 1 }), true).description).toBe(
        "Change within ±3%: consistent with normal run-to-run variation",
      );
      expect(deltaView(delta({ changePercent: null }), true)).toMatchObject({ text: "n/a" });
    } finally {
      setLang("it");
    }
    expect(deltaView(delta({ changePercent: 10 }), true).description).toBe("Migliore dell'esecuzione precedente");
  });
});
