import type { ReactNode } from "react";

/**
 * Local surface classes for the benchmark and history screens. `PANEL` is the `<Card>`
 * surface, for sections that need their own heading markup or ARIA attributes; `INSET` is
 * a quieter box nested inside a panel (metric tiles, charts, comparison).
 */
export const PANEL = "rounded-xl border border-line bg-surface shadow-[inset_0_1px_0_var(--highlight)]";
export const INSET = "rounded-lg border border-line";

/** Plain column header matching `<Table>`'s own headers, for custom `head` rows. */
export function Th({
  children,
  align = "left",
  className = "",
}: {
  children?: ReactNode;
  align?: "left" | "right";
  className?: string;
}) {
  return (
    <th
      scope="col"
      className={`pr-4 pb-2 text-2xs font-medium whitespace-nowrap text-fg-subtle ${align === "right" ? "text-right" : "text-left"} ${className}`}
    >
      {children}
    </th>
  );
}
