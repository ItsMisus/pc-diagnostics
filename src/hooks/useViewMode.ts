import { useMemo } from "react";
import { useAppStore } from "@/app/store";
import { useT } from "@/i18n";
import type { ViewMode } from "@/types/generated/ViewMode";

/**
 * Global "Semplice | Avanzata" preference. Components read the mode only through these
 * hooks, never from `settings` directly, so the fallback (simple when settings are not
 * available) lives in one place.
 */
export function useViewMode(): ViewMode {
  return useAppStore((s) => s.viewMode);
}

export function useIsSimple(): boolean {
  return useAppStore((s) => s.viewMode === "simple");
}

/** Changes the mode and persists it with the other settings (`update_settings`). */
export function useSetViewMode(): (mode: ViewMode) => void {
  return useAppStore((s) => s.setViewMode);
}

/** "Semplice | Avanzata" options, translated. */
export function useViewModeOptions(): readonly { value: ViewMode; label: string }[] {
  const t = useT("common");
  return useMemo(
    () => [
      { value: "simple", label: t("viewModeSimple") },
      { value: "advanced", label: t("viewModeAdvanced") },
    ],
    [t],
  );
}
