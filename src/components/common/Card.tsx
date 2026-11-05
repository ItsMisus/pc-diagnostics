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
 * The surface every section sits on: hairline border, a faint top highlight in dark mode,
 * no heavy shadow. Titles are sentence case and quiet; the content carries the weight.
 */
export function Card({ title, description, aside, children, className = "", flush = false }: CardProps) {
  return (
    <section
      className={`rounded-xl border border-line bg-surface shadow-[inset_0_1px_0_var(--highlight)] ${flush ? "" : "p-4"} ${className}`}
    >
      {(title || aside) && (
        <header className={`mb-3 flex min-h-5 items-start justify-between gap-3 ${flush ? "px-4 pt-4" : ""}`}>
          <div className="min-w-0">
            {title && <h2 className="text-[13px] leading-5 font-medium text-fg-muted">{title}</h2>}
            {description && <p className="mt-0.5 text-xs text-fg-subtle">{description}</p>}
          </div>
          {aside && <div className="flex shrink-0 items-center gap-2">{aside}</div>}
        </header>
      )}
      {children}
    </section>
  );
}
