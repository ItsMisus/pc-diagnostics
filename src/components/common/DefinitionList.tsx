import type { ReactNode } from "react";

export interface DefinitionItem {
  label: string;
  value: ReactNode;
  /** Optional caption under the value (e.g. the metric source). */
  caption?: ReactNode;
}

/** Two-column label/value list with tabular numbers. */
export function DefinitionList({ items, className = "" }: { items: readonly DefinitionItem[]; className?: string }) {
  return (
    <dl className={`grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1.5 text-sm ${className}`}>
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
