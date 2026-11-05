import { sourceLabel } from "@/lib/format";
import type { Metric } from "@/types/generated/Metric";
import type { Source } from "@/types/generated/Source";
import { useT } from "@/i18n";

/** Subtle provenance caption ("fonte: PDH"). Renders nothing for an unavailable metric. */
export function SourceCaption({ metric, source }: { metric?: Metric<unknown>; source?: Source }) {
  const resolved = source ?? (metric?.status === "available" ? metric.source : undefined);
  const t = useT("common");
  if (!resolved) return null;
  return <span className="text-2xs text-fg-subtle">{t("source", { source: sourceLabel(resolved) })}</span>;
}
