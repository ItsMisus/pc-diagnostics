import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import { containsJargon } from "@/lib/plain";
import type { Confidence } from "@/types/generated/Confidence";
import type { RemediationAction } from "@/types/generated/RemediationAction";
import type { SettingsPage } from "@/types/generated/SettingsPage";
import type { Severity } from "@/types/generated/Severity";
import type { StepRisk } from "@/types/generated/StepRisk";
import {
  actionStatesIrreversible,
  actionTechnicalLabel,
  assistantReadiness,
  canShowRisolvi,
  describeAction,
  formatCostEstimate,
  formatTokenUsage,
  isPreviewExpired,
  modelHint,
  modelLabel,
  modelOptionLabel,
  notReadyText,
  planConfidenceLabel,
  planConfidenceShort,
  settingsPageLabel,
  stepRiskLabel,
  stepRiskTone,
} from "./model";

/** One sample per action kind: the compiler fails here when a new kind is added. */
const ACTIONS: { [K in RemediationAction["kind"]]: Extract<RemediationAction, { kind: K }> } = {
  openSettingsPage: { kind: "openSettingsPage", page: "windowsUpdate" },
  openDeviceManager: { kind: "openDeviceManager" },
  openDiskCleanup: { kind: "openDiskCleanup", drive: "C:" },
  emptyRecycleBin: { kind: "emptyRecycleBin" },
  cleanTempFiles: { kind: "cleanTempFiles", olderThanDays: 7 },
  closeProgram: { kind: "closeProgram", pid: 4242, name: "chrome.exe" },
  runSystemFileCheck: { kind: "runSystemFileCheck" },
};

const PAGES: Record<SettingsPage, true> = {
  windowsUpdate: true,
  windowsUpdateOptional: true,
  storageSense: true,
  display: true,
  sound: true,
  bluetooth: true,
  network: true,
  power: true,
  troubleshoot: true,
  startupApps: true,
  installedApps: true,
  privacy: true,
};

const RISKS: Record<StepRisk, true> = { low: true, medium: true, high: true };
const CONFIDENCES: Record<Confidence, true> = { low: true, medium: true, high: true };

describe("describeAction", () => {
  it.each(Object.values(ACTIONS))("describes $kind in plain Italian without jargon", (action) => {
    const text = describeAction(action);
    expect(text.length).toBeGreaterThan(20);
    expect(containsJargon(text), text).toBe(false);
    expect(text).not.toContain(action.kind);
  });

  it("uses the agreed wording for the destructive and elevated actions", () => {
    expect(describeAction(ACTIONS.closeProgram)).toBe(
      "Chiederà a «chrome.exe» di chiudersi, come cliccare la X. Salva prima il tuo lavoro.",
    );
    expect(describeAction(ACTIONS.emptyRecycleBin)).toBe(
      "Svuoterà il Cestino. Windows ti chiederà conferma. Non si può annullare.",
    );
    expect(describeAction(ACTIONS.runSystemFileCheck)).toMatch(/^Windows ti chiederà il permesso di amministratore/);
  });

  it("includes the parameters the user must know about", () => {
    expect(describeAction(ACTIONS.openDiskCleanup)).toContain("C:");
    expect(describeAction(ACTIONS.cleanTempFiles)).toContain("7 giorni");
    expect(describeAction({ kind: "cleanTempFiles", olderThanDays: 1 })).toContain("1 giorno ");
    expect(describeAction(ACTIONS.cleanTempFiles)).toMatch(/Non si può annullare/);
    expect(describeAction(ACTIONS.openSettingsPage)).toContain("«Windows Update»");
  });

  it("describes every Settings page with its own label", () => {
    const pages = Object.keys(PAGES) as SettingsPage[];
    const labels = pages.map(settingsPageLabel);
    expect(new Set(labels).size).toBe(pages.length);
    for (const page of pages) {
      const text = describeAction({ kind: "openSettingsPage", page });
      expect(text).toContain(`«${settingsPageLabel(page)}»`);
      expect(containsJargon(text)).toBe(false);
    }
  });

  it("has a technical label per kind for the advanced view", () => {
    for (const action of Object.values(ACTIONS)) expect(actionTechnicalLabel(action)).toContain(action.kind);
    expect(actionTechnicalLabel(ACTIONS.closeProgram)).toContain("PID 4242");
  });
});

describe("risk and confidence labels", () => {
  it("labels risk as Basso / Medio / Alto with escalating tones", () => {
    expect((Object.keys(RISKS) as StepRisk[]).map(stepRiskLabel)).toEqual(["Rischio basso", "Rischio medio", "Rischio alto"]);
    expect(stepRiskTone("low")).toBe("neutral");
    expect(stepRiskTone("medium")).toBe("warn");
    expect(stepRiskTone("high")).toBe("bad");
  });

  it("states the model's confidence plainly, never as a certainty", () => {
    for (const c of Object.keys(CONFIDENCES) as Confidence[]) {
      const text = planConfidenceLabel(c);
      expect(text).toMatch(/^L'AI /);
      expect(text).not.toMatch(/certamente|sicuramente|garantit/i);
      expect(containsJargon(text)).toBe(false);
      expect(planConfidenceShort(c)).toMatch(/^Fiducia /);
    }
    expect(new Set((Object.keys(CONFIDENCES) as Confidence[]).map(planConfidenceLabel)).size).toBe(3);
  });
});

describe("model labels", () => {
  it("names the known models", () => {
    expect(modelLabel("claude-sonnet-5-5")).toBe("Claude Sonnet 5.5");
    expect(modelLabel("claude-opus-5-5")).toBe("Claude Opus 5.5");
    expect(modelLabel("claude-opus-4-5")).toBe("Claude Opus 4.5");
    expect(modelLabel("claude-haiku-5-5")).toBe("Claude Haiku 5.5");
  });

  it("marks only the recommended model and gives each known model a hint", () => {
    expect(modelOptionLabel("claude-sonnet-5-5")).toBe("Claude Sonnet 5.5 (consigliato)");
    expect(modelOptionLabel("claude-opus-5-5")).toBe("Claude Opus 5.5");
    expect(modelHint("claude-sonnet-5-5")).toBe("Buon equilibrio tra qualità e costo (~0,03 $ a richiesta)");
    expect(modelHint("claude-opus-5-5")).toBe("Per i problemi più difficili (~0,06–0,10 $)");
    expect(modelHint("claude-opus-4-5")).toBe("Generazione precedente (~0,10–0,18 $)");
    expect(modelHint("claude-haiku-5-5")).toBe("Il più economico, per problemi semplici (~0,002 $)");
  });

  it("falls back to a generic label for unknown ids", () => {
    expect(modelLabel("claude-sonnet-4-5-20250929")).toBe("Claude Sonnet 4.5");
    expect(modelLabel("claude-fable-6")).toBe("Claude Fable 6");
    expect(modelLabel("some-other-model")).toBe("some-other-model");
    expect(modelOptionLabel("claude-fable-6")).toBe("Claude Fable 6");
    expect(modelHint("claude-fable-6")).toBe("");
  });
});

describe("canShowRisolvi and readiness", () => {
  it("is offered for warning and critical findings only", () => {
    const expected: Record<Severity, boolean> = { info: false, warning: true, critical: true };
    for (const [severity, shown] of Object.entries(expected) as [Severity, boolean][]) {
      expect(canShowRisolvi({ severity })).toBe(shown);
    }
  });

  it("needs the switch on and a stored key", () => {
    expect(assistantReadiness({ enabled: false, keyConfigured: false })).toBe("disabled");
    expect(assistantReadiness({ enabled: false, keyConfigured: true })).toBe("disabled");
    expect(assistantReadiness({ enabled: true, keyConfigured: false })).toBe("noKey");
    expect(assistantReadiness({ enabled: true, keyConfigured: true })).toBe("ready");
  });

  it("explains how to enable it, says nothing was sent, without jargon", () => {
    for (const reason of ["disabled", "noKey"] as const) {
      const t = notReadyText(reason);
      expect(t.explanation).toMatch(/Nessun dato è stato inviato/);
      expect(t.steps.some((s) => s.includes("Settings"))).toBe(true);
      for (const text of [t.title, t.explanation, ...t.steps]) expect(containsJargon(text), text).toBe(false);
    }
    expect(notReadyText("disabled").steps.some((s) => /Attiva/.test(s))).toBe(true);
    expect(notReadyText("noKey").steps.some((s) => /Attiva/.test(s))).toBe(false);
  });
});

describe("numbers", () => {
  it("formats the cost estimate Italian style", () => {
    expect(formatCostEstimate(0.034)).toBe("≈ 0,03 $");
    expect(formatCostEstimate(0.1)).toBe("≈ 0,10 $");
    expect(formatCostEstimate(1.256)).toBe("≈ 1,26 $");
    expect(formatCostEstimate(0.002)).toBe("meno di 0,01 $");
    expect(formatCostEstimate(0)).toBe("0 $");
    expect(formatCostEstimate(Number.NaN)).toBe("—");
    expect(formatCostEstimate(-1)).toBe("—");
  });

  it("formats token usage with Italian thousands separators", () => {
    expect(formatTokenUsage({ inputTokens: 12345, outputTokens: 678 })).toBe("12.345 token inviati · 678 token ricevuti");
  });

  it("treats a preview as expired from its deadline on", () => {
    expect(isPreviewExpired(1000, 999)).toBe(false);
    expect(isPreviewExpired(1000, 1000)).toBe(true);
  });
});

describe("irreversibility note", () => {
  it("matches what the description already says, so the dialog never repeats or omits it", () => {
    for (const action of Object.values(ACTIONS)) {
      expect(actionStatesIrreversible(action), action.kind).toBe(/non si può annullare/i.test(describeAction(action)));
    }
  });
});

describe("English", () => {
  afterEach(() => setLang("it"));

  it("describes every action in English, with the same parameters and irreversibility notes", () => {
    setLang("en");
    for (const action of Object.values(ACTIONS)) {
      const text = describeAction(action);
      expect(text.length).toBeGreaterThan(20);
      expect(containsJargon(text), text).toBe(false);
      expect(text).not.toContain(action.kind);
      expect(actionStatesIrreversible(action), action.kind).toBe(/can't be undone/i.test(text));
    }
    expect(describeAction(ACTIONS.closeProgram)).toBe(
      "It will ask “chrome.exe” to close, like clicking the X. Save your work first.",
    );
    expect(describeAction(ACTIONS.cleanTempFiles)).toContain("older than 7 days");
    expect(describeAction({ kind: "cleanTempFiles", olderThanDays: 1 })).toContain("older than 1 day ");
    expect(describeAction(ACTIONS.openDiskCleanup)).toContain("drive C:");
    expect(describeAction(ACTIONS.openSettingsPage)).toContain("“Windows Update”");
    const pages = Object.keys(PAGES) as SettingsPage[];
    expect(new Set(pages.map((p) => settingsPageLabel(p))).size).toBe(pages.length);
  });

  it("translates labels, readiness texts and model hints", () => {
    setLang("en");
    expect((Object.keys(RISKS) as StepRisk[]).map((r) => stepRiskLabel(r))).toEqual(["Low risk", "Medium risk", "High risk"]);
    for (const c of Object.keys(CONFIDENCES) as Confidence[]) {
      expect(planConfidenceLabel(c)).toMatch(/^The AI /);
      expect(planConfidenceLabel(c)).not.toMatch(/certainly|surely|guarantee/i);
      expect(planConfidenceShort(c)).toMatch(/ confidence$/);
    }
    expect(modelOptionLabel("claude-sonnet-5-5")).toBe("Claude Sonnet 5.5 (recommended)");
    expect(modelHint("claude-haiku-5-5")).toBe("The cheapest, for simple problems (~$0.002)");
    expect(modelHint("claude-fable-6")).toBe("");
    for (const reason of ["disabled", "noKey"] as const) {
      const t = notReadyText(reason);
      expect(t.explanation).toMatch(/No data has been sent/);
      expect(t.steps.some((s) => s.includes("Settings"))).toBe(true);
    }
    expect(notReadyText("disabled").steps).toHaveLength(3);
    expect(notReadyText("noKey").steps.some((s) => /Turn on/.test(s))).toBe(false);
  });

  it("formats cost and tokens US style", () => {
    setLang("en");
    expect(formatCostEstimate(0.034)).toBe("≈ $0.03");
    expect(formatCostEstimate(1.256)).toBe("≈ $1.26");
    expect(formatCostEstimate(0.002)).toBe("less than $0.01");
    expect(formatCostEstimate(0)).toBe("$0");
    expect(formatCostEstimate(-1)).toBe("—");
    expect(formatTokenUsage({ inputTokens: 12345, outputTokens: 678 })).toBe("12,345 tokens sent · 678 tokens received");
  });
});
