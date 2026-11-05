import { clamp } from "@/components/charts/scale";

interface ProgressBarProps {
  /** 0–1. */
  fraction: number;
  label: string;
  className?: string;
}

/**
 * Determinate progress (role="progressbar"); the percentage is also exposed as text.
 * The fill scales (transform only) and eases between progress events.
 */
export function ProgressBar({ fraction, label, className = "" }: ProgressBarProps) {
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
      className={`h-1.5 w-full overflow-hidden rounded-full bg-surface-3 ${className}`}
    >
      <div
        className="h-full origin-left rounded-full bg-accent transition-transform duration-300 ease-out"
        style={{ transform: `scaleX(${f})` }}
      />
    </div>
  );
}
