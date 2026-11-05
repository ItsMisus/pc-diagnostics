import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import type { DeviceHealth } from "@/types/generated/DeviceHealth";
import {
  categoryLabel,
  filterCounts,
  filterDevices,
  filterLabel,
  groupDevices,
  healthLabel,
  matchesFilter,
  summaryCards,
  updateLabel,
  updateSourceLabel,
} from "./model";
import { device, EMPTY_SUMMARY } from "./testing/fixtures";

const update = { kind: "updateAvailable", source: "windowsUpdate", title: "Intel - Display - 31.0", version: "31.0", date: null, hardwareId: "PCI\\VEN_8086" } as const;

describe("healthLabel", () => {
  it("maps every health state to an explicit label and tone", () => {
    expect(healthLabel("working")).toEqual({ label: "Funzionante", tone: "ok" });
    expect(healthLabel("problemDetected")).toEqual({ label: "Problema rilevato", tone: "bad" });
    expect(healthLabel("driverMissing")).toEqual({ label: "Driver assente o non associato", tone: "bad" });
    expect(healthLabel("disabled")).toEqual({ label: "Disabilitato", tone: "neutral" });
  });
});

describe("updateLabel", () => {
  it("maps every update status", () => {
    expect(updateLabel(update)).toEqual({ label: "Aggiornamento disponibile", tone: "warn" });
    expect(updateLabel({ kind: "verified", source: "windowsUpdate", detail: "x" })).toEqual({ label: "Versione verificata", tone: "ok" });
    expect(updateLabel({ kind: "notVerifiable", reason: "r" })).toEqual({ label: "Non verificabile", tone: "neutral" });
    expect(updateLabel({ kind: "notChecked" })).toEqual({ label: "Non verificato", tone: "neutral" });
    expect(updateSourceLabel("windowsUpdate")).toBe("Windows Update");
  });
});

describe("device filters", () => {
  const healths: DeviceHealth[] = ["working", "problemDetected", "driverMissing", "disabled"];
  const [ok, problem, missing, disabled] = healths.map((health) => device(health, { health }));
  const withUpdate = device("GPU Intel", { category: "gpu", update });
  const unverifiable = device("Sensore", { update: { kind: "notVerifiable", reason: "Nessun ID hardware" } });
  const inbox = device("Tastiera HID", {
    category: "input",
    driver: { version: "10.0", date: "2006-06-21", provider: "Microsoft", infName: "keyboard.inf", signed: { status: "available", value: true, unit: "none", source: "configManager", at: 0 }, signer: "Microsoft Windows", isInbox: true },
  });
  const all = [ok!, problem!, missing!, disabled!, withUpdate, unverifiable, inbox];

  it("problems = problemDetected + driverMissing; disabled and inbox drivers are not problems", () => {
    expect(all.filter((d) => matchesFilter(d, "problems")).map((d) => d.name)).toEqual(["problemDetected", "driverMissing"]);
    expect(matchesFilter(inbox, "problems")).toBe(false);
    expect(matchesFilter(disabled!, "problems")).toBe(false);
  });

  it("counts chips over the full list", () => {
    expect(filterCounts(all)).toEqual({ all: 7, problems: 2, updates: 1, notVerifiable: 1 });
  });

  it("combines chip, category and case-insensitive name search", () => {
    expect(filterDevices(all, { filter: "all", category: null, search: "  gpu " }).map((d) => d.name)).toEqual(["GPU Intel"]);
    expect(filterDevices(all, { filter: "updates", category: "input", search: "" })).toEqual([]);
    expect(filterDevices(all, { filter: "all", category: "input", search: "" }).map((d) => d.name)).toEqual(["Tastiera HID"]);
    expect(filterDevices(all, { filter: "notVerifiable", category: null, search: "" }).map((d) => d.name)).toEqual(["Sensore"]);
  });

  it("groups in a fixed category order, attention first within a group", () => {
    const groups = groupDevices([inbox, device("Zeta"), problem!, withUpdate]);
    expect(groups.map((g) => g.category)).toEqual(["gpu", "input", "other"]);
    expect(groups[2]?.devices.map((d) => d.name)).toEqual(["problemDetected", "Zeta"]);
  });
});

describe("summaryCards", () => {
  it("derives the five cards from the backend summary", () => {
    const cards = summaryCards({
      ...EMPTY_SUMMARY,
      total: 120,
      working: 110,
      problems: 2,
      driverMissing: 1,
      disabled: 3,
      updatesAvailable: 4,
      notVerifiable: 5,
      notChecked: 6,
    });
    expect(cards.map((c) => [c.id, c.value])).toEqual([
      ["total", 120],
      ["working", 110],
      ["problems", 3],
      ["updates", 4],
      ["unverified", 11],
    ]);
    expect(cards.find((c) => c.id === "problems")?.tone).toBe("bad");
    expect(cards.find((c) => c.id === "updates")?.tone).toBe("warn");
    expect(cards.find((c) => c.id === "working")?.tone).toBe("neutral");
  });

  it("uses neutral tones when nothing needs attention", () => {
    expect(summaryCards(EMPTY_SUMMARY).every((c) => c.tone === "neutral")).toBe(true);
  });
});

describe("English labels", () => {
  afterEach(() => setLang("it"));

  it("labels states, categories, filters and cards in English, with the same tones", () => {
    setLang("en");
    expect(healthLabel("working")).toEqual({ label: "Working", tone: "ok" });
    expect(updateLabel(update)).toEqual({ label: "Update available", tone: "warn" });
    expect(categoryLabel("gpu")).toBe("Graphics cards");
    expect(filterLabel("notVerifiable")).toBe("Not verifiable");
    expect(updateSourceLabel("windowsUpdate")).toBe("Windows Update");
    const cards = summaryCards({ ...EMPTY_SUMMARY, problems: 2, driverMissing: 1 });
    expect(cards.find((c) => c.id === "problems")?.hint).toBe("2 with a detected problem, 1 without an associated driver");
    const groups = groupDevices([device("Zeta"), device("alpha"), device("GPU", { category: "gpu" })]);
    expect(groups.map((g) => g.label)).toEqual(["Graphics cards", "Other devices"]);
    expect(groups[1]?.devices.map((d) => d.name)).toEqual(["alpha", "Zeta"]);
    expect(filterDevices([device("Zeta")], { filter: "all", category: null, search: "ZET" })).toHaveLength(1);
  });
});
