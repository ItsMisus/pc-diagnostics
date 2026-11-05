import { useIsSimple } from "@/hooks/useViewMode";
import { reasonTexts } from "@/lib/plain";
import type { Metric } from "@/types/generated/Metric";
import type { Unavailability } from "@/types/generated/Unavailability";
import { useT } from "@/i18n";

interface MetricValueProps<T> {
  metric: Metric<T>;
  format: (value: T) => string;
  className?: string;
}

/**
 * Renders a measured value, or "n/d" with the reason as an accessible description.
 * In the "Semplice" view the reason is in plain words, with the technical one in the tooltip.
 */
export function MetricValue<T>({ metric, format, className = "" }: MetricValueProps<T>) {
  const simple = useIsSimple();
  const t = useT("common");
  if (metric.status === "available") {
    return <span className={`tabular ${className}`}>{format(metric.value)}</span>;
  }
  const { text, tooltip } = reasonTexts(metric.reason, simple);
  return (
    <span className={`text-fg-subtle ${className}`} title={tooltip}>
      {t("notAvailable")}<span className="sr-only"> — {text}</span>
    </span>
  );
}

/** A reason shown as text: plain in "Semplice" (technical one in the tooltip), as today in "Avanzata". */
export function UnavailableReason({ reason, className = "" }: { reason: Unavailability; className?: string }) {
  const simple = useIsSimple();
  const { text, tooltip } = reasonTexts(reason, simple);
  return (
    <span className={className} title={simple ? tooltip : undefined}>
      {text}
    </span>
  );
}

/** For props that need a string (e.g. `StateView.description`). */
export function useUnavailabilityText(): (reason: Unavailability) => string {
  const simple = useIsSimple();
  return (reason) => reasonTexts(reason, simple).text;
}
