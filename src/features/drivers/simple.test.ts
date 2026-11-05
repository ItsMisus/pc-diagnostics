import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import { containsJargon } from "@/lib/plain";
import type { DeviceHealth } from "@/types/generated/DeviceHealth";
import type { UpdateStatus } from "@/types/generated/UpdateStatus";
import { healthLabel, updateLabel } from "./model";
import {
  actionText,
  deviceProblemExplanation,
  KNOWN_PROBLEM_CODES,
  problemCodeExplanation,
  simpleDeviceName,
  simpleDriverSummary,
  simpleHealthLabel,
  simpleNoInstallNote,
  simpleUpdateLabel,
} from "./simple";
import { device, EMPTY_SUMMARY } from "./testing/fixtures";

const REQUIRED_CODES = [1, 3, 10, 12, 14, 18, 19, 21, 22, 24, 28, 31, 32, 37, 39, 41, 43, 45, 47, 48, 52, 54];
const HEALTHS: DeviceHealth[] = ["working", "problemDetected", "driverMissing", "disabled"];
const UPDATES: UpdateStatus[] = [
  { kind: "notChecked" },
  { kind: "notVerifiable", reason: "r" },
  { kind: "verified", source: "windowsUpdate", detail: "d" },
  { kind: "updateAvailable", source: "windowsUpdate", title: "t", version: null, date: null, hardwareId: "PCI\\VEN" },
];

/** Advice the app must never give a non-technical user. */
const FORBIDDEN = /regedit|registro di sistema|BIOS|UEFI|firmware|disattiva(re)? (windows|il servizio|la firma)|acquista|compra|sostituisci/i;

describe("problemCodeExplanation", () => {
  it("has a dedicated explanation for every common code", () => {
    const generic = problemCodeExplanation(null);
    for (const code of REQUIRED_CODES) {
      expect(KNOWN_PROBLEM_CODES).toContain(code);
      expect(problemCodeExplanation(code), `codice ${code}`).not.toBe(generic);
    }
  });

  it("falls back to a generic explanation for unknown or missing codes", () => {
    const generic = problemCodeExplanation(null);
    expect(generic.title).toBe("Windows segnala un problema con questo dispositivo");
    expect(generic.actions).toEqual([actionText("restart"), actionText("reconnect"), actionText("update")]);
    expect(problemCodeExplanation(9999)).toBe(generic);
    expect(problemCodeExplanation(2)).toBe(generic);
  });

  it("suggests a restart for code 14 and a driver update for a missing driver", () => {
    expect(problemCodeExplanation(14).actions).toEqual([actionText("restart")]);
    expect(problemCodeExplanation(28).actions[0]).toBe(actionText("update"));
  });

  it("only gives safe advice, in plain words, with at least one action", () => {
    for (const code of [...REQUIRED_CODES, null]) {
      const e = problemCodeExplanation(code);
      expect(e.actions.length).toBeGreaterThan(0);
      for (const text of [e.title, e.meaning, ...e.actions]) {
        expect(containsJargon(text), text).toBe(false);
        expect(text, text).not.toMatch(FORBIDDEN);
        expect(text).not.toMatch(/codice \d/i);
      }
    }
    expect(containsJargon(simpleNoInstallNote())).toBe(false);
  });
});

describe("deviceProblemExplanation", () => {
  it("is null for a working device without a code", () => {
    expect(deviceProblemExplanation(device("ok"))).toBeNull();
  });

  it("uses the problem code when present", () => {
    expect(deviceProblemExplanation(device("x", { health: "problemDetected", problemCode: 43 }))).toBe(
      problemCodeExplanation(43),
    );
  });

  it("explains a missing driver without a code, and falls back for an uncoded problem", () => {
    expect(deviceProblemExplanation(device("x", { health: "driverMissing" }))).toBe(problemCodeExplanation(28));
    expect(deviceProblemExplanation(device("x", { health: "problemDetected" }))).toBe(problemCodeExplanation(null));
    expect(deviceProblemExplanation(device("x", { health: "disabled" }))).toBe(problemCodeExplanation(22));
  });
});

describe("simple labels", () => {
  it("keep the tones of the technical labels", () => {
    for (const h of HEALTHS) expect(simpleHealthLabel(h).tone).toBe(healthLabel(h).tone);
    for (const u of UPDATES) expect(simpleUpdateLabel(u).tone).toBe(updateLabel(u).tone);
  });

  it("are plain words", () => {
    expect(simpleHealthLabel("working").label).toBe("Funziona");
    expect(simpleHealthLabel("driverMissing").label).toBe("Manca il driver");
    for (const h of HEALTHS) expect(containsJargon(simpleHealthLabel(h).label)).toBe(false);
  });
});

describe("simpleDriverSummary", () => {
  it("never claims drivers are up to date when nothing is wrong", () => {
    const lines = simpleDriverSummary({ ...EMPTY_SUMMARY, total: 12, working: 12 });
    expect(lines[0]).toBe("Windows non segnala problemi su nessuno dei 12 dispositivi collegati.");
    expect(lines[1]).toMatch(/non significa che tutti i driver siano aggiornati/);
  });

  it("counts problems and missing drivers together, with plurals", () => {
    expect(simpleDriverSummary({ ...EMPTY_SUMMARY, total: 10, problems: 1 })[0]).toMatch(/^1 dispositivo su 10 ha/);
    expect(simpleDriverSummary({ ...EMPTY_SUMMARY, total: 10, problems: 1, driverMissing: 2 })[0]).toMatch(
      /^3 dispositivi su 10 hanno/,
    );
  });

  it("mentions updates and disabled devices without alarm", () => {
    const lines = simpleDriverSummary({ ...EMPTY_SUMMARY, total: 5, working: 4, disabled: 1, updatesAvailable: 2 });
    expect(lines.some((l) => l.includes("2 aggiornamenti"))).toBe(true);
    expect(lines.some((l) => l.includes("non è un problema"))).toBe(true);
    for (const l of lines) expect(containsJargon(l)).toBe(false);
  });

  it("handles an empty inventory", () => {
    expect(simpleDriverSummary(EMPTY_SUMMARY)).toEqual(["Windows non ha riportato dispositivi collegati."]);
  });
});

describe("simpleDeviceName", () => {
  it("replaces hardware ids with Device Manager wording", () => {
    expect(simpleDeviceName({ name: "ACPI\\AMDI0204\\2&DABA3FF&0", instanceId: "ACPI\\AMDI0204\\2&DABA3FF&0" })).toBe("Dispositivo sconosciuto");
    expect(simpleDeviceName({ name: "", instanceId: "X" })).toBe("Dispositivo sconosciuto");
    expect(simpleDeviceName({ name: "VB-Audio Virtual Cable", instanceId: "X" })).toBe("VB-Audio Virtual Cable");
  });
});

/** Technical tokens that must not reach English simple-view copy either. */
const EN_JARGON = ["PDH", "WMI", "HRESULT", "LUID", "ConfigManager", "Configuration Manager", "CM_PROB", "0x", "NTSTATUS", "WDDM"];
const EN_FORBIDDEN = /regedit|registry|BIOS|UEFI|firmware|disable (windows|the service|driver signing)|\bbuy\b|purchase|replace/i;
const hasEnJargon = (text: string) => EN_JARGON.some((j) => text.includes(j));

describe("English simple view", () => {
  afterEach(() => setLang("it"));

  it("explains every code in plain English, with safe advice only", () => {
    setLang("en");
    for (const code of [...REQUIRED_CODES, null]) {
      const e = problemCodeExplanation(code);
      for (const text of [e.title, e.meaning, ...e.actions]) {
        expect(hasEnJargon(text), text).toBe(false);
        expect(text, text).not.toMatch(EN_FORBIDDEN);
        expect(text).not.toMatch(/code \d/i);
        expect(text).not.toMatch(/[àèéìòù]/);
      }
    }
    expect(problemCodeExplanation(null).title).toBe("Windows reports a problem with this device");
    expect(problemCodeExplanation(14).actions).toEqual(["Restart the PC."]);
    expect(hasEnJargon(simpleNoInstallNote())).toBe(false);
  });

  it("keeps one object per code within a language and follows a language change", () => {
    setLang("en");
    const en = problemCodeExplanation(43);
    expect(problemCodeExplanation(43)).toBe(en);
    expect(deviceProblemExplanation(device("x", { health: "driverMissing" }))).toBe(problemCodeExplanation(28));
    setLang("it");
    expect(problemCodeExplanation(43)).not.toBe(en);
    expect(problemCodeExplanation(43).title).toBe("Il dispositivo ha segnalato un problema ed è stato fermato");
  });

  it("labels and summarizes in plain English, with plurals", () => {
    setLang("en");
    expect(simpleHealthLabel("working").label).toBe("Works");
    expect(simpleUpdateLabel({ kind: "notChecked" }).label).toBe("Not checked yet");
    for (const h of HEALTHS) expect(hasEnJargon(simpleHealthLabel(h).label)).toBe(false);
    expect(simpleDriverSummary({ ...EMPTY_SUMMARY, total: 12, working: 12 })).toEqual([
      "Windows reports no problems on any of the 12 connected devices.",
      "This doesn't mean that all drivers are on the latest version.",
    ]);
    expect(simpleDriverSummary({ ...EMPTY_SUMMARY, total: 1, working: 1 })[0]).toBe(
      "Windows reports no problems on the only connected device.",
    );
    expect(simpleDriverSummary({ ...EMPTY_SUMMARY, total: 10, problems: 1 })[0]).toMatch(/^1 device out of 10 needs/);
    expect(simpleDriverSummary({ ...EMPTY_SUMMARY, total: 10, problems: 1, driverMissing: 2 })[0]).toMatch(
      /^3 devices out of 10 need/,
    );
    const lines = simpleDriverSummary({ ...EMPTY_SUMMARY, total: 5, working: 4, disabled: 1, updatesAvailable: 2 });
    expect(lines.some((l) => l.includes("2 driver updates"))).toBe(true);
    expect(lines.some((l) => l.includes("1 device is turned off"))).toBe(true);
    for (const l of lines) expect(hasEnJargon(l)).toBe(false);
    expect(simpleDriverSummary(EMPTY_SUMMARY)).toEqual(["Windows didn't report any connected devices."]);
    expect(simpleDeviceName({ name: "", instanceId: "X" })).toBe("Unknown device");
  });
});
