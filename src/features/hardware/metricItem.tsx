import type { DefinitionItem } from "@/components/common/DefinitionList";
import { MetricValue } from "@/components/common/MetricValue";
import { SourceCaption } from "@/components/common/SourceCaption";
import type { Metric } from "@/types/generated/Metric";

/** Definition row for a metric: the value (or n/d with reason) plus its source caption. */
export function metricItem<T>(label: string, metric: Metric<T>, format: (value: T) => string): DefinitionItem {
  return {
    label,
    value: <MetricValue metric={metric} format={format} />,
    caption: <SourceCaption metric={metric} />,
  };
}
