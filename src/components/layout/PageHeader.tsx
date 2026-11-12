import type { ReactNode } from "react";
import { useIsMac } from "@/lib/platform";

/**
 * Page toolbar: the title in the display face, actions on the right. Sticky at the top of the
 * main area; it turns into a blurred glass band once content scrolls under it (`.page-toolbar`
 * in index.css, driven by AppShell's `data-scrolled`). 64 px tall, so on macOS the title sits
 * on the same line as the traffic lights, and there the band also drags the window.
 *
 * No subtitle: the navigation already says what the page is, and context that matters belongs
 * next to the data it explains (docs/ui-dashboard-brief.md, rule R2). Render it as the first
 * child of the page. On narrow windows the actions wrap under the title.
 */
export function PageHeader({
  title,
  actions,
  leading,
}: {
  title: string;
  /** Page-level actions: at most one primary button, the rest secondary/ghost. */
  actions?: ReactNode;
  /** Before the title, e.g. a back button in a drill-down. */
  leading?: ReactNode;
}) {
  const mac = useIsMac();
  return (
    <header
      {...(mac ? { "data-tauri-drag-region": "deep" } : {})}
      className="page-toolbar mb-4 flex min-h-16 flex-wrap items-center justify-between gap-x-6 gap-y-2 py-3"
    >
      <div className="flex min-w-0 items-center gap-2">
        {leading}
        <h1 className="min-w-0 truncate font-display text-2xl font-semibold text-fg">{title}</h1>
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </header>
  );
}
