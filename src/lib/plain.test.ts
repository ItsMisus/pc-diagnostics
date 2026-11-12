import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import type { ErrorCode } from "@/types/generated/ErrorCode";
import type { Unavailability } from "@/types/generated/Unavailability";
import { unavailabilityText } from "./format";
import { containsJargon, JARGON_TOKENS, reasonTexts, simpleError, unavailabilitySimpleText } from "./plain";

/** A Record keyed by the union: the compiler fails if a new value is not listed here. */
const ALL_CODES: Record<ErrorCode, true> = {
  invalidInput: true,
  notFound: true,
  permissionDenied: true,
  notSupported: true,
  platform: true,
  database: true,
  busy: true,
  cancelled: true,
  consentRequired: true,
  network: true,
  internal: true,
};
const ERROR_CODES = Object.keys(ALL_CODES) as ErrorCode[];

const REASONS: Record<Unavailability["kind"], Unavailability> = {
  notSupported: { kind: "notSupported" },
  providerMissing: { kind: "providerMissing", detail: "nvml.dll not found (NVML)" },
  permissionDenied: { kind: "permissionDenied" },
  notImplemented: { kind: "notImplemented" },
  failed: {
    kind: "failed",
    detail: { code: "platform", message: "PDH query failed HRESULT 0x800007D0", detail: null, retriable: true },
  },
  inconclusive: { kind: "inconclusive", detail: "the chassis type indicates desktop but an internal battery is present" },
};

describe("unavailabilitySimpleText", () => {
  it("has the agreed plain wording for every kind", () => {
    expect(unavailabilitySimpleText(REASONS.notSupported)).toBe("Questo PC non fornisce questo dato");
    expect(unavailabilitySimpleText(REASONS.providerMissing)).toBe("Manca un componente necessario per leggerlo");
    expect(unavailabilitySimpleText(REASONS.permissionDenied)).toBe("Serve avviare l'app come amministratore");
    expect(unavailabilitySimpleText(REASONS.notImplemented)).toBe("Non ancora supportato dall'app");
    expect(unavailabilitySimpleText(REASONS.failed)).toBe("Lettura non riuscita, riprova più tardi");
    expect(unavailabilitySimpleText(REASONS.inconclusive)).toBe(
      "Le informazioni del sistema non bastano per stabilirlo",
    );
  });

  it("never leaks the technical detail into the visible text", () => {
    for (const reason of Object.values(REASONS)) {
      const text = unavailabilitySimpleText(reason);
      expect(containsJargon(text), text).toBe(false);
      expect(text).not.toContain("nvml.dll");
    }
  });
});

describe("reasonTexts", () => {
  it("is unchanged in the advanced view", () => {
    for (const reason of Object.values(REASONS)) {
      expect(reasonTexts(reason, false)).toEqual({ text: unavailabilityText(reason), tooltip: unavailabilityText(reason) });
    }
  });

  it("shows the plain text and keeps the technical reason in the tooltip in the simple view", () => {
    const { text, tooltip } = reasonTexts(REASONS.providerMissing, true);
    expect(text).toBe(unavailabilitySimpleText(REASONS.providerMissing));
    expect(tooltip).toContain(text);
    expect(tooltip).toContain(unavailabilityText(REASONS.providerMissing));
  });
});

describe("simpleError", () => {
  it.each(ERROR_CODES)("has a friendly title, explanation and action for %s", (code) => {
    const e = simpleError(code);
    expect(e.title.length).toBeGreaterThan(0);
    expect(e.explanation.length).toBeGreaterThan(0);
    expect(e.action.length).toBeGreaterThan(0);
    for (const text of [e.title, e.explanation, e.action]) {
      expect(containsJargon(text), text).toBe(false);
      expect(text).not.toContain(code);
    }
  });

  it("points to the administrator restart for permission errors and to Settings for consent", () => {
    expect(simpleError("permissionDenied").action).toMatch(/amministratore/);
    expect(simpleError("consentRequired").action).toMatch(/Consenti verifiche online/);
    expect(simpleError("notSupported").explanation).toMatch(/non è un guasto/i);
  });
});

describe("containsJargon", () => {
  it("detects every listed token", () => {
    for (const t of JARGON_TOKENS) expect(containsJargon(`errore ${t} 42`)).toBe(true);
    expect(containsJargon("La scheda video non risponde")).toBe(false);
  });
});

/** Technical tokens that must not reach English simple-view copy either. */
const EN_JARGON = [...JARGON_TOKENS, "WMI", "ConfigManager", "Configuration Manager", "0x", "DLL", "dll"];
const hasEnJargon = (text: string) => EN_JARGON.some((j) => text.includes(j));

describe("English plain-language texts", () => {
  beforeEach(() => setLang("en"));
  afterEach(() => setLang("it"));

  it("explains every unavailability reason in plain English, keeping the detail in the tooltip", () => {
    expect(unavailabilitySimpleText(REASONS.notSupported)).toBe("This PC does not provide this information");
    expect(unavailabilitySimpleText(REASONS.permissionDenied)).toBe("The app needs to run as administrator");
    for (const reason of Object.values(REASONS)) {
      const text = unavailabilitySimpleText(reason);
      expect(hasEnJargon(text), text).toBe(false);
      expect(text).not.toMatch(/[àèéìòù]/);
      const { tooltip } = reasonTexts(reason, true);
      expect(tooltip).toContain(text);
      expect(tooltip).toContain(unavailabilityText(reason));
    }
    expect(reasonTexts(REASONS.failed, true).tooltip).toBe(
      "Couldn't read it, try again later. Technical detail: Read failed: PDH query failed HRESULT 0x800007D0",
    );
  });

  it.each(ERROR_CODES)("has a plain English title, explanation and action for %s", (code) => {
    const e = simpleError(code);
    for (const text of [e.title, e.explanation, e.action]) {
      expect(text.length).toBeGreaterThan(0);
      expect(hasEnJargon(text), text).toBe(false);
      expect(text).not.toContain(code);
      expect(text).not.toMatch(/[àèéìòù«»]/);
    }
  });

  it("points to the administrator restart and to the online-checks setting", () => {
    expect(simpleError("permissionDenied").action).toMatch(/Run as administrator/);
    expect(simpleError("consentRequired").action).toMatch(/Allow online checks/);
    expect(simpleError("notSupported").explanation).toMatch(/isn't a fault/i);
  });

  it("follows a language change at call time", () => {
    const en = simpleError("network").title;
    setLang("it");
    expect(simpleError("network").title).toBe("Connessione non riuscita");
    expect(en).toBe("Couldn't connect");
  });
});
