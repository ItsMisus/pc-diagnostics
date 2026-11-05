import type { ReactNode } from "react";

/**
 * Page title block: the title in the display face, actions on the right. No subtitle: the
 * navigation already says what the page is, and context that matters belongs next to the
 * data it explains (docs/ui-dashboard-brief.md, rule R2). On narrow windows the actions wrap
 * under the title instead of squeezing it.
 */
export function PageHeader({
  title,
  actions,
}: {
  title: string;
  /** @deprecated Ignored (no subtitles); remove it from call sites. Dropped after the dashboard redesign. */
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <header className="mb-6 flex min-h-9 flex-wrap items-center justify-between gap-x-6 gap-y-3">
      <h1 className="min-w-0 font-display text-[26px] leading-8 font-semibold tracking-[-0.015em] text-fg">{title}</h1>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </header>
  );
}
