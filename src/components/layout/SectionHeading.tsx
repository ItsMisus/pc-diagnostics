import type { ReactNode } from "react";

/**
 * Heading of a group of panels inside a page ("Andamento recente", "Avvisi"). Sits between the
 * page title (24 px) and the panel titles (17/15 px). The optional `count` is muted and
 * tabular; `aside` takes a small control (a ghost "Mostra tutto", a filter).
 */
export function SectionHeading({
  id,
  children,
  count,
  aside,
  className = "",
}: {
  id?: string;
  children: ReactNode;
  count?: number;
  aside?: ReactNode;
  className?: string;
}) {
  const heading = (
    <h2 id={id} className="flex items-baseline gap-2 font-display text-lg font-semibold text-fg">
      {children}
      {count !== undefined && <span className="text-sm font-normal text-fg-subtle tabular">{count}</span>}
    </h2>
  );
  if (!aside) return <div className={className}>{heading}</div>;
  return (
    <div className={`flex items-center justify-between gap-3 ${className}`}>
      {heading}
      <div className="flex shrink-0 items-center gap-2">{aside}</div>
    </div>
  );
}
