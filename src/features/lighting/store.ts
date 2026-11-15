import { create } from "zustand";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { LightingColor } from "@/types/generated/LightingColor";
import type { LightingDevice } from "@/types/generated/LightingDevice";
import type { LightingEffect } from "@/types/generated/LightingEffect";
import type { LightingProfile } from "@/types/generated/LightingProfile";
import type { LightingSetup } from "@/types/generated/LightingSetup";
import type { LightingState } from "@/types/generated/LightingState";
import { lightingApi } from "./api";
import { pushRecent } from "./colorMath";
import { monochromeCompatible } from "./effect";

export type EffectKind = LightingEffect["kind"];

const AZURE: LightingColor = { r: 78, g: 165, b: 255 };
const PALETTE: LightingColor[] = [{ r: 255, g: 59, b: 107 }, { r: 255, g: 179, b: 64 }, AZURE];

/** The default parameters of each effect when the user switches to it, keeping the colors in use. */
export function effectOfKind(kind: EffectKind, from: LightingEffect): LightingEffect {
  const colors = effectColorsForSwitch(from);
  const one = colors[0] ?? AZURE;
  const many = colors.length >= 2 ? colors : PALETTE;
  const speed = "speed" in from ? from.speed : 4;
  switch (kind) {
    case "off":
      return { kind };
    case "static":
      return { kind, color: one };
    case "breathing":
      return { kind, color: one, period: from.kind === "breathing" ? from.period : 4000 };
    case "rainbow":
      return { kind, speed };
    case "wave":
      return { kind, colors: many, speed, direction: from.kind === "wave" ? from.direction : "leftToRight" };
    case "gradient":
      return { kind, colors: many };
    case "colorCycle":
      return { kind, colors: many, speed };
  }
}

function effectColorsForSwitch(e: LightingEffect): LightingColor[] {
  switch (e.kind) {
    case "static":
    case "breathing":
      return [e.color];
    case "wave":
    case "gradient":
    case "colorCycle":
      return e.colors;
    default:
      return [];
  }
}

export const DEFAULT_SETUP: LightingSetup = { effect: { kind: "static", color: AZURE }, brightness: 80, targets: { kind: "all" }, sync: true };

export function setupOf(p: LightingProfile): LightingSetup {
  return { effect: p.effect, brightness: p.brightness, targets: p.targets, sync: p.sync };
}

export function sameSetup(a: LightingSetup, b: LightingSetup): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** Controllable devices the setup reaches and that can show its effect. */
export function reachedDevices(devices: readonly LightingDevice[], setup: LightingSetup): LightingDevice[] {
  return devices.filter(
    (d) =>
      (d.brightness.state === "supported" || d.color.state === "supported") &&
      (setup.targets.kind === "all" || setup.targets.ids.includes(d.id)) &&
      (d.colorCapable || monochromeCompatible(setup.effect)),
  );
}

export function controllable(d: LightingDevice): boolean {
  return d.brightness.state === "supported" || d.color.state === "supported";
}

const RECENT_KEY = "lighting.recentColors";

function loadRecent(): LightingColor[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY);
    const list = raw ? (JSON.parse(raw) as unknown) : [];
    return Array.isArray(list) ? (list.filter((c) => c && typeof c.r === "number" && typeof c.g === "number" && typeof c.b === "number") as LightingColor[]) : [];
  } catch {
    return [];
  }
}

type Status = "idle" | "loading" | "ready" | "error";

interface LightingStore {
  status: Status;
  state: LightingState | null;
  loadError: AppError | null;
  /** Last failed action (shown inline). */
  actionError: AppError | null;
  busy: "apply" | "refresh" | "save" | "delete" | "restore" | "brightness" | null;
  /** The setup being edited (what the preview shows). */
  draft: LightingSetup;
  /** Profile the draft came from (selection on the page). */
  profileId: string | null;
  recent: LightingColor[];
  load: () => Promise<void>;
  refresh: () => Promise<void>;
  setDraft: (patch: Partial<LightingSetup>) => void;
  setEffect: (effect: LightingEffect) => void;
  pickProfile: (id: string) => void;
  apply: () => Promise<void>;
  turnOff: () => Promise<void>;
  setBrightnessLive: (brightness: number) => Promise<void>;
  save: (name: string, asNew: boolean) => Promise<boolean>;
  remove: (id: string) => Promise<void>;
  restore: () => Promise<void>;
  rememberColor: (c: LightingColor) => void;
  clearError: () => void;
}

export const useLighting = create<LightingStore>((set, get) => {
  /** Runs an action that returns the new state. */
  const run = async (busy: NonNullable<LightingStore["busy"]>, fn: () => Promise<LightingState>): Promise<boolean> => {
    set({ busy, actionError: null });
    try {
      const state = await fn();
      set({ state, busy: null });
      return true;
    } catch (e) {
      set({ busy: null, actionError: toAppError(e) });
      return false;
    }
  };

  return {
    status: "idle",
    state: null,
    loadError: null,
    actionError: null,
    busy: null,
    draft: DEFAULT_SETUP,
    profileId: null,
    recent: loadRecent(),

    load: async () => {
      if (get().status === "loading") return;
      set({ status: get().state ? "ready" : "loading", loadError: null });
      try {
        const state = await lightingApi.getState();
        // First load: edit what is on, else the selected profile, else the default.
        const first = get().state === null;
        const patch: Partial<LightingStore> = { state, status: "ready" };
        if (first) {
          const selected = state.profiles.find((p) => p.id === state.selected);
          if (state.applied) {
            patch.draft = state.applied.setup;
            patch.profileId = state.applied.profileId;
          } else if (selected) {
            patch.draft = setupOf(selected);
            patch.profileId = selected.id;
          }
        }
        set(patch);
      } catch (e) {
        set({ status: "error", loadError: toAppError(e) });
      }
    },

    refresh: async () => {
      await run("refresh", lightingApi.refresh);
    },

    setDraft: (patch) => set((s) => ({ draft: { ...s.draft, ...patch } })),
    setEffect: (effect) => set((s) => ({ draft: { ...s.draft, effect } })),

    pickProfile: (id) => {
      const p = get().state?.profiles.find((x) => x.id === id);
      if (p) set({ draft: setupOf(p), profileId: p.id, actionError: null });
    },

    apply: async () => {
      const { draft, profileId, state } = get();
      const profile = state?.profiles.find((p) => p.id === profileId);
      if (profile && sameSetup(setupOf(profile), draft)) {
        await run("apply", () => lightingApi.applyProfile(profile.id));
      } else {
        await run("apply", () => lightingApi.applySetup(draft));
      }
    },

    turnOff: async () => {
      const off = get().state?.profiles.find((p) => p.id === "builtin.off");
      if (!off) return;
      set({ draft: setupOf(off), profileId: off.id });
      await run("apply", () => lightingApi.applyProfile(off.id));
    },

    setBrightnessLive: async (brightness) => {
      set((s) => ({ draft: { ...s.draft, brightness } }));
      if (get().state?.applied) await run("brightness", () => lightingApi.setBrightness(brightness));
    },

    save: async (name, asNew) => {
      const { profileId, state, draft } = get();
      const own = state?.profiles.find((p) => p.id === profileId && !p.builtin);
      const ok = await run("save", () => lightingApi.saveProfile(asNew || !own ? null : own.id, name, draft));
      if (ok) set({ profileId: get().state?.selected ?? null });
      return ok;
    },

    remove: async (id) => {
      const ok = await run("delete", () => lightingApi.deleteProfile(id));
      if (ok && get().profileId === id) set({ profileId: null });
    },

    restore: async () => {
      await run("restore", lightingApi.restore);
    },

    rememberColor: (c) => {
      const recent = pushRecent(get().recent, c);
      set({ recent });
      try {
        localStorage.setItem(RECENT_KEY, JSON.stringify(recent));
      } catch {
        // Per-viewer convenience only.
      }
    },

    clearError: () => set({ actionError: null }),
  };
});
