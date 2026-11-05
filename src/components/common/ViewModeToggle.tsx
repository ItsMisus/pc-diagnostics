import { useSetViewMode, useViewMode, useViewModeOptions } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { Segmented } from "./Segmented";

/** Compact "Semplice | Avanzata" switch for page headers. Same setting as in Settings. */
export function ViewModeToggle() {
  const mode = useViewMode();
  const setMode = useSetViewMode();
  const options = useViewModeOptions();
  const t = useT("common");
  return (
    <span
      className="inline-flex"
      title={t("viewModeHint")}
    >
      <Segmented label={t("viewModeLabel")} options={options} value={mode} onChange={setMode} />
    </span>
  );
}
