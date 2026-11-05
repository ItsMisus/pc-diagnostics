import type { ReactNode } from "react";

/**
 * Small neutral label for a classification ("questa app", "driver Microsoft"). Not a status:
 * statuses use `StatusBadge`.
 */
export function Tag({ children, title }: { children: ReactNode; title?: string }) {
  return (
    <span
      className="inline-flex shrink-0 items-center rounded-sm border border-line bg-surface-2 px-1 py-px text-2xs leading-3.5 font-medium text-fg-muted"
      title={title}
    >
      {children}
    </span>
  );
}
