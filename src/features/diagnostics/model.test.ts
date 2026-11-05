import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import { createDiagStore } from "./diagStore";
import {
  confidenceLabel,
  coverageSentence,
  coverageSummary,
  coverageTone,
  findingKindLabel,
  groupFindings,
  noFindingsCopy,
  severityCounts,
  severityLabel,
  severityTone,
  sortFindings,
  topFindings,
} from "./model";
import { coverage, finding, report } from "./testing/fixtures";

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("labels", () => {
  it("uses distinct wording per finding kind", () => {
    const labels = (["observation", "hypothesis", "confirmed"] as const).map(findingKindLabel);
    expect(labels).toEqual(["Osservazione", "Ipotesi", "Confermato"]);
    expect(new Set(labels).size).toBe(3);
  });

  it("maps severities to tones: critical=bad, warning=warn, info=neutral", () => {
    expect(severityTone("critical")).toBe("bad");
    expect(severityTone("warning")).toBe("warn");
    expect(severityTone("info")).toBe("neutral");
    expect(severityLabel("critical")).toBe("Critico");
    expect(confidenceLabel("high")).toBe("Confidenza alta");
  });

  it("never colours coverage green", () => {
    expect(coverageTone("evaluated")).toBe("neutral");
    expect(coverageTone("partial")).toBe("warn");
    expect(coverageTone("notEvaluated")).toBe("warn");
  });
});

describe("grouping and sorting", () => {
  const findings = [
    finding("info-1", { severity: "info" }),
    finding("warn-obs", { severity: "warning", kind: "observation" }),
    finding("crit", { severity: "critical", confidence: "low" }),
    finding("warn-conf", { severity: "warning", kind: "confirmed" }),
    finding("warn-hyp-high", { severity: "warning", kind: "hypothesis", confidence: "high" }),
    finding("warn-hyp-low", { severity: "warning", kind: "hypothesis", confidence: "low" }),
  ];

  it("sorts by severity, then kind, then confidence", () => {
    expect(sortFindings(findings).map((f) => f.ruleId)).toEqual([
      "crit",
      "warn-conf",
      "warn-hyp-high",
      "warn-hyp-low",
      "warn-obs",
      "info-1",
    ]);
  });

  it("groups into non-empty severity groups in order", () => {
    const groups = groupFindings(findings);
    expect(groups.map((g) => g.severity)).toEqual(["critical", "warning", "info"]);
    expect(groups[1]?.findings).toHaveLength(4);
    expect(groupFindings([finding("a", { severity: "info" })]).map((g) => g.severity)).toEqual(["info"]);
    expect(groupFindings([])).toEqual([]);
  });

  it("counts severities and picks the top 3", () => {
    expect(severityCounts(findings)).toEqual({ critical: 1, warning: 4, info: 1 });
    expect(topFindings(findings).map((f) => f.ruleId)).toEqual(["crit", "warn-conf", "warn-hyp-high"]);
  });
});

describe("coverage summary", () => {
  it("lists gaps, not-evaluated first, and is never complete when something is missing", () => {
    const s = coverageSummary([
      coverage("cpu", "evaluated"),
      coverage("gpu", "partial", "temperatura non disponibile"),
      coverage("storage", "notEvaluated", "SMART non accessibile"),
    ]);
    expect(s).toMatchObject({ evaluated: 1, partial: 1, notEvaluated: 1, total: 3, complete: false });
    expect(s.gaps.map((g) => g.component)).toEqual(["storage", "gpu"]);
    expect(coverageSentence(s)).toBe("1 di 3 componenti valutati completamente, 1 parzialmente, 1 non valutati.");
  });

  it("treats empty coverage as unknown, not complete", () => {
    const s = coverageSummary([]);
    expect(s.complete).toBe(false);
    expect(coverageSentence(s)).toMatch(/non è stata riportata/);
  });
});

describe("English", () => {
  afterEach(() => setLang("it"));

  it("translates labels and the coverage sentence", () => {
    setLang("en");
    expect((["observation", "hypothesis", "confirmed"] as const).map((k) => findingKindLabel(k))).toEqual([
      "Observation",
      "Hypothesis",
      "Confirmed",
    ]);
    expect(severityLabel("critical")).toBe("Critical");
    expect(confidenceLabel("high")).toBe("High confidence");
    expect(noFindingsCopy()).toBe("No problems detected by the available rules");
    const s = coverageSummary([
      coverage("cpu", "evaluated"),
      coverage("gpu", "partial"),
      coverage("storage", "notEvaluated"),
    ]);
    expect(coverageSentence(s)).toBe("1 of 3 components fully evaluated, 1 partially, 1 not evaluated.");
    expect(coverageSentence(coverageSummary([coverage("cpu", "evaluated")]))).toBe("1 of 1 component fully evaluated.");
    expect(coverageSentence(coverageSummary([]))).toBe("Diagnostic coverage was not reported.");
  });
});

describe("diagnostics store", () => {
  it("loads the latest report (null = never ran), runs and exports", async () => {
    const store = createDiagStore({
      getLatest: async () => null,
      run: async () => report({ id: "new", findings: [finding("x")] }),
      exportReport: async (format, redact) => ({ path: `C:\\r.${format}`, bytes: 10, redacted: redact }),
    });
    store.getState().ensureLoaded();
    await flush();
    expect(store.getState().status).toBe("ready");
    expect(store.getState().report).toBeNull();

    await store.getState().run();
    expect(store.getState().report?.id).toBe("new");
    expect(store.getState().running).toBe(false);

    await store.getState().exportReport("html", true);
    const ex = store.getState().export;
    expect(ex.status === "done" && ex.result.redacted).toBe(true);
  });

  it("keeps the previous report when a run fails and reports export notFound", async () => {
    const store = createDiagStore({
      getLatest: async () => report({ id: "old" }),
      run: () => Promise.reject({ code: "internal", message: "boom", detail: null, retriable: true }),
      exportReport: () => Promise.reject({ code: "notFound", message: "no report", detail: null, retriable: false }),
    });
    store.getState().ensureLoaded();
    await flush();
    await store.getState().run();
    expect(store.getState().report?.id).toBe("old");
    expect(store.getState().runError?.code).toBe("internal");
    await store.getState().exportReport("json", false);
    const ex = store.getState().export;
    expect(ex.status === "error" && ex.error.code).toBe("notFound");
  });
});
