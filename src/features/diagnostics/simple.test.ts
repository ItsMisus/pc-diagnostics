import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import { containsJargon } from "@/lib/plain";
import type { ComponentKind } from "@/types/generated/ComponentKind";
import type { CoverageStatus } from "@/types/generated/CoverageStatus";
import type { Severity } from "@/types/generated/Severity";
import { severityTone } from "./model";
import {
  simpleComponentName,
  simpleCoverageLabel,
  simpleCoverageNote,
  simpleFindingText,
  simpleKindHint,
  simpleNotAGuarantee,
  simpleNothingFound,
  simpleSeverityLabel,
  simpleShortWindowNote,
  simpleSummary,
} from "./simple";
import { coverage, finding } from "./testing/fixtures";

const SEVERITIES: Severity[] = ["info", "warning", "critical"];
const COMPONENTS: Record<ComponentKind, true> = {
  cpu: true,
  gpu: true,
  memory: true,
  storage: true,
  drivers: true,
  processes: true,
  system: true,
};
const STATUSES: CoverageStatus[] = ["evaluated", "partial", "notEvaluated"];

describe("simpleSeverityLabel", () => {
  it("uses plain labels", () => {
    expect(simpleSeverityLabel("info")).toBe("Da sapere");
    expect(simpleSeverityLabel("warning")).toBe("Da controllare");
    expect(simpleSeverityLabel("critical")).toBe("Importante");
  });

  it("keeps the colour semantics of the technical view", () => {
    expect(severityTone("critical")).toBe("bad");
    expect(severityTone("warning")).toBe("warn");
    expect(severityTone("info")).toBe("neutral");
  });
});

describe("simpleKindHint", () => {
  it("marks hypotheses as uncertain and nothing else", () => {
    expect(simpleKindHint("hypothesis")).toBe("Possibile causa, non certa");
    expect(simpleKindHint("observation")).toBeNull();
    expect(simpleKindHint("confirmed")).toBeNull();
  });
});

describe("simpleFindingText", () => {
  it("uses the plain explanation when present", () => {
    const f = finding("memory.pressure", {
      title: "Commit charge near limit",
      simple: { headline: "Il PC sta finendo la memoria", meaning: "I programmi rallentano", action: "Chiudi i programmi" },
    });
    expect(simpleFindingText(f)).toEqual({
      headline: "Il PC sta finendo la memoria",
      meaning: "I programmi rallentano",
      action: "Chiudi i programmi",
      fallback: false,
    });
  });

  it("falls back to title, description and recommendation when the plain text is empty", () => {
    const f = finding("x", { title: "T", description: "D", recommendation: "R" });
    expect(simpleFindingText(f)).toEqual({ headline: "T", meaning: "D", action: "R", fallback: true });
  });

  it("falls back field by field and treats whitespace as empty", () => {
    const f = finding("x", {
      title: "T",
      description: "D",
      recommendation: "R",
      simple: { headline: "Semplice", meaning: "  ", action: "" },
    });
    expect(simpleFindingText(f)).toEqual({ headline: "Semplice", meaning: "D", action: "R", fallback: true });
  });

  it("tolerates reports saved before the field existed", () => {
    const f = finding("x", { title: "T" });
    delete (f as Partial<typeof f>).simple;
    expect(simpleFindingText(f).headline).toBe("T");
  });

  it("leaves the action empty when there is nothing to do", () => {
    expect(simpleFindingText(finding("x", { recommendation: "" })).action).toBe("");
  });
});

describe("simple coverage", () => {
  it("labels each status in plain words and notes only what is missing", () => {
    expect(simpleCoverageLabel("partial")).toBe("Controllo parziale");
    expect(simpleCoverageNote(coverage("cpu", "partial", "temperatura non leggibile su questo PC"))).toBe(
      "temperatura non leggibile su questo PC",
    );
    expect(simpleCoverageNote(coverage("memory", "evaluated", "valutata su 120 s"))).toBe("");
    expect(simpleCoverageNote(coverage("drivers", "notEvaluated"))).toBe("mancano i dati necessari");
    expect(simpleCoverageNote(coverage("gpu", "partial"))).toBe("alcuni dati non erano disponibili");
  });

  it("never uses the technical status words", () => {
    for (const c of Object.keys(COMPONENTS) as ComponentKind[]) {
      expect(simpleComponentName(c).length).toBeGreaterThan(0);
    }
    for (const s of STATUSES) {
      const text = `${simpleCoverageLabel(s)} ${simpleCoverageNote(coverage("cpu", s))}`;
      expect(text).not.toMatch(/Valutato|Parziale|Non valutato/);
      expect(containsJargon(text), text).toBe(false);
    }
  });
});

describe("simpleSummary", () => {
  it("states explicitly that no findings is not a guarantee", () => {
    const lines = simpleSummary([], [coverage("cpu", "evaluated")]);
    expect(lines[0]).toBe("Non abbiamo trovato problemi.");
    expect(lines).toContain(simpleNotAGuarantee());
    expect(simpleNotAGuarantee()).toBe("Nessun problema rilevato non significa che sia tutto perfetto.");
  });

  it("keeps the honesty note when only informational findings exist", () => {
    const lines = simpleSummary([finding("a", { severity: "info" })], []);
    expect(lines).toContain(simpleNotAGuarantee());
  });

  it("counts important and to-check items with correct plurals", () => {
    const lines = simpleSummary(
      [
        finding("a", { severity: "critical" }),
        finding("b", { severity: "warning" }),
        finding("c", { severity: "warning" }),
        finding("d", { severity: "info" }),
      ],
      [],
    );
    expect(lines[0]).toBe("Abbiamo trovato 1 cosa importante e 2 cose da controllare.");
    expect(lines[1]).toBe("In più, 1 informazione da sapere.");
    expect(lines).not.toContain(simpleNotAGuarantee());
  });

  it("mentions parts that were not fully checked", () => {
    const lines = simpleSummary([], [coverage("cpu", "partial"), coverage("gpu", "notEvaluated")]);
    expect(lines.at(-1)).toMatch(/^2 parti del PC/);
  });

  it("contains no jargon", () => {
    for (const s of SEVERITIES) {
      for (const line of simpleSummary([finding("a", { severity: s })], [coverage("cpu", "partial")])) {
        expect(containsJargon(line), line).toBe(false);
      }
    }
    expect(containsJargon(simpleShortWindowNote())).toBe(false);
    for (const s of SEVERITIES) expect(containsJargon(simpleSeverityLabel(s))).toBe(false);
  });
});

describe("English", () => {
  afterEach(() => setLang("it"));

  it("writes the plain labels, coverage sentences and summary in English", () => {
    setLang("en");
    expect(simpleSeverityLabel("critical")).toBe("Important");
    expect(simpleKindHint("hypothesis")).toBe("Possible cause, not certain");
    expect(simpleComponentName("cpu")).toBe("Processor (CPU)");
    expect(simpleCoverageLabel("evaluated")).toBe("Fully checked");
    expect(simpleCoverageNote(coverage("drivers", "notEvaluated"))).toBe("the data needed is missing");
    expect(simpleNothingFound()).toBe("We didn't find any problems.");
    expect(simpleSummary([], [coverage("cpu", "evaluated")])).toEqual([
      "We didn't find any problems.",
      "Finding no problems doesn't mean everything is perfect.",
    ]);
    expect(
      simpleSummary(
        [finding("a", { severity: "critical" }), finding("b", { severity: "critical" }), finding("c", { severity: "info" })],
        [coverage("cpu", "partial")],
      ),
    ).toEqual([
      "We found 2 important things.",
      "Also, 1 thing worth knowing.",
      "One part of the PC was only partly checked or not checked: see below.",
    ]);
    expect(simpleSummary([finding("a", { severity: "critical" }), finding("b", { severity: "warning" })], [])[0]).toBe(
      "We found 1 important thing and 1 thing to check.",
    );
  });

  it("keeps the English copy free of jargon and of technical status words", () => {
    setLang("en");
    for (const s of STATUSES) {
      const text = `${simpleCoverageLabel(s)} ${simpleCoverageNote(coverage("cpu", s))}`;
      expect(text).not.toMatch(/Evaluated|Partial|Not evaluated/);
      expect(containsJargon(text), text).toBe(false);
    }
    expect(containsJargon(simpleShortWindowNote())).toBe(false);
  });
});
