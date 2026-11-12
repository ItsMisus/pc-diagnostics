import type { ReactNode } from "react";

export type ReadoutSize = "sm" | "md" | "lg" | "xl";

const VALUE: Record<ReadoutSize, string> = {
  sm: "text-xl",
  md: "text-2xl",
  lg: "text-4xl",
  xl: "text-5xl",
};

const UNIT: Record<ReadoutSize, string> = {
  sm: "text-xs",
  md: "text-sm",
  lg: "text-md",
  xl: "text-lg",
};

/**
 * A big number with a smaller unit ("9 KB/s", "5425 MHz"): display face, regular weight,
 * tabular figures so live values never jitter. For unavailable values pass a `MetricValue`
 * (which renders "n/d" with its reason) or the "n/d" text as `value`, and no unit.
 * Sizes: `xl` 44 hero (one per screen), `lg` 34 headline, `md` 24 secondary, `sm` 20 dense.
 */
export function Readout({
  value,
  unit,
  size = "lg",
  className = "",
}: {
  value: ReactNode;
  unit?: string;
  size?: ReadoutSize;
  className?: string;
}) {
  return (
    <span className={`tabular font-display font-normal text-fg ${VALUE[size]} ${className}`}>
      {value}
      {unit && <span className={`ml-1 font-normal tracking-normal text-fg-muted ${UNIT[size]}`}>{unit}</span>}
    </span>
  );
}

/**
 * A labelled reading: small label, a `Readout`, and an optional line under it (a secondary
 * value, a status word, a source caption). The label names the reading once ("Lettura",
 * "Carico"); don't repeat the panel title in it.
 */
export function Stat({
  label,
  value,
  unit,
  size = "md",
  footer,
  className = "",
}: {
  label: ReactNode;
  value: ReactNode;
  unit?: string;
  size?: ReadoutSize;
  /** Under the value: e.g. "di 32 GB", a `StatusBadge`, a `SourceCaption`. */
  footer?: ReactNode;
  className?: string;
}) {
  return (
    <div className={`flex min-w-0 flex-col gap-1 ${className}`}>
      <span className="text-xs text-fg-muted">{label}</span>
      <Readout value={value} size={size} {...(unit ? { unit } : {})} />
      {footer && <div className="text-xs text-fg-subtle">{footer}</div>}
    </div>
  );
}
