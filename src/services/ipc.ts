import { invoke } from "@tauri-apps/api/core";
import type { AppError } from "@/types/generated/AppError";
import type { ErrorCode } from "@/types/generated/ErrorCode";

/** Every backend command name. Keep in sync with `generate_handler!` in src-tauri/src/lib.rs. */
export type CommandName =
  | "get_app_info"
  | "get_system_info"
  | "get_settings"
  | "get_language"
  | "update_settings"
  | "get_hardware_inventory"
  | "get_live_history"
  | "get_monitor_status"
  | "get_app_overhead"
  | "get_process_snapshot"
  | "get_process_history"
  | "get_driver_inventory"
  | "check_driver_updates"
  | "get_benchmark_capabilities"
  | "start_benchmark"
  | "cancel_benchmark"
  | "get_benchmark_status"
  | "get_benchmark_history"
  | "compare_benchmarks"
  | "run_diagnostics"
  | "get_latest_diagnostics"
  | "export_report"
  | "get_assistant_status"
  | "set_assistant_api_key"
  | "clear_assistant_api_key"
  | "preview_fix"
  | "request_fix"
  | "execute_fix_step"
  // Performance center
  | "get_performance_state"
  | "apply_performance_profile"
  | "save_performance_profile"
  | "delete_performance_profile"
  | "restore_performance_original"
  | "open_power_settings"
  // Cooling center
  | "get_cooling_state"
  | "apply_cooling_profile"
  | "save_cooling_profile"
  | "delete_cooling_profile"
  | "restore_cooling"
  // Lighting center
  | "lighting_get_state"
  | "lighting_refresh"
  | "lighting_apply_profile"
  | "lighting_apply_setup"
  | "lighting_preview_setup"
  | "lighting_set_brightness"
  | "lighting_save_profile"
  | "lighting_delete_profile"
  | "lighting_restore"
  // Gaming center
  | "get_gaming_state"
  | "get_game_art"
  | "launch_game"
  | "add_manual_game"
  | "remove_manual_game"
  | "set_game_hidden"
  | "set_game_profile"
  | "set_game_mode"
  | "stop_game_session";

/** Keyed by the union so the compiler fails when a new `ErrorCode` is not listed. */
const ERROR_CODE_SET: Record<ErrorCode, true> = {
  invalidInput: true,
  notFound: true,
  permissionDenied: true,
  notSupported: true,
  platform: true,
  database: true,
  busy: true,
  cancelled: true,
  consentRequired: true,
  network: true,
  internal: true,
};
const ERROR_CODES = Object.keys(ERROR_CODE_SET) as readonly ErrorCode[];

export function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.code === "string" &&
    (ERROR_CODES as readonly string[]).includes(v.code) &&
    typeof v.message === "string" &&
    typeof v.retriable === "boolean"
  );
}

/** Normalizes anything thrown by `invoke` (or our own code) into an `AppError`. */
export function toAppError(raw: unknown): AppError {
  if (isAppError(raw)) return { ...raw, detail: raw.detail ?? null };
  const detail =
    raw instanceof Error
      ? raw.message
      : typeof raw === "string"
        ? raw
        : JSON.stringify(raw);
  return {
    code: "internal",
    message: "Unexpected IPC failure",
    detail,
    retriable: false,
  };
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Invokes a backend command. Rejects only with an `AppError`. */
export async function call<T>(
  cmd: CommandName,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauriRuntime()) {
    const error: AppError = {
      code: "notSupported",
      message:
        "Tauri runtime not available: the UI is running outside the desktop app",
      detail: null,
      retriable: false,
    };
    throw error;
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (raw) {
    throw toAppError(raw);
  }
}
