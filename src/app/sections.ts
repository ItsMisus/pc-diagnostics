import type { IconName } from "@/components/common/Icon";
import { tr } from "@/i18n";
import type { Platform } from "@/types/generated/Platform";

export type SectionId =
  | "overview"
  | "hardware"
  | "benchmarks"
  | "processes"
  | "drivers"
  | "diagnostics"
  | "history"
  | "settings";

export interface Section {
  id: SectionId;
  icon: IconName;
  /** Platforms the section exists on; absent = every platform. */
  platforms?: readonly Platform[];
}

/** Every section, in sidebar order. Use `visibleSections` for what the current platform shows. */
export const SECTIONS: readonly Section[] = [
  { id: "overview", icon: "overview" },
  { id: "hardware", icon: "hardware" },
  { id: "benchmarks", icon: "benchmarks" },
  { id: "processes", icon: "processes" },
  // Device and driver inventory (SetupAPI / Configuration Manager) and the Windows Update
  // driver search: Windows services with no macOS counterpart the app could read.
  { id: "drivers", icon: "drivers", platforms: ["windows"] },
  { id: "diagnostics", icon: "diagnostics" },
  { id: "history", icon: "history" },
  { id: "settings", icon: "settings" },
];

export function isSectionAvailable(id: SectionId, platform: Platform): boolean {
  const s = SECTIONS.find((x) => x.id === id);
  return s !== undefined && (s.platforms === undefined || s.platforms.includes(platform));
}

const cache = new Map<Platform, readonly Section[]>();

/**
 * Sections shown on `platform`, in sidebar order. The keyboard shortcut of a section is its
 * position in this list (Ctrl+1…8 on Windows; ⌘1…7 on macOS, where Drivers is absent), so
 * the sidebar tooltips and the shortcuts always agree. Stable array per platform.
 */
export function visibleSections(platform: Platform): readonly Section[] {
  let list = cache.get(platform);
  if (!list) {
    list = SECTIONS.filter((s) => isSectionAvailable(s.id, platform));
    cache.set(platform, list);
  }
  return list;
}

/** Translated navigation label (call at render time so it follows the language). */
export const sectionLabel = (id: SectionId): string => tr("shell", `section_${id}`);
