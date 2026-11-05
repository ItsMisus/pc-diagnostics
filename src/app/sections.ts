import type { IconName } from "@/components/common/Icon";
import { tr } from "@/i18n";

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
}

export const SECTIONS: readonly Section[] = [
  { id: "overview", icon: "overview" },
  { id: "hardware", icon: "hardware" },
  { id: "benchmarks", icon: "benchmarks" },
  { id: "processes", icon: "processes" },
  { id: "drivers", icon: "drivers" },
  { id: "diagnostics", icon: "diagnostics" },
  { id: "history", icon: "history" },
  { id: "settings", icon: "settings" },
];

/** Translated navigation label (call at render time so it follows the language). */
export const sectionLabel = (id: SectionId): string => tr("shell", `section_${id}`);
