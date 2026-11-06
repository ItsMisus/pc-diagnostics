import type { ReactNode } from "react";
import { Skeleton } from "@/components/common/Skeleton";
import { UsageBar } from "@/components/common/UsageBar";
import type { Translator } from "@/i18n";
import type { Metric } from "@/types/generated/Metric";

/** Value as text for assistive technology ("62 °C", or "non disponibile"). */
export function metricText<T>(metric: Metric<T>, format: (value: T) => string, tm: Translator<"monitoring">): string {
  return metric.status === "available" ? format(metric.value) : tm("notAvailableText");
}

/** "12.3 GiB" → ["12.3", "GiB"], for `Readout` / `RingGauge` (number and unit drawn apart). */
export function splitUnit(text: string): [string, string | undefined] {
  const i = text.lastIndexOf(" ");
  return i > 0 ? [text.slice(0, i), text.slice(i + 1)] : [text, undefined];
}

export interface Bar {
  /** 0–100, `null` when the value is unavailable (empty track). */
  percent: number | null;
  /** Accessible name of the meter. */
  label: string;
  valueText: string;
}

/**
 * Secondary value, dashboard style: a thin bar above (only when the value has a real scale),
 * label on the left, value on the right. Not animated (live data).
 */
export function BarRow({ label, value, bar, title }: { label: string; value: ReactNode; bar?: Bar | undefined; title?: string | undefined }) {
  return (
    <div className="min-w-0" title={title}>
      {bar && <UsageBar percent={bar.percent} label={bar.label} valueText={bar.valueText} />}
      <div className={`flex items-baseline justify-between gap-3 text-[13px] leading-5 ${bar ? "mt-1" : ""}`}>
        <span className="min-w-[3.5rem] truncate text-fg-muted">{label}</span>
        <span className="shrink-0 text-right text-fg tabular">{value}</span>
      </div>
    </div>
  );
}

/** Gauge on the left, compact rows on the right (wrapping under the gauge when narrow). */
export function GaugeLayout({ gauge, children }: { gauge: ReactNode; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center gap-x-6 gap-y-4">
      {gauge}
      <div className="flex min-w-32 flex-1 flex-col gap-2.5">{children}</div>
    </div>
  );
}

const SIZE: Record<number, string> = { 140: "size-[140px]", 112: "size-[112px]" };

/** First-load placeholder with the panel's geometry (gauge + rows). Decorative. */
export function GaugeSkeleton({ rows, size = 140 }: { rows: number; size?: 140 | 112 }) {
  return (
    <span aria-hidden="true" className="flex flex-wrap items-center gap-x-6 gap-y-4">
      <Skeleton className={`shrink-0 rounded-full ${SIZE[size]}`} />
      <span className="flex min-w-32 flex-1 flex-col gap-3.5">
        {Array.from({ length: rows }, (_, i) => (
          <span key={i} className="block">
            <Skeleton className="h-1.5 w-full rounded-[2px]" />
            <span className="mt-1.5 flex justify-between">
              <Skeleton className="h-2.5 w-16" />
              <Skeleton className="h-2.5 w-12" />
            </span>
          </span>
        ))}
      </span>
    </span>
  );
}
