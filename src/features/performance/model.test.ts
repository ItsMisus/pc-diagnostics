import { describe, expect, it } from "vitest";
import type { PerformanceState } from "@/types/generated/PerformanceState";
import type { PerformanceSupplySettings } from "@/types/generated/PerformanceSupplySettings";
import {
  anyAdvancedSupported,
  controlList,
  CONTROLS,
  coreLook,
  copySupply,
  displayedMode,
  draftErrors,
  editableSupplies,
  effectiveProfileId,
  minutesToSeconds,
  newDraft,
  parseField,
  profileIcon,
  secondsToMinutes,
  toInput,
  updateSupply,
} from "./model";

const plain = (mode: PerformanceSupplySettings["mode"]): PerformanceSupplySettings => ({
  mode,
  keepAwake: "off",
  processorMaxPercent: null,
  processorMinPercent: null,
  boostMode: null,
  displayOffSeconds: null,
  sleepSeconds: null,
});

const supported = { state: "supported", reason: null } as const;
const readOnly = { state: "readOnly", reason: "Impostazioni di Sistema" } as const;
const unsupported = { state: "unsupported", reason: "macOS" } as const;

function macState(over: Partial<PerformanceState> = {}): PerformanceState {
  return {
    capabilities: {
      mode: readOnly,
      keepAwake: supported,
      processorState: unsupported,
      boostMode: unsupported,
      displayTimeout: readOnly,
      sleepTimeout: readOnly,
    },
    os: {
      ac: { mode: "balanced", processorMaxPercent: null, processorMinPercent: null, boostMode: null, displayOffSeconds: 600, sleepSeconds: 60 },
      battery: { mode: "silent", processorMaxPercent: null, processorMinPercent: null, boostMode: null, displayOffSeconds: 600, sleepSeconds: 60 },
      schemeName: null,
      keepAwake: "off",
    },
    supply: "battery",
    hasBattery: true,
    profiles: [
      { id: "silent", name: "Silenzioso", builtin: true, ac: plain("silent"), battery: plain("silent") },
      { id: "balanced", name: "Bilanciato", builtin: true, ac: plain("balanced"), battery: plain("balanced") },
      { id: "performance", name: "Prestazioni elevate", builtin: true, ac: { ...plain("highPerformance"), keepAwake: "system" }, battery: plain("highPerformance") },
      { id: "custom-1", name: "Notte", builtin: false, ac: plain("silent"), battery: plain("silent") },
    ],
    selectedProfileId: null,
    gameOverride: null,
    canRestore: false,
    issues: [],
    ...over,
  };
}

const windowsCaps = {
  mode: supported,
  keepAwake: supported,
  processorState: supported,
  boostMode: supported,
  displayTimeout: supported,
  sleepTimeout: supported,
};

describe("performance model", () => {
  it("lists every control with its support: available, then unsupported, then read-only", () => {
    const list = controlList(macState().capabilities);
    expect(list.map((c) => c.control).sort()).toEqual([...CONTROLS].sort());
    const ranks = list.map((c) => ({ supported: 0, unsupported: 1, readOnly: 2 })[c.support.state]);
    expect(ranks).toEqual([...ranks].sort((a, b) => a - b));
    expect(list[0]?.support.state).toBe("supported");
    expect(controlList(windowsCaps).map((c) => c.control)).toEqual([...CONTROLS]);
    expect(anyAdvancedSupported(macState().capabilities)).toBe(false);
    expect(anyAdvancedSupported(windowsCaps)).toBe(true);
  });

  it("picks glyphs for built-in and user profiles", () => {
    expect(profileIcon({ id: "silent", builtin: true })).toBe("leaf");
    expect(profileIcon({ id: "performance", builtin: true })).toBe("bolt");
    expect(profileIcon({ id: "custom-1", builtin: false })).toBe("sliders");
  });

  it("puts the game override over the selection", () => {
    const s = macState({ selectedProfileId: "silent", gameOverride: { profileId: "performance", profileName: "Prestazioni elevate" } });
    expect(effectiveProfileId(s)).toBe("performance");
    expect(effectiveProfileId(macState({ selectedProfileId: "silent" }))).toBe("silent");
    expect(effectiveProfileId(macState())).toBeNull();
  });

  it("shows the system's mode where the app cannot set it", () => {
    // macOS: the performance profile is selected, but Low Power Mode is what runs on battery.
    const s = macState({ selectedProfileId: "performance" });
    expect(displayedMode(s)).toEqual({ mode: "silent", source: "system" });
    expect(displayedMode({ ...s, supply: "ac" })).toEqual({ mode: "balanced", source: "system" });
  });

  it("shows the profile's mode for the current supply where the app sets it", () => {
    const s = macState({ capabilities: windowsCaps, selectedProfileId: "performance", supply: "ac" });
    expect(displayedMode(s)).toEqual({ mode: "highPerformance", source: "profile" });
    expect(displayedMode({ ...s, selectedProfileId: null })).toEqual({ mode: "balanced", source: "system" });
  });

  it("makes the core calm for silent and vivid for high performance", () => {
    expect(coreLook("silent").glow).toBeLessThan(coreLook("balanced").glow);
    expect(coreLook("balanced").speed).toBeLessThan(coreLook("highPerformance").speed);
    expect(coreLook(null).speed).toBe(0);
  });

  it("edits one supply at a time and copies between them", () => {
    let d = newDraft("Nuovo profilo");
    d = updateSupply(d, "battery", { mode: "silent", sleepSeconds: 300 });
    expect(d.ac.mode).toBe("balanced");
    expect(d.battery).toMatchObject({ mode: "silent", sleepSeconds: 300 });
    const copied = copySupply(d, "battery");
    expect(copied.ac).toEqual(copied.battery);
    expect(copied.ac).not.toBe(copied.battery);
  });

  it("starts a new profile from the one in effect", () => {
    const base = macState().profiles[2]!;
    const d = newDraft("Copia", base);
    expect(d.id).toBeNull();
    expect(d.ac.keepAwake).toBe("system");
    d.ac.keepAwake = "off";
    expect(base.ac.keepAwake).toBe("system");
  });

  it("mirrors the mains settings without a battery", () => {
    const d = updateSupply(newDraft("  Scrivania "), "ac", { mode: "highPerformance" });
    expect(editableSupplies(false)).toEqual(["ac"]);
    const input = toInput(d, false);
    expect(input.name).toBe("Scrivania");
    expect(input.battery.mode).toBe("highPerformance");
    expect(toInput(d, true).battery.mode).toBe("balanced");
  });

  it("validates the draft like the backend", () => {
    expect(draftErrors(newDraft(" "), true)).toEqual(["nameEmpty"]);
    expect(draftErrors(newDraft("x".repeat(41)), true)).toEqual(["nameLong"]);
    const order = updateSupply(newDraft("A"), "battery", { processorMaxPercent: 50, processorMinPercent: 60 });
    expect(draftErrors(order, true)).toEqual(["processorOrder"]);
    // The battery side does not count on a desktop.
    expect(draftErrors(order, false)).toEqual([]);
    expect(draftErrors(updateSupply(newDraft("A"), "ac", { processorMaxPercent: 3 }), true)).toEqual(["processorRange"]);
    expect(draftErrors(updateSupply(newDraft("A"), "ac", { sleepSeconds: 301 * 60 }), true)).toEqual(["timeoutRange"]);
  });

  it("converts timeouts and parses number fields", () => {
    expect(secondsToMinutes(null)).toBeNull();
    expect(secondsToMinutes(600)).toBe(10);
    expect(minutesToSeconds(15)).toBe(900);
    expect(minutesToSeconds(null)).toBeNull();
    expect(parseField("")).toBeNull();
    expect(parseField(" 80 ")).toBe(80);
    expect(parseField("7,6")).toBe(8);
    expect(parseField("abc")).toBeNull();
  });
});
