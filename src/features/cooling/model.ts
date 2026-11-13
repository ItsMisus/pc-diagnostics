import type { IconName } from "@/components/common/Icon";
import type { CoolingCurvePoint } from "@/types/generated/CoolingCurvePoint";
import type { CoolingFan } from "@/types/generated/CoolingFan";
import type { CoolingMode } from "@/types/generated/CoolingMode";
import type { CoolingPolicy } from "@/types/generated/CoolingPolicy";
import type { CoolingPolicySetting } from "@/types/generated/CoolingPolicySetting";
import type { CoolingProfile } from "@/types/generated/CoolingProfile";
import type { CoolingProfileDraft } from "@/types/generated/CoolingProfileDraft";
import type { CoolingState } from "@/types/generated/CoolingState";
import type { PowerSupply } from "@/types/generated/PowerSupply";
import type { Support } from "@/types/generated/Support";
import { PRESETS, toCurve } from "./curve";

/** The cooling types the page can draw. */
export type CoolingType = "fan" | "tower" | "aio" | "passive";
export const COOLING_TYPES: readonly CoolingType[] = ["fan", "tower", "aio", "passive"];

/** What the focal stage shows for this computer: only what the system reported. */
export type FocalModel = { kind: "passive" } | { kind: "fan"; rpm: number | null } | { kind: "unknown" };

export function focalModel(state: Pick<CoolingState, "system" | "fans">): FocalModel {
  if (state.system.kind === "passive") return { kind: "passive" };
  if (state.system.kind === "fans" && state.fans.length > 0) return { kind: "fan", rpm: spinRpm(state.fans[0]) };
  return { kind: "unknown" };
}

/** The type the focal stage stands for (it is left out of the "other types" gallery). */
export function presentType(model: FocalModel): CoolingType | null {
  return model.kind === "passive" ? "passive" : model.kind === "fan" ? "fan" : null;
}

/** Gallery types: every type except the one this computer has. */
export const galleryTypes = (model: FocalModel): CoolingType[] => COOLING_TYPES.filter((t) => t !== presentType(model));

/** A real, positive measured speed; null when unknown or stopped (the 3D fan does not spin). */
export function spinRpm(fan: CoolingFan | undefined): number | null {
  const rpm = fan?.currentRpm;
  return typeof rpm === "number" && Number.isFinite(rpm) && rpm > 0 ? rpm : null;
}

/**
 * Rotation speed of the 3D rotor in radians per second for a measured RPM, slowed down so a
 * real 1 200 rpm (20 rev/s, which would alias into a strobe) reads as "fast" but smooth:
 * proportional to the RPM, capped. Zero without a real reading or with reduced motion.
 */
export function rotorSpeed(rpm: number | null, reducedMotion: boolean): number {
  if (reducedMotion || rpm === null || rpm <= 0) return 0;
  return Math.min(rpm, 6000) * ((2 * Math.PI) / 60) * 0.12;
}

export const MODE_ICON: Record<CoolingMode, IconName> = {
  automatic: "sparkle",
  silent: "moon",
  balanced: "gauge",
  custom: "sliders",
};

/** Tiles for the profile switcher: a profile that cannot be applied here is disabled with the backend's reason. */
export function profileOptions(state: Pick<CoolingState, "profiles">, hint: (mode: CoolingMode) => string) {
  return state.profiles.map((e) => ({
    id: e.profile.id,
    name: e.profile.name,
    hint: e.applicable ? (e.note && e.profile.mode !== "automatic" ? e.note : hint(e.profile.mode)) : hint(e.profile.mode),
    icon: MODE_ICON[e.profile.mode],
    disabled: !e.applicable,
    ...(e.applicable || !e.note ? {} : { disabledReason: e.note }),
    builtin: e.profile.builtin,
  }));
}

export const userProfiles = (state: Pick<CoolingState, "profiles">): CoolingProfile[] => state.profiles.filter((e) => !e.profile.builtin).map((e) => e.profile);

/** The cooling policy for `supply` in a list, or null ("as the system"). */
export const policyFor = (list: readonly CoolingPolicySetting[], supply: PowerSupply): CoolingPolicy | null => list.find((s) => s.supply === supply)?.policy ?? null;

/** Replaces (or removes, with null) the policy of one supply. */
export function withPolicy(list: readonly CoolingPolicySetting[], supply: PowerSupply, policy: CoolingPolicy | null): CoolingPolicySetting[] {
  const rest = list.filter((s) => s.supply !== supply);
  const next = policy ? [...rest, { supply, policy }] : rest;
  return next.sort((a, b) => (a.supply === b.supply ? 0 : a.supply === "ac" ? -1 : 1));
}

/** Editor draft: name, curves per target ("*" = every fan) and the policy per supply. */
export interface Draft {
  id: string | null;
  name: string;
  curves: Record<string, CoolingCurvePoint[]>;
  policy: CoolingPolicySetting[];
}

export const ALL_FANS = "*";

export function newDraft(): Draft {
  return { id: null, name: "", curves: { [ALL_FANS]: PRESETS.balanced.map((p) => ({ ...p })) }, policy: [] };
}

export function draftFromProfile(p: CoolingProfile): Draft {
  const curves: Record<string, CoolingCurvePoint[]> = {};
  for (const c of p.curves) curves[c.fanId ?? ALL_FANS] = c.curve.points.map((q) => ({ ...q }));
  if (!curves[ALL_FANS]) curves[ALL_FANS] = PRESETS.balanced.map((q) => ({ ...q }));
  return { id: p.builtin ? null : p.id, name: p.builtin ? "" : p.name, curves, policy: [...p.coolingPolicy] };
}

/** The points edited for `target`, falling back to the all-fans curve. */
export const curveOf = (d: Draft, target: string): CoolingCurvePoint[] => d.curves[target] ?? d.curves[ALL_FANS] ?? [];

export function toProfileDraft(d: Draft, fanIds: readonly string[]): CoolingProfileDraft {
  const curves = Object.entries(d.curves)
    .filter(([k]) => k === ALL_FANS || fanIds.includes(k))
    .map(([k, points]) => ({ fanId: k === ALL_FANS ? null : k, curve: toCurve(points) }));
  return { id: d.id, name: d.name.trim(), curves, coolingPolicy: d.policy };
}

export const NAME_MAX = 40;

export function nameIssue(name: string): "required" | "tooLong" | null {
  const n = name.trim();
  if (n.length === 0) return "required";
  if ([...n].length > NAME_MAX) return "tooLong";
  return null;
}

/**
 * Whether "Salva e applica" can act: curves need fan control, a policy needs the Windows
 * setting. Null when it can, otherwise the reason to show.
 */
export function applyBlockReason(caps: CoolingState["capabilities"], d: Draft): string | null {
  if (caps.curves.state === "supported") return null;
  if (caps.coolingPolicy.state === "supported" && d.policy.length > 0) return null;
  return caps.curves.reason ?? caps.fanSpeed.reason ?? "";
}

/** Order of the capability rows. */
export const CAPABILITY_KEYS = ["fanSpeed", "curves", "coolingPolicy"] as const;
export type CapabilityKey = (typeof CAPABILITY_KEYS)[number];
export const capability = (caps: CoolingState["capabilities"], key: CapabilityKey): Support => caps[key];

/** Name of the profile with `id` (for the override strip and toasts). */
export const profileName = (state: Pick<CoolingState, "profiles">, id: string | null): string | null =>
  id === null ? null : (state.profiles.find((e) => e.profile.id === id)?.profile.name ?? null);
