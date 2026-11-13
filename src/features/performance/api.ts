import { call, type CommandName } from "@/services/ipc";
import type { PerformanceProfileInput } from "@/types/generated/PerformanceProfileInput";
import type { PerformanceState } from "@/types/generated/PerformanceState";

/** The performance center's commands (src-tauri/src/commands/performance.rs). */
type PerformanceCommand =
  | "get_performance_state"
  | "apply_performance_profile"
  | "save_performance_profile"
  | "delete_performance_profile"
  | "restore_performance_original"
  | "open_power_settings";

// The shared `CommandName` union lives in services/ipc.ts (orchestrator-owned); the names are
// checked here against our own list and widened once.
const run = <T>(cmd: PerformanceCommand, args?: Record<string, unknown>) => call<T>(cmd as CommandName, args);

export const performanceApi = {
  getState: () => run<PerformanceState>("get_performance_state"),
  applyProfile: (profileId: string) => run<PerformanceState>("apply_performance_profile", { profileId }),
  saveProfile: (profile: PerformanceProfileInput) => run<PerformanceState>("save_performance_profile", { profile }),
  deleteProfile: (profileId: string) => run<PerformanceState>("delete_performance_profile", { profileId }),
  restoreOriginal: () => run<PerformanceState>("restore_performance_original"),
  openPowerSettings: () => run<void>("open_power_settings"),
};

export type PerformanceApi = typeof performanceApi;
