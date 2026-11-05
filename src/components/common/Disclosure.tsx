import type { ReactNode } from "react";
import { Icon } from "./Icon";

/**
 * Native, keyboard-accessible "show more" used to tuck technical details away in the simple
 * view. Height animates via `::details-content` (global CSS); the chevron rotates.
 */
export function Disclosure({
  summary,
  children,
  className = "",
}: {
  summary: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <details className={`group ${className}`}>
      <summary className="inline-flex items-center gap-1 rounded text-xs text-fg-muted transition-colors duration-150 select-none hover:text-fg">
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
