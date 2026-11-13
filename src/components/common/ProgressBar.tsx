import { clamp } from "@/components/charts/scale";

interface ProgressBarProps {
  /** 0–1. */
  fraction: number;
  label: string;
  className?: string;
  /** `accent` for the one progress the user is waiting on; default grey meter. */
  tone?: "meter" | "accent";
}

/**
 * Determinate progress (role="progressbar"); the percentage is also exposed as text.
 * The fill scales (transform only) and eases between progress events.
 */
export function ProgressBar({ fraction, label, className = "", tone = "meter" }: ProgressBarProps) {
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
      <div
        className={`h-full origin-left transition-transform duration-300 ease-out ${tone === "accent" ? "bg-accent-solid" : "bg-meter"}`}
        style={{ transform: `scaleX(${f})` }}
      />
    </div>
  );
}
