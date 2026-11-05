import { clamp } from "@/components/charts/scale";

interface UsageBarProps {
  /** 0–100. `null` renders an empty track (value unavailable). */
  percent: number | null;
  label: string;
  /** Human-readable value announced to assistive technology. */
  valueText: string;
  className?: string;
}

/**
 * Thin meter for live values. Deliberately not animated: samples arrive every second, so
 * a transition would keep the compositor busy for most of the time. Measured on the release
 * build, Overview idle: 0.32 % machine CPU with a 500 ms transition, 0.08 % without (same as
 * before the redesign). The fill is a scaled bar (transform only, no layout).
 * The value is always also written out as text by the caller.
 */
export function UsageBar({ percent, label, valueText, className = "" }: UsageBarProps) {
  const p = percent === null ? 0 : clamp(percent, 0, 100);
  return (
    <div
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent === null ? undefined : Math.round(p)}
      aria-valuetext={valueText}
      className={`h-1 w-full overflow-hidden rounded-full bg-surface-3 ${className}`}
    >
      <div
        className="h-full origin-left rounded-full bg-accent"
        style={{ transform: `scaleX(${p / 100})` }}
      />
    </div>
  );
}
