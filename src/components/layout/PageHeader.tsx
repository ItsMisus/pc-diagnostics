import type { ReactNode } from "react";
import { useAppStore } from "@/app/store";
import { useT } from "@/i18n";
import { useIsMac } from "@/lib/platform";

/**
 * Page header in the Apple "large title" style: a 34 px bold title with one line under it that
 * says what the page is for, actions on the right. Sticky at the top of the main area: once
 * content scrolls under it (`data-scrolled` on <main>, set by AppShell) the title shrinks to
 * toolbar size, the subtitle folds away and the band turns into blurred glass
 * (`.page-toolbar` / `.page-title` in index.css). On macOS it also drags the window.
 *
 * The subtitle comes from the current section (`shell:subtitle_<section>`) unless one is
 * passed. Render it as the first child of the page. On narrow windows the actions wrap.
 */
export function PageHeader({
  title,
  subtitle,
  actions,
  leading,
}: {
  title: string;
  /** Overrides the section's own subtitle (e.g. a drill-down). `null` hides it. */
  subtitle?: string | null;
  /** Page-level actions: at most one primary button, the rest secondary/ghost. */
  actions?: ReactNode;
  /** Before the title, e.g. a back button in a drill-down. */
  leading?: ReactNode;
}) {
  const mac = useIsMac();
  const t = useT("shell");
  const section = useAppStore((s) => s.section);
  const line = subtitle === undefined ? t(`subtitle_${section}`) : subtitle;
  return (
    <header
      {...(mac ? { "data-tauri-drag-region": "deep" } : {})}
      className="page-toolbar mb-6 flex flex-wrap items-end justify-between gap-x-6 gap-y-3 pt-5 pb-4"
    >
      <div className="flex min-w-0 items-center gap-3">
        {leading}
        <div className="min-w-0">
          <h1 className="page-title min-w-0 truncate font-display font-bold text-fg">{title}</h1>
          {line && <p className="page-subtitle text-md text-fg-muted">{line}</p>}
        </div>
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2 pb-0.5">{actions}</div>}
    </header>
  );
}
