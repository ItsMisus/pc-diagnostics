import type { ReactNode } from "react";

/**
 * Page title block. Display face for the title, one muted line of context, actions on the
 * right; on narrow windows the actions wrap under the title instead of squeezing it.
 */
export function PageHeader({ title, description, actions }: { title: string; description?: string; actions?: ReactNode }) {
  return (
    <header className="mb-6 flex flex-wrap items-end justify-between gap-x-6 gap-y-3">
      <div className="min-w-0 max-w-2xl">
        <h1 className="font-display text-[22px] leading-7 font-semibold tracking-[-0.01em] text-fg">{title}</h1>
        {description && <p className="mt-1 text-[13px] text-fg-muted">{description}</p>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </header>
  );
}
