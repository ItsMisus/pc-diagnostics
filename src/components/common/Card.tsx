import type { ReactNode } from "react";

interface CardProps {
  title?: string;
  /** Optional line under the title. */
  description?: ReactNode;
  aside?: ReactNode;
  children: ReactNode;
  className?: string;
  /** Removes the inner padding (e.g. for edge-to-edge tables). Header keeps its padding. */
  flush?: boolean;
}

/**
 * The panel every section sits on, monitoring-dashboard style: flat, slightly lighter than
 * the canvas, a visible hairline, small radius, no shadow. The title is the component's name
 * ("CPU", "RAM"), short and bold; the content (big numbers, gauges, tables) carries the rest.
 */
export function Card({ title, description, aside, children, className = "", flush = false }: CardProps) {
  return (
    <section
      className={`rounded-md border border-line bg-surface ${flush ? "" : "p-4"} ${className}`}
    >
      {(title || aside) && (
        <header className={`mb-3 flex min-h-6 items-start justify-between gap-3 ${flush ? "px-4 pt-4" : ""}`}>
          <div className="min-w-0">
            {title && <h2 className="font-display text-lg leading-6 font-semibold text-fg">{title}</h2>}
            {description && <p className="mt-0.5 text-xs text-fg-subtle">{description}</p>}
          </div>
          {aside && <div className="flex shrink-0 items-center gap-2">{aside}</div>}
        </header>
      )}
      {children}
    </section>
  );
}
