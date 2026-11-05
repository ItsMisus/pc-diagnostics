import type { ReactNode } from "react";

/**
 * A big live number with a smaller unit ("9 KB/s", "5425 MHz"), dashboard style: light
 * weight, tabular figures. For unavailable values pass the "n/d" text as `value` and no unit.
 */
export function Readout({
  value,
  unit,
  size = "lg",
  className = "",
}: {
  value: ReactNode;
  unit?: string;
  size?: "md" | "lg";
  className?: string;
}) {
  return (
    <span className={`tabular font-display font-light tracking-[-0.02em] text-fg ${size === "lg" ? "text-[34px] leading-10" : "text-2xl leading-8"} ${className}`}>
      {value}
      {unit && <span className={`ml-1 font-normal text-fg-muted ${size === "lg" ? "text-base" : "text-sm"}`}>{unit}</span>}
    </span>
  );
}
