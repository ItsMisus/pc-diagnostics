import { call, type CommandName } from "@/services/ipc";
import type { CoolingProfileDraft } from "@/types/generated/CoolingProfileDraft";
import type { CoolingState } from "@/types/generated/CoolingState";

/** The cooling center's commands (src-tauri/src/commands/cooling.rs). */
export const COOLING_COMMANDS = [
  "get_cooling_state",
  "apply_cooling_profile",
  "save_cooling_profile",
  "delete_cooling_profile",
  "restore_cooling",
] as const satisfies readonly CommandName[];
type CoolingCommand = (typeof COOLING_COMMANDS)[number];

const invoke = <T>(
  cmd: CoolingCommand,
  args?: Record<string, unknown>,
): Promise<T> => call<T>(cmd, args);

export const coolingApi = {
  state: () => invoke<CoolingState>("get_cooling_state"),
  apply: (id: string) => invoke<CoolingState>("apply_cooling_profile", { id }),
  save: (draft: CoolingProfileDraft) =>
    invoke<CoolingState>("save_cooling_profile", { draft }),
  remove: (id: string) =>
    invoke<CoolingState>("delete_cooling_profile", { id }),
  restore: () => invoke<CoolingState>("restore_cooling"),
};
