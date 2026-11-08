import type { ReactNode } from "react";

export interface DefinitionItem {
  label: string;
  value: ReactNode;
  /** Optional caption under the value (e.g. the metric source). */
  caption?: ReactNode;
}

/**
 * Two-column label/value list with tabular numbers. `aligned` gives the label column a fixed
 * width (long labels wrap), so lists in panels side by side start their values at the same
 * x; by default the column is as wide as the longest label.
 */
export function DefinitionList({
  items,
  className = "",
  aligned = false,
}: {
  items: readonly DefinitionItem[];
  className?: string;
  aligned?: boolean;
}) {
  const columns = aligned ? "grid-cols-[minmax(0,12rem)_minmax(0,1fr)]" : "grid-cols-[max-content_1fr]";
  return (
    <dl className={`grid ${columns} gap-x-6 gap-y-1.5 text-sm ${className}`}>
      {items.map((item) => (
        <div key={item.label} className="contents">
          <dt className="text-fg-muted">{item.label}</dt>
          <dd className="tabular min-w-0">
            <span className="break-words">{item.value}</span>
            {item.caption && <span className="ml-2">{item.caption}</span>}
          </dd>
        </div>
      ))}
    </dl>
  );
}
