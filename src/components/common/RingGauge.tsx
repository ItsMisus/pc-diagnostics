import { clamp } from "@/components/charts/scale";
import { METER_FILL, type MeterTone } from "./UsageBar";

interface RingGaugeProps {
  /** Fill, 0–100. `null` draws only the track (value unavailable). */
  percent: number | null;
  /** Number in the centre, already formatted ("42", "61", "n/d"). */
  value: string;
  /** Unit drawn smaller next to the number ("%", "°C"). Omitted when the value is unavailable. */
  unit?: string;
  /** Small caption in the open part of the ring ("Carico", "Load"). */
  caption?: string;
  /** Accessible name of the meter. */
  label: string;
  /** Accessible value; defaults to `value` + `unit`. */
  valueText?: string;
  /** Status of the value: colours the arc. The caller also states it as text. */
  tone?: MeterTone | undefined;
  /** Outer diameter in px. */
  size?: number;
  /** Ring thickness in px. */
  stroke?: number;
  className?: string;
}

/** The arc covers 270°, open at the bottom; the caption sits in the opening. */
const SWEEP = 0.75;

/**
 * Open circular meter for the headline live values of the dashboard (load, temperature,
 * memory). Monochrome (`--meter` on `--gauge-track`); `tone` colours the arc when the value
 * is at a warning level. Round caps, the number in the display face with a smaller unit.
 * Like `UsageBar`, deliberately not animated: samples arrive every second
 * and a transition would keep the compositor busy (docs/design-system.md, "Live data never
 * animates"). The value is always written out, so the arc is never the only signal.
 */
export function RingGauge({
  percent,
  value,
  unit,
  caption,
  label,
  valueText,
  tone,
  size = 140,
  stroke = 8,
  className = "",
}: RingGaugeProps) {
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const arc = c * SWEEP;
  const p = percent === null ? 0 : clamp(percent, 0, 100);
  const big = size >= 112;
  const available = percent !== null;
  // Rotate so the 270° arc starts at 7:30 and ends at 4:30 (opening centred at the bottom).
  const rotate = 90 + (360 * (1 - SWEEP)) / 2;
  const common = {
    cx: size / 2,
    cy: size / 2,
    r,
    fill: "none",
    strokeWidth: stroke,
    strokeLinecap: "round" as const,
  };
  return (
    <div
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={available ? Math.round(p) : undefined}
      aria-valuetext={valueText ?? `${value}${available && unit ? ` ${unit}` : ""}`}
      className={`relative inline-grid shrink-0 place-items-center ${className}`}
      style={{ width: size, height: size }}
    >
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden="true" style={{ transform: `rotate(${rotate}deg)` }}>
        <circle {...common} stroke="var(--gauge-track)" strokeDasharray={`${arc} ${c}`} />
        {available && p > 0 && <circle {...common} stroke={METER_FILL[tone ?? "normal"]} strokeDasharray={`${(arc * p) / 100} ${c}`} />}
      </svg>
      <div aria-hidden="true" className="absolute inset-0 grid place-content-center text-center">
        <span className={`tabular font-display font-normal ${big ? "text-4xl" : "text-xl"} ${available ? "text-fg" : "text-fg-subtle"}`}>
          {value}
          {available && unit && <span className={`ml-0.5 font-normal tracking-normal text-fg-muted ${big ? "text-md" : "text-xs"}`}>{unit}</span>}
        </span>
      </div>
      {caption && (
        <span aria-hidden="true" className={`absolute inset-x-0 text-center text-fg-muted ${big ? "bottom-1 text-sm" : "bottom-0 text-2xs"}`}>
          {caption}
        </span>
      )}
    </div>
  );
}
