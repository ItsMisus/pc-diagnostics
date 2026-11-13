import type { ReactNode } from "react";

export interface DefinitionItem {
  label: string;
  value: ReactNode;
  /** Optional caption under the value (e.g. the metric source). */
  caption?: ReactNode;
}

/**
 * Label/value list with tabular numbers.
 * - default: two columns, the label column as wide as the longest label.
 * - `aligned`: fixed label column (long labels wrap), so lists in panels side by side start
 *   their values at the same x.
 * - `divided`: macOS "About" style rows: label left, value right-aligned, a hairline between
 *   rows. For spec sheets (Hardware) and short summaries.
 */
export function DefinitionList({
  items,
  className = "",
  aligned = false,
  divided = false,
}: {
  items: readonly DefinitionItem[];
  className?: string;
  aligned?: boolean;
  divided?: boolean;
}) {
  if (divided) {
    return (
      <dl className={`text-sm ${className}`}>
        {items.map((item) => (
          <div key={item.label} className="flex items-baseline justify-between gap-6 border-t border-line py-2 first:border-t-0 first:pt-0 last:pb-0">
            <dt className="shrink-0 text-fg-muted">{item.label}</dt>
            <dd className="tabular min-w-0 text-right text-fg">
              <span className="break-words">{item.value}</span>
              {item.caption && <span className="ml-2 empty:hidden">{item.caption}</span>}
            </dd>
          </div>
        ))}
      </dl>
    );
  }
  const columns = aligned ? "grid-cols-[minmax(0,12rem)_minmax(0,1fr)]" : "grid-cols-[max-content_1fr]";
  return (
    <dl className={`grid ${columns} gap-x-6 gap-y-1.5 text-sm ${className}`}>
      {items.map((item) => (
        <div key={item.label} className="contents">
          <dt className="text-fg-muted">{item.label}</dt>
          <dd className="tabular min-w-0 text-fg">
            <span className="break-words">{item.value}</span>
            {item.caption && <span className="ml-2 empty:hidden">{item.caption}</span>}
          </dd>
        </div>
      ))}
    </dl>
  );
}
