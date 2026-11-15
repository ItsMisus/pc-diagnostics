import { describe, expect, it } from "vitest";
import type { CoolingFan } from "@/types/generated/CoolingFan";
import type { CoolingProfile } from "@/types/generated/CoolingProfile";
import type { CoolingState } from "@/types/generated/CoolingState";
import { COOLING_COMMANDS } from "./api";
import { PRESETS } from "./curve";
import {
  ALL_FANS,
  applyBlockReason,
  curveOf,
  draftFromProfile,
  focalModel,
  galleryTypes,
  nameIssue,
  newDraft,
  policyFor,
  profileName,
  profileOptions,
  rotorSpeed,
  spinRpm,
  toProfileDraft,
  withPolicy,
} from "./model";

const unsupported = (reason: string) => ({ state: "unsupported" as const, reason });
const supported = { state: "supported" as const, reason: null };

const profile = (id: string, mode: CoolingProfile["mode"], builtin = true): CoolingProfile => ({
  id,
  name: id,
  builtin,
  mode,
  curves: mode === "automatic" ? [] : [{ fanId: null, curve: { points: PRESETS.balanced } }],
  coolingPolicy: [],
});

/** What the backend returns on this fanless MacBook Air. */
const macAir: CoolingState = {
  system: { kind: "passive", description: "Raffreddamento passivo" },
  fans: [],
  capabilities: { fanSpeed: unsupported("Nessuna ventola."), curves: unsupported("Serve il controllo ventole."), coolingPolicy: unsupported("Impostazione di Windows.") },
  currentPolicy: [],
  profiles: [
    { profile: profile("auto", "automatic"), applicable: true, note: "Il sistema regola già il raffreddamento da solo." },
    { profile: profile("silent", "silent"), applicable: false, note: "Curva salvata, ma non applicabile." },
    { profile: profile("balanced", "balanced"), applicable: false, note: "Curva salvata, ma non applicabile." },
    { profile: { ...profile("user-1", "custom", false), name: "Notte" }, applicable: false, note: "Curva salvata, ma non applicabile." },
  ],
  selected: null,
  gameOverride: null,
  active: "auto",
  canRestore: false,
  cpuTemperatureC: 41,
};

const fan = (currentRpm: number | null): CoolingFan => ({
  id: "smc-0",
  label: "Ventola 1",
  currentRpm,
  minRpm: 1000,
  maxRpm: 5000,
  targetRpm: null,
  rpmReason: null,
  mode: "auto",
  speedControl: { state: "readOnly", reason: "root" },
});

describe("focal model", () => {
  it("shows the passive chassis on a fanless Mac and the other types in the gallery", () => {
    const m = focalModel(macAir);
    expect(m).toEqual({ kind: "passive" });
    expect(galleryTypes(m)).toEqual(["fan", "tower", "aio"]);
  });

  it("shows a fan only when fans were reported, spinning only with a real reading", () => {
    expect(focalModel({ system: { kind: "fans", description: "" }, fans: [fan(1800)] })).toEqual({ kind: "fan", rpm: 1800 });
    expect(focalModel({ system: { kind: "fans", description: "" }, fans: [fan(null)] })).toEqual({ kind: "fan", rpm: null });
    expect(focalModel({ system: { kind: "unknown", description: "" }, fans: [] })).toEqual({ kind: "unknown" });
    expect(galleryTypes({ kind: "unknown" })).toHaveLength(4);
    expect(spinRpm(fan(0))).toBeNull();
    expect(spinRpm(undefined)).toBeNull();
  });

  it("spins proportionally, never with reduced motion or without RPM", () => {
    expect(rotorSpeed(2000, false)).toBeCloseTo(2 * rotorSpeed(1000, false));
    expect(rotorSpeed(2000, true)).toBe(0);
    expect(rotorSpeed(null, false)).toBe(0);
    expect(rotorSpeed(9000, false)).toBe(rotorSpeed(6000, false));
  });
});

describe("profile tiles", () => {
  it("disables what cannot be applied, with the backend's reason", () => {
    const opts = profileOptions(macAir, (m) => `hint-${m}`);
    expect(opts.map((o) => [o.id, o.disabled])).toEqual([
      ["auto", false],
      ["silent", true],
      ["balanced", true],
      ["user-1", true],
    ]);
    expect(opts[0]?.hint).toBe("hint-automatic");
    expect(opts[1]?.disabledReason).toBe("Curva salvata, ma non applicabile.");
    expect(opts[0]).not.toHaveProperty("disabledReason");
    expect(profileName(macAir, "user-1")).toBe("Notte");
    expect(profileName(macAir, null)).toBeNull();
  });
});

describe("drafts", () => {
  it("round-trips a profile and drops curves of fans that are gone", () => {
    const p: CoolingProfile = {
      ...profile("user-2", "custom", false),
      name: "Gioco",
      curves: [
        { fanId: null, curve: { points: PRESETS.silent } },
        { fanId: "smc-0", curve: { points: PRESETS.aggressive } },
        { fanId: "old", curve: { points: PRESETS.balanced } },
      ],
      coolingPolicy: [{ supply: "battery", policy: "passive" }],
    };
    const d = draftFromProfile(p);
    expect(d.id).toBe("user-2");
    expect(curveOf(d, "smc-0")).toEqual(PRESETS.aggressive);
    expect(curveOf(d, "smc-1")).toEqual(PRESETS.silent);
    const out = toProfileDraft({ ...d, name: "  Gioco " }, ["smc-0"]);
    expect(out.name).toBe("Gioco");
    expect(out.curves.map((c) => c.fanId)).toEqual([null, "smc-0"]);
    expect(out.coolingPolicy).toEqual([{ supply: "battery", policy: "passive" }]);
  });

  it("starts a built-in as a new draft", () => {
    const d = draftFromProfile(profile("silent", "silent"));
    expect(d.id).toBeNull();
    expect(d.name).toBe("");
    expect(newDraft().curves[ALL_FANS]).toEqual(PRESETS.balanced);
  });

  it("edits the policy per supply", () => {
    let list = withPolicy([], "battery", "passive");
    list = withPolicy(list, "ac", "active");
    expect(list).toEqual([
      { supply: "ac", policy: "active" },
      { supply: "battery", policy: "passive" },
    ]);
    expect(policyFor(list, "ac")).toBe("active");
    expect(policyFor(withPolicy(list, "ac", null), "ac")).toBeNull();
  });

  it("validates names like the backend", () => {
    expect(nameIssue("  ")).toBe("required");
    expect(nameIssue("x".repeat(41))).toBe("tooLong");
    expect(nameIssue(" Notte ")).toBeNull();
  });

  it("blocks apply where curves cannot be applied, unless a Windows policy can be", () => {
    expect(applyBlockReason(macAir.capabilities, newDraft())).toBe("Serve il controllo ventole.");
    const win = { ...macAir.capabilities, coolingPolicy: supported };
    expect(applyBlockReason(win, newDraft())).not.toBeNull();
    expect(applyBlockReason(win, { ...newDraft(), policy: [{ supply: "ac", policy: "passive" }] })).toBeNull();
  });
});

describe("api", () => {
  it("lists the commands registered in src-tauri", () => {
    expect(COOLING_COMMANDS).toEqual(["get_cooling_state", "apply_cooling_profile", "save_cooling_profile", "delete_cooling_profile", "restore_cooling"]);
  });
});
