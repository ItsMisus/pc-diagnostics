import { hasCommandModifier } from "@/lib/shortcuts";
import type { Platform } from "@/types/generated/Platform";
import { type SectionId, visibleSections } from "./sections";

export type ShortcutAction = { kind: "toggleSidebar" } | { kind: "section"; id: SectionId };

type KeyLike = Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">;

/**
 * The app action bound to a key press, or null. With the platform's command modifier
 * (Ctrl on Windows, ⌘ on macOS; `lib/shortcuts.ts`):
 * - B toggles the sidebar;
 * - 1…n opens the n-th section of the sidebar as the platform shows it (`visibleSections`):
 *   Ctrl+1…9 / ⌘1…9; the order differs because Drivers is absent on macOS.
 */
export function resolveShortcut(e: KeyLike, platform: Platform): ShortcutAction | null {
  if (!hasCommandModifier(e, platform)) return null;
  if (e.key.toLowerCase() === "b") return { kind: "toggleSidebar" };
  if (e.key === ",") return { kind: "section", id: "settings" };
  const n = Number(e.key);
  const target = Number.isInteger(n) && n >= 1 ? visibleSections(platform)[n - 1] : undefined;
  return target ? { kind: "section", id: target.id } : null;
}

/**
 * The key that opens a section with the command modifier, as the sidebar tooltips show it:
 * the position for the first nine, "," for Settings (the platform convention), none otherwise.
 */
export function sectionShortcutKey(id: SectionId, index: number): string | null {
  if (index < 9) return String(index + 1);
  return id === "settings" ? "," : null;
}
