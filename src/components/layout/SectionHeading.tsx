import type { ReactNode } from "react";

/**
 * Heading of a group of cards inside a page ("Andamento recente", "Avvisi"). Sits between the
 * page title (26 px) and the card titles (18 px), so a group always reads above the cards it
 * holds. The optional `count` is muted and tabular.
 */
export function SectionHeading({ id, children, count, className = "" }: { id?: string; children: ReactNode; count?: number; className?: string }) {
  return (
    <h2 id={id} className={`flex items-baseline gap-2 font-display text-xl leading-7 font-semibold text-fg ${className}`}>
      {children}
      {count !== undefined && <span className="text-sm font-normal text-fg-subtle tabular">{count}</span>}
    </h2>
  );
}
