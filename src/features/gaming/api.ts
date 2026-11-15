import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  call,
  isTauriRuntime,
  toAppError,
  type CommandName,
} from "@/services/ipc";
import type { GameArtKind } from "@/types/generated/GameArtKind";
import type { GameProfile } from "@/types/generated/GameProfile";
import type { GamingSessionEvent } from "@/types/generated/GamingSessionEvent";
import type { GamingState } from "@/types/generated/GamingState";

/** Commands of the gaming center (`src-tauri/src/commands/gaming.rs`). */
export type GamingCommand = Extract<
  CommandName,
  | "get_gaming_state"
  | "get_game_art"
  | "launch_game"
  | "add_manual_game"
  | "remove_manual_game"
  | "set_game_hidden"
  | "set_game_profile"
  | "set_game_mode"
  | "stop_game_session"
>;

const gamingCall = <T>(
  cmd: GamingCommand,
  args?: Record<string, unknown>,
): Promise<T> => call<T>(cmd, args);

/** Must match `models::control::gaming::GAMING_SESSION_EVENT`. */
export const GAMING_SESSION_EVENT = "gaming:session";

export const gamingApi = {
  getState: (refresh = false) =>
    gamingCall<GamingState>("get_gaming_state", { refresh }),
  getArt: (id: string, kind: GameArtKind) =>
    gamingCall<string | null>("get_game_art", { id, kind }),
  launch: (id: string, withProfile: boolean) =>
    gamingCall<GamingState>("launch_game", { id, withProfile }),
  addManual: (path: string) =>
    gamingCall<GamingState>("add_manual_game", { path }),
  removeManual: (id: string) =>
    gamingCall<GamingState>("remove_manual_game", { id }),
  setHidden: (id: string, hidden: boolean) =>
    gamingCall<GamingState>("set_game_hidden", { id, hidden }),
  setProfile: (profile: GameProfile) =>
    gamingCall<GamingState>("set_game_profile", { profile }),
  setGameMode: (enabled: boolean) =>
    gamingCall<GamingState>("set_game_mode", { enabled }),
  stopSession: () => gamingCall<GamingState>("stop_game_session"),
};

export async function onGamingSession(
  handler: (e: GamingSessionEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => {};
  return listen<GamingSessionEvent>(GAMING_SESSION_EVENT, (event) =>
    handler(event.payload),
  );
}

/**
 * Opens the native file picker (tauri-plugin-dialog, `dialog:allow-open` only) for an app
 * bundle on macOS or an executable on Windows. Returns the chosen path or null. The plugin's
 * JS package is a thin wrapper over this same `invoke`, so it is not a dependency.
 */
export async function pickGameFile(opts: {
  title: string;
  filterName: string;
  mac: boolean;
}): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  try {
    const picked = await invoke<string | string[] | null>(
      "plugin:dialog|open",
      {
        options: {
          title: opts.title,
          multiple: false,
          directory: false,
          ...(opts.mac ? { defaultPath: "/Applications" } : {}),
          filters: [
            { name: opts.filterName, extensions: [opts.mac ? "app" : "exe"] },
          ],
        },
      },
    );
    if (Array.isArray(picked)) return picked[0] ?? null;
    return picked ?? null;
  } catch (raw) {
    throw toAppError(raw);
  }
}
