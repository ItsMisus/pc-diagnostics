import { clamp } from "@/components/charts/scale";

interface UsageBarProps {
  /** 0–100. `null` renders an empty track (value unavailable). */
  percent: number | null;
  label: string;
  /** Human-readable value announced to assistive technology. */
  valueText: string;
  /**
   * Status of the value. `warn` / `bad` colour the fill; the caller must also write the
   * status as text (colour is never the only signal). Omitted: the grey meter fill.
   */
  tone?: MeterTone | undefined;
  className?: string;
}

export type MeterTone = "warn" | "bad";

/** Fill colour per tone; shared with `RingGauge`. */
export const METER_FILL: Record<MeterTone | "normal", string> = {
  normal: "var(--meter)",
  warn: "var(--warn)",
  bad: "var(--bad)",
};

/**
 * Thin meter for live values. Deliberately not animated: samples arrive every second, so
 * a transition would keep the compositor busy for most of the time. Measured on the release
 * build, Overview idle: 0.32 % machine CPU with a 500 ms transition, 0.08 % without (same as
 * before the redesign). The fill is a scaled bar (transform only, no layout).
 * The value is always also written out as text by the caller.
 */
export function UsageBar({ percent, label, valueText, tone, className = "" }: UsageBarProps) {
  const p = percent === null ? 0 : clamp(percent, 0, 100);
  return (
    <div
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent === null ? undefined : Math.round(p)}
      aria-valuetext={valueText}
      className={`h-1.5 w-full overflow-hidden rounded-full bg-gauge-track ${className}`}
    >
      <div
        className="h-full origin-left"
        style={{ transform: `scaleX(${p / 100})`, background: METER_FILL[tone ?? "normal"] }}
      />
    </div>
  );
}
