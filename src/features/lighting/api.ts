import { call, type CommandName } from "@/services/ipc";
import type { LightingSetup } from "@/types/generated/LightingSetup";
import type { LightingState } from "@/types/generated/LightingState";

/** The lighting center's commands (`src-tauri/src/commands/lighting.rs`). */
export const LIGHTING_COMMANDS = [
  "lighting_get_state",
  "lighting_refresh",
  "lighting_apply_profile",
  "lighting_apply_setup",
  "lighting_preview_setup",
  "lighting_set_brightness",
  "lighting_save_profile",
  "lighting_delete_profile",
  "lighting_restore",
] as const satisfies readonly CommandName[];

export type LightingCommand = (typeof LIGHTING_COMMANDS)[number];

const invoke = (cmd: LightingCommand, args?: Record<string, unknown>) => call<LightingState>(cmd, args);

export const lightingApi = {
  getState: () => invoke("lighting_get_state"),
  refresh: () => invoke("lighting_refresh"),
  applyProfile: (id: string) => invoke("lighting_apply_profile", { id }),
  applySetup: (setup: LightingSetup) => invoke("lighting_apply_setup", { setup }),
  previewSetup: (setup: LightingSetup) => invoke("lighting_preview_setup", { setup }),
  setBrightness: (brightness: number) => invoke("lighting_set_brightness", { brightness }),
  saveProfile: (id: string | null, name: string, setup: LightingSetup) => invoke("lighting_save_profile", { id, name, setup }),
  deleteProfile: (id: string) => invoke("lighting_delete_profile", { id }),
  restore: () => invoke("lighting_restore"),
};
