import type { ReactNode } from "react";
import { clamp } from "@/components/charts/scale";

/**
 * Local surface classes for the benchmark and history screens, mapped onto the design
 * system's materials (docs/design-system.md). `PANEL` is the secondary `Panel` surface, for
 * sections that need their own heading markup or ARIA attributes; `INSET` is a quieter box
 * nested inside a panel (metric tiles, charts, comparison). `PANEL_TITLE` matches the panel
 * title.
 */
export const PANEL = "surface-secondary";
export const INSET = "rounded-md border border-line";
export const PANEL_TITLE = "font-display text-md font-semibold text-fg";

/**
 * Determinate run progress, monochrome like every other meter. Not animated: progress
 * events arrive many times per second, and live values never animate.
 */
export function RunProgress({ fraction, label, className = "" }: { fraction: number; label: string; className?: string }) {
  const f = clamp(Number.isFinite(fraction) ? fraction : 0, 0, 1);
  const pct = Math.round(f * 100);
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={pct}
      aria-valuetext={`${pct}%`}
      className={`h-1.5 w-full overflow-hidden rounded-full bg-gauge-track ${className}`}
    >
      <div className="h-full origin-left bg-meter" style={{ transform: `scaleX(${f})` }} />
    </div>
  );
}

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
