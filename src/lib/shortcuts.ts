import type { Platform } from "@/types/generated/Platform";

/**
 * App keyboard shortcuts use the platform's command modifier: Cmd (⌘) on macOS, Ctrl
 * elsewhere. Exactly that modifier must be held: Alt, Shift and the other one of Ctrl/Cmd
 * must not be, so system and browser combinations (Ctrl+Cmd+F full screen, Ctrl+1 desktop
 * switching on macOS, Ctrl+Shift+…) are never taken over.
 */

type ModifierState = Pick<KeyboardEvent, "ctrlKey" | "metaKey" | "altKey" | "shiftKey">;

export function hasCommandModifier(e: ModifierState, platform: Platform): boolean {
  if (e.altKey || e.shiftKey) return false;
  return platform === "macos" ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/** Shortcut as shown in tooltips: "⌘B" on macOS (Apple's notation), "Ctrl+B" elsewhere. */
export function shortcutLabel(key: string, platform: Platform): string {
  const k = key.toUpperCase();
  return platform === "macos" ? `⌘${k}` : `Ctrl+${k}`;
}
