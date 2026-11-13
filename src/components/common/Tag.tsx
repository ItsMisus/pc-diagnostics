import type { ReactNode } from "react";

/**
 * Small neutral label for a classification ("questa app", "driver Microsoft", "NVMe"). Not a
 * status: statuses use `StatusBadge`. `title` is a supplementary explanation only.
 */
export function Tag({ children, title }: { children: ReactNode; title?: string }) {
  return (
    <span
      className="inline-flex h-[18px] shrink-0 items-center rounded-sm border border-line bg-control px-1.5 text-2xs font-medium text-fg-muted"
      title={title}
    >
      {children}
    </span>
  );
}
