import { memo, useId, useMemo } from "react";
import { useElementWidth } from "@/hooks/useElementWidth";
import { SERIES_STROKE, type SeriesTone } from "./LineChart";
import { areaPath, type Domain, lastPoint, linePath, type Point } from "./scale";

interface SparklineProps {
  points: readonly Point[];
  /** Time window shown (e.g. the last 60 s), so the line does not stretch while the buffer fills. */
  xDomain: Domain;
  /** Fixed value domain (0–100 for percentages). Never auto-scaled. */
  yDomain: Domain;
  /** Describes what the trend shows ("Carico CPU, ultimi 60 secondi"). */
  ariaLabel: string;
  height?: number;
  tone?: SeriesTone;
  /** Draws a dot on the latest sample. */
  markLast?: boolean;
  className?: string;
}

/**
 * Compact trend line for summary panels (Overview): no axes, no hover, fixed scales. The
 * current value must be written next to it as text; the sparkline only shows the shape of the
 * recent past. Like `LineChart`, it redraws in place without animating.
 */
function SparklineImpl({ points, xDomain, yDomain, ariaLabel, height = 32, tone = "primary", markLast = false, className = "" }: SparklineProps) {
  const [ref, width] = useElementWidth<HTMLDivElement>();
  const gradientId = `spark-${useId().replace(/:/g, "")}`;
  const plotH = height - 2;
  const line = useMemo(() => linePath(points, xDomain, yDomain, width, plotH), [points, xDomain, yDomain, width, plotH]);
  const area = tone === "secondary" ? "" : areaPath(line, plotH);
  const last = markLast ? lastPoint(line) : null;
  return (
    <div ref={ref} className={`w-full ${className}`} style={{ height }}>
      {width > 0 && (
        <svg width={width} height={height} role="img" aria-label={ariaLabel} className="block overflow-visible">
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={SERIES_STROKE[tone]} stopOpacity={0.18} />
              <stop offset="100%" stopColor={SERIES_STROKE[tone]} stopOpacity={0} />
            </linearGradient>
          </defs>
          <g transform="translate(0,1)">
            {area && <path d={area} fill={`url(#${gradientId})`} stroke="none" />}
            <path d={line} fill="none" stroke={SERIES_STROKE[tone]} strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round" />
            {last && <circle cx={last[0]} cy={last[1]} r={2.25} fill={SERIES_STROKE[tone]} />}
          </g>
        </svg>
      )}
    </div>
  );
}

export const Sparkline = memo(SparklineImpl);
