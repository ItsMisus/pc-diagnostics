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
 *   Ctrl+1…8 on Windows; ⌘1…7 on macOS, where Drivers is absent (⌘5 is Diagnostics there).
 */
export function resolveShortcut(e: KeyLike, platform: Platform): ShortcutAction | null {
  if (!hasCommandModifier(e, platform)) return null;
  if (e.key.toLowerCase() === "b") return { kind: "toggleSidebar" };
  const n = Number(e.key);
  const target = Number.isInteger(n) && n >= 1 ? visibleSections(platform)[n - 1] : undefined;
  return target ? { kind: "section", id: target.id } : null;
}
