import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import { causeLabel, causeStatus, overallHeadline, overallState, relatedSection } from "./model";
import { finding } from "./testing/fixtures";

afterEach(() => setLang("it"));

describe("cause status", () => {
  it("distinguishes a confirmed cause from a possible one and from a bare observation", () => {
    expect(causeStatus("confirmed")).toBe("confirmed");
    expect(causeStatus("hypothesis")).toBe("possible");
    expect(causeStatus("observation")).toBe("undetermined");
    const labels = (["confirmed", "possible", "undetermined"] as const).map(causeLabel);
    expect(labels).toEqual(["Causa confermata", "Causa possibile", "Causa non determinata"]);
  });

  it("has English labels", () => {
    setLang("en");
    expect(causeLabel("confirmed")).toBe("Confirmed cause");
    expect(causeLabel("possible")).toBe("Possible cause");
  });
});

describe("overall state", () => {
  it("is ok with no findings, and says so without a score", () => {
    const s = overallState([]);
    expect(s).toEqual({ worst: null, tone: "ok", counts: { critical: 0, warning: 0, info: 0 }, total: 0 });
    expect(overallHeadline(s)).toBe("Nessun problema rilevato");
  });

  it("stays ok when there is only information", () => {
    const s = overallState([finding("a", { severity: "info" })]);
    expect(s.tone).toBe("ok");
    expect(s.worst).toBe("info");
    expect(overallHeadline(s)).toBe("Nessun problema rilevato");
  });

  it("takes the most severe level and names its count", () => {
    const warn = overallState([finding("a"), finding("b"), finding("c", { severity: "info" })]);
    expect(warn.tone).toBe("warn");
    expect(warn.worst).toBe("warning");
    expect(overallHeadline(warn)).toBe("2 avvisi da controllare");

    const bad = overallState([finding("a"), finding("b", { severity: "critical" })]);
    expect(bad.tone).toBe("bad");
    expect(bad.counts).toEqual({ critical: 1, warning: 1, info: 0 });
    expect(overallHeadline(bad)).toBe("1 problema critico");
  });

  it("follows the language", () => {
    setLang("en");
    expect(overallHeadline(overallState([finding("a", { severity: "critical" }), finding("b", { severity: "critical" })]))).toBe(
      "2 critical problems",
    );
    expect(overallHeadline(overallState([]))).toBe("No problems found");
  });
});

describe("related section", () => {
  it("points load and memory rules to the processes", () => {
    for (const id of ["cpu.process.dominant", "cpu.utilization.sustained", "memory.pressure.high", "processes.impact.high"])
      expect(relatedSection(id, "macos")).toBe("processes");
  });

  it("points driver problems to the device list only on Windows", () => {
    for (const id of ["drivers.device.problem", "drivers.updates.available", "gpu.driver.problem"]) {
      expect(relatedSection(id, "windows")).toBe("drivers");
      expect(relatedSection(id, "macos")).toBeNull();
    }
  });

  it("points benchmark comparisons to the history, and offers nothing elsewhere", () => {
    expect(relatedSection("cpu.benchmark.sustained_drop", "macos")).toBe("history");
    expect(relatedSection("storage.benchmark.drop", "windows")).toBe("history");
    for (const id of ["storage.volume.low_space", "storage.disk.health", "gpu.vram.high", "cpu.frequency.limit_cause", "x"])
      expect(relatedSection(id, "windows")).toBeNull();
  });
});
