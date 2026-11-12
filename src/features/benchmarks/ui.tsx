import type { ReactNode } from "react";
import type { IconName } from "@/components/common/Icon";
import type { BenchComponent } from "./model";

/**
 * Local surface classes for the benchmark and history screens, mapped onto the design
 * system's materials (docs/design-system.md). `PANEL` is the secondary `Panel` surface, for
 * sections that need their own heading markup or ARIA attributes; `INSET` is a quieter box
 * nested inside a panel (metric tiles, charts, comparison). `PANEL_TITLE` matches the panel
 * title.
 */
export const PANEL = "surface-secondary";
export const INSET = "surface-inset";
export const PANEL_TITLE = "font-display text-md font-semibold text-fg";

/** Glyph of each benchmarkable component (one icon family). */
export const COMPONENT_ICON: Record<BenchComponent, IconName> = {
  cpu: "cpu",
  gpu: "gpu",
  memory: "memory",
  storage: "storage",
};

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
