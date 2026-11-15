import type { IconName } from "@/components/common/Icon";
import { bySupport } from "@/components/control/support";
import type { PerformanceBoostMode } from "@/types/generated/PerformanceBoostMode";
import type { PerformanceCapabilities } from "@/types/generated/PerformanceCapabilities";
import type { PerformanceControl } from "@/types/generated/PerformanceControl";
import type { PerformanceMode } from "@/types/generated/PerformanceMode";
import type { PerformanceOsSupply } from "@/types/generated/PerformanceOsSupply";
import type { PerformanceProfile } from "@/types/generated/PerformanceProfile";
import type { PerformanceProfileInput } from "@/types/generated/PerformanceProfileInput";
import type { PerformanceState } from "@/types/generated/PerformanceState";
import type { PerformanceSupplySettings } from "@/types/generated/PerformanceSupplySettings";
import type { PowerSupply } from "@/types/generated/PowerSupply";
import type { Support } from "@/types/generated/Support";

/**
 * Pure logic of the Performance page: which profile is in effect, which mode the stage shows,
 * the capability list, and the profile editor's draft (per supply). No React, no IPC.
 */

export const MODES: readonly PerformanceMode[] = ["silent", "balanced", "highPerformance"];

/** Built-in profile id → glyph; user profiles use `sliders`. */
const BUILTIN_ICON: Record<string, IconName> = { silent: "leaf", balanced: "balance", performance: "bolt" };
export const MODE_ICON: Record<PerformanceMode, IconName> = { silent: "leaf", balanced: "balance", highPerformance: "bolt" };

export function profileIcon(p: Pick<PerformanceProfile, "id" | "builtin">): IconName {
  return (p.builtin && BUILTIN_ICON[p.id]) || "sliders";
}

/** Same longest name as the backend (`models::control::PROFILE_NAME_MAX_CHARS`). */
export const PROFILE_NAME_MAX = 40;
export const PROCESSOR_MAX_FLOOR = 5;
/** 5 hours, like `PERFORMANCE_TIMEOUT_MAX_SECONDS`. */
export const TIMEOUT_MAX_MINUTES = 300;

/** Boost values offered in the editor (the others are only shown when Windows reports them). */
export const EDITABLE_BOOST: readonly PerformanceBoostMode[] = ["disabled", "enabled", "aggressive", "efficientEnabled", "efficientAggressive"];

/** Controls in the order of the "what you can control" list. */
export const CONTROLS: readonly PerformanceControl[] = ["mode", "keepAwake", "processorState", "boostMode", "displayTimeout", "sleepTimeout"];

const CAP_KEY: Record<PerformanceControl, keyof PerformanceCapabilities> = {
  mode: "mode",
  keepAwake: "keepAwake",
  processorState: "processorState",
  boostMode: "boostMode",
  displayTimeout: "displayTimeout",
  sleepTimeout: "sleepTimeout",
};

export function supportOf(caps: PerformanceCapabilities, control: PerformanceControl): Support {
  return caps[CAP_KEY[control]];
}

/** Every control with its support, grouped available → unsupported → read-only (stable within a group). */
export function controlList(caps: PerformanceCapabilities): { control: PerformanceControl; support: Support }[] {
  return bySupport(
    CONTROLS.map((control) => ({ control, support: supportOf(caps, control) })),
    (c) => c.support,
  );
}

/** The advanced (Windows) controls: the editor shows their group only when one is usable. */
export const ADVANCED: readonly PerformanceControl[] = ["processorState", "boostMode", "displayTimeout", "sleepTimeout"];

export function anyAdvancedSupported(caps: PerformanceCapabilities): boolean {
  return ADVANCED.some((c) => supportOf(caps, c).state === "supported");
}

/** The profile in effect: the game override, else the user's selection. */
export function effectiveProfileId(state: PerformanceState): string | null {
  return state.gameOverride?.profileId ?? state.selectedProfileId;
}

export function findProfile(state: PerformanceState, id: string | null): PerformanceProfile | null {
  return id === null ? null : (state.profiles.find((p) => p.id === id) ?? null);
}

export function settingsFor(p: Pick<PerformanceProfile, "ac" | "battery">, supply: PowerSupply): PerformanceSupplySettings {
  return supply === "battery" ? p.battery : p.ac;
}

export function osSupply(state: PerformanceState, supply: PowerSupply): PerformanceOsSupply | null {
  return supply === "battery" ? state.os.battery : state.os.ac;
}

/** Mirrors the backend: a profile applies when one of its settings is usable here. */
export function profileApplicable(caps: PerformanceCapabilities, p: Pick<PerformanceProfile, "ac" | "battery">): boolean {
  const ok = (c: PerformanceControl) => supportOf(caps, c).state === "supported";
  if (ok("mode") || ok("keepAwake")) return true;
  return [p.ac, p.battery].some(
    (s) =>
      ((s.processorMaxPercent !== null || s.processorMinPercent !== null) && ok("processorState")) ||
      (s.boostMode !== null && ok("boostMode")) ||
      (s.displayOffSeconds !== null && ok("displayTimeout")) ||
      (s.sleepSeconds !== null && ok("sleepTimeout")),
  );
}

/** Supplies a profile has settings for on this computer (a desktop has only mains power). */
export function editableSupplies(hasBattery: boolean): PowerSupply[] {
  return hasBattery ? ["ac", "battery"] : ["ac"];
}

/**
 * The mode the stage shows, and where it comes from. Where the app can set the mode, the
 * effective profile's mode is what the OS runs. Where it cannot (macOS), the stage shows what
 * the system reports, never a mode the app did not set.
 */
export function displayedMode(state: PerformanceState): { mode: PerformanceMode | null; source: "profile" | "system" } {
  const profile = findProfile(state, effectiveProfileId(state));
  if (profile && state.capabilities.mode.state === "supported") {
    return { mode: settingsFor(profile, state.supply).mode, source: "profile" };
  }
  return { mode: osSupply(state, state.supply)?.mode ?? null, source: "system" };
}

/** Visual parameters of the power core per mode (glow 0–1, flow speed, color temperature). */
export interface CoreLook {
  glow: number;
  speed: number;
  /** 0 = cool (blue-white), 1 = warm (amber-white). */
  warmth: number;
}

export const CORE_LOOK: Record<PerformanceMode | "unknown", CoreLook> = {
  silent: { glow: 0.28, speed: 0.25, warmth: 0 },
  balanced: { glow: 0.55, speed: 0.6, warmth: 0.35 },
  highPerformance: { glow: 1, speed: 1.4, warmth: 0.85 },
  unknown: { glow: 0.12, speed: 0, warmth: 0.2 },
};

export function coreLook(mode: PerformanceMode | null): CoreLook {
  return CORE_LOOK[mode ?? "unknown"];
}

// ------------------------------------------------------------------ Editor draft

export type ProfileDraft = PerformanceProfileInput;

/** A new profile starts from `base` (the profile in effect, if any) or from Balanced. */
export function newDraft(name: string, base?: Pick<PerformanceProfile, "ac" | "battery"> | null): ProfileDraft {
  const plain: PerformanceSupplySettings = {
    mode: "balanced",
    keepAwake: "off",
    processorMaxPercent: null,
    processorMinPercent: null,
    boostMode: null,
    displayOffSeconds: null,
    sleepSeconds: null,
  };
  return { id: null, name, ac: { ...(base?.ac ?? plain) }, battery: { ...(base?.battery ?? plain) } };
}

export function draftOf(p: PerformanceProfile): ProfileDraft {
  return { id: p.id, name: p.name, ac: { ...p.ac }, battery: { ...p.battery } };
}

/** Changes one supply of the draft (immutably). */
export function updateSupply(draft: ProfileDraft, supply: PowerSupply, patch: Partial<PerformanceSupplySettings>): ProfileDraft {
  return supply === "battery" ? { ...draft, battery: { ...draft.battery, ...patch } } : { ...draft, ac: { ...draft.ac, ...patch } };
}

/** Copies one supply's settings to the other. */
export function copySupply(draft: ProfileDraft, from: PowerSupply): ProfileDraft {
  return from === "ac" ? { ...draft, battery: { ...draft.ac } } : { ...draft, ac: { ...draft.battery } };
}

/** Without a battery only the mains settings matter: the battery side mirrors them on save. */
export function toInput(draft: ProfileDraft, hasBattery: boolean): PerformanceProfileInput {
  return { ...draft, name: draft.name.trim(), battery: hasBattery ? draft.battery : { ...draft.ac } };
}

export type DraftError = "nameEmpty" | "nameLong" | "processorRange" | "processorOrder" | "timeoutRange";

function supplyErrors(s: PerformanceSupplySettings): DraftError[] {
  const errors: DraftError[] = [];
  const max = s.processorMaxPercent;
  const min = s.processorMinPercent;
  if ((max !== null && (max < PROCESSOR_MAX_FLOOR || max > 100)) || (min !== null && (min < 0 || min > 100))) errors.push("processorRange");
  if (max !== null && min !== null && min > max) errors.push("processorOrder");
  for (const t of [s.displayOffSeconds, s.sleepSeconds]) {
    if (t !== null && (t < 0 || t > TIMEOUT_MAX_MINUTES * 60)) {
      errors.push("timeoutRange");
      break;
    }
  }
  return errors;
}

/** Problems that block saving (the backend checks the same rules). */
export function draftErrors(draft: ProfileDraft, hasBattery: boolean): DraftError[] {
  const name = draft.name.trim();
  const errors = new Set<DraftError>();
  if (name.length === 0) errors.add("nameEmpty");
  if ([...name].length > PROFILE_NAME_MAX) errors.add("nameLong");
  for (const s of editableSupplies(hasBattery).map((supply) => settingsFor(draft, supply))) {
    for (const e of supplyErrors(s)) errors.add(e);
  }
  return [...errors];
}

/** Minutes shown in the editor; null = leave as is. */
export function secondsToMinutes(seconds: number | null): number | null {
  return seconds === null ? null : Math.round(seconds / 60);
}

export function minutesToSeconds(minutes: number | null): number | null {
  return minutes === null || !Number.isFinite(minutes) ? null : Math.max(0, Math.round(minutes)) * 60;
}

/** Parses a number field: empty = null (leave as is). */
export function parseField(text: string): number | null {
  const t = text.trim();
  if (t === "") return null;
  const n = Number(t.replace(",", "."));
  return Number.isFinite(n) ? Math.round(n) : null;
}
