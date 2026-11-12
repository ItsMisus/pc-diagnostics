import type { ReactNode } from "react";
import { Icon } from "./Icon";

/**
 * Native, keyboard-accessible "show more" used to tuck technical details away (the simple
 * view, raw error details, long lists). Height animates via `::details-content` (global CSS,
 * Chromium; instant in WKWebView); the chevron rotates.
 *
 * `variant="row"` is a full-width row with a hairline, for lists of collapsible sections
 * inside a panel; the default is an inline text link.
 */
export function Disclosure({
  summary,
  children,
  className = "",
  defaultOpen = false,
  variant = "inline",
}: {
  summary: ReactNode;
  children: ReactNode;
  className?: string;
  defaultOpen?: boolean;
  variant?: "inline" | "row";
}) {
  if (variant === "row") {
    return (
      <details className={`group border-t border-line first:border-t-0 ${className}`} open={defaultOpen || undefined}>
        <summary className="flex min-h-10 items-center gap-2 rounded-md py-2 text-sm font-medium text-fg select-none hover:text-fg">
          <span className="min-w-0 flex-1">{summary}</span>
          <Icon
            name="chevronRight"
            size={14}
            strokeWidth={1.75}
            className="shrink-0 text-fg-subtle transition-transform duration-200 ease-out group-open:rotate-90"
          />
        </summary>
        <div className="pb-3">{children}</div>
      </details>
    );
  }
  return (
    <details className={`group ${className}`} open={defaultOpen || undefined}>
      <summary className="inline-flex items-center gap-1 rounded-sm text-xs text-fg-muted transition-colors duration-150 select-none hover:text-fg">
        <Icon
          name="chevronRight"
          size={12}
          strokeWidth={1.75}
          className="text-fg-subtle transition-transform duration-200 ease-out group-open:rotate-90"
        />
        {summary}
      </summary>
      <div className="pt-2">{children}</div>
    </details>
  );
}
