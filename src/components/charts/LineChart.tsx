import { memo, useId, useMemo, useState, type PointerEvent } from "react";
import { useElementWidth } from "@/hooks/useElementWidth";
import { areaPath, type Domain, linear, linePath, nearestIndex, type Point, ticks } from "./scale";
import { localeTag, useT } from "@/i18n";

export type SeriesTone = "primary" | "secondary" | "accent";

export interface Series {
  id: string;
  label: string;
  points: readonly Point[];
  /**
   * `primary` (default): the main series, `--chart-1` with a soft area fill. `secondary`: a
   * comparison series, `--chart-2`, dashed, no fill. `accent`: the one highlighted series of
   * a screen, `--chart-accent` with fill. Never a status color.
   */
  tone?: SeriesTone;
}

interface LineChartProps {
  series: readonly Series[];
  xDomain: Domain;
  /** Fixed y domain: never auto-scaled, to avoid exaggerating small variations. */
  yDomain: Domain;
  /** Formats y labels and tooltip values, with the unit ("40 %", "1,2 GB/s"). */
  formatY: (v: number) => string;
  formatX?: (t: number) => string;
  height?: number;
  ariaLabel: string;
  /** Horizontal grid lines / y labels, including the baseline (default 3). */
  yTickCount?: number;
  /** Width reserved for the y labels (default 88, enough for "25.0 TFLOP/s"); ~48 for "%" charts. */
  axisWidth?: number;
}

export const SERIES_STROKE: Record<SeriesTone, string> = {
  primary: "var(--chart-1)",
  secondary: "var(--chart-2)",
  accent: "var(--chart-accent)",
};
const SWATCH: Record<SeriesTone, string> = {
  primary: "bg-chart-1",
  secondary: "bg-chart-2",
  accent: "bg-chart-accent",
};
const toneOf = (s: Series | undefined): SeriesTone => s?.tone ?? "primary";

const PAD_TOP = 6;
const PAD_BOTTOM = 6;
const TOOLTIP_W = 168;

/**
 * Static SVG line chart on a solid surface. Stable scales: the y domain is fixed by the caller
 * (0–100 for percentages, a fixed ceiling for temperatures), never auto-zoomed. The y labels
 * carry the unit (`formatY`); gaps (`v: null`) break the line instead of interpolating. Live
 * updates redraw the path in place: there is deliberately no per-sample animation (a 1 s
 * cadence would keep the chart permanently in motion). Hover shows a crosshair and a glass
 * tooltip with every series' value at that time.
 */
function LineChartImpl({
  series,
  xDomain,
  yDomain,
  formatY,
  formatX = (t) => new Date(t).toLocaleTimeString(localeTag()),
  height = 160,
  ariaLabel,
  yTickCount = 3,
  axisWidth = 88,
}: LineChartProps) {
  const [ref, width] = useElementWidth<HTMLDivElement>();
  const tc = useT("common");
  const [hoverT, setHoverT] = useState<number | null>(null);
  const gradientId = `chart-fill-${useId().replace(/:/g, "")}`;
  const plotW = Math.max(0, width - axisWidth);
  const plotH = height - PAD_TOP - PAD_BOTTOM;

  const paths = useMemo(
    () =>
      series.map((s) => {
        const line = linePath(s.points, xDomain, yDomain, plotW, plotH);
        const tone = toneOf(s);
        return { line, tone, area: tone === "secondary" ? "" : areaPath(line, plotH) };
      }),
    [series, xDomain, yDomain, plotW, plotH],
  );
  const yTicks = useMemo(() => ticks(yDomain, yTickCount), [yDomain, yTickCount]);
  const yScale = linear(yDomain, [plotH, 0]);

  const onMove = (e: PointerEvent<SVGRectElement>) => {
    const box = e.currentTarget.getBoundingClientRect();
    const t = linear([0, plotW], xDomain)(e.clientX - box.left);
    setHoverT(t);
  };

  const reference = series[0]?.points ?? [];
  const idx = hoverT === null ? -1 : nearestIndex(reference, hoverT);
  const hovered = idx >= 0 ? reference[idx] : undefined;
  const hoverX = hovered ? linear(xDomain, [0, plotW])(hovered.t) : 0;
  const flip = axisWidth + hoverX + 12 + TOOLTIP_W > width;

  return (
    <div ref={ref} className="relative w-full select-none" style={{ height }}>
      {width > 0 && (
        <svg width={width} height={height} role="img" aria-label={ariaLabel} className="block overflow-visible">
          <defs>
            {(["primary", "accent"] as const).map((tone) => (
              <linearGradient key={tone} id={`${gradientId}-${tone}`} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor={SERIES_STROKE[tone]} stopOpacity={0.2} />
                <stop offset="100%" stopColor={SERIES_STROKE[tone]} stopOpacity={0} />
              </linearGradient>
            ))}
          </defs>
          <g transform={`translate(${axisWidth},${PAD_TOP})`}>
            {yTicks.map((v, i) => (
              <g key={v} transform={`translate(0,${yScale(v)})`}>
                <line
                  x1={0}
                  x2={plotW}
                  stroke={i === 0 ? "var(--line-strong)" : "var(--chart-grid)"}
                  strokeWidth={1}
                  shapeRendering="crispEdges"
                />
                <text x={-10} dy="0.32em" textAnchor="end" className="fill-fg-subtle text-2xs tabular">
                  {formatY(v)}
                </text>
              </g>
            ))}
            {paths.map(({ area, tone }, i) =>
              area ? <path key={`a-${series[i]?.id ?? i}`} d={area} fill={`url(#${gradientId}-${tone})`} stroke="none" /> : null,
            )}
            {paths.map(({ line, tone }, i) => (
              <path
                key={series[i]?.id ?? i}
                d={line}
                fill="none"
                stroke={SERIES_STROKE[tone]}
                strokeWidth={tone === "secondary" ? 1.25 : 1.75}
                strokeDasharray={tone === "secondary" ? "3 2.5" : undefined}
                strokeLinejoin="round"
                strokeLinecap="round"
              />
            ))}
            {hovered && (
              <g className="pointer-events-none">
                <line x1={hoverX} x2={hoverX} y1={0} y2={plotH} stroke="var(--line-strong)" strokeWidth={1} shapeRendering="crispEdges" />
                {series.map((s) => {
                  const p = s.points[nearestIndex(s.points, hovered.t)];
                  if (!p || p.v === null) return null;
                  return (
                    <circle
                      key={s.id}
                      cx={hoverX}
                      cy={Math.min(plotH, Math.max(0, yScale(p.v)))}
                      r={3}
                      fill="var(--surface-raised)"
                      stroke={SERIES_STROKE[toneOf(s)]}
                      strokeWidth={1.75}
                    />
                  );
                })}
              </g>
            )}
            <rect width={plotW} height={plotH} fill="transparent" onPointerMove={onMove} onPointerLeave={() => setHoverT(null)} />
          </g>
        </svg>
      )}
      {hovered && (
        <div
          className="glass-thick pointer-events-none absolute top-0 z-10 rounded-md px-2.5 py-2 text-xs"
          style={{
            width: TOOLTIP_W,
            left: flip ? Math.max(0, axisWidth + hoverX - 12 - TOOLTIP_W) : axisWidth + hoverX + 12,
          }}
        >
          <div className="mb-1 text-2xs text-fg-subtle tabular">{formatX(hovered.t)}</div>
          {series.map((s) => {
            const p = s.points[nearestIndex(s.points, hovered.t)];
            return (
              <div key={s.id} className="flex items-center justify-between gap-3 tabular">
                <span className="flex min-w-0 items-center gap-1.5 text-fg-muted">
                  <span aria-hidden="true" className={`h-0.5 w-2.5 shrink-0 rounded-full ${SWATCH[toneOf(s)]}`} />
                  <span className="truncate">{s.label}</span>
                </span>
                <span className="font-medium text-fg">{p && p.v !== null ? formatY(p.v) : tc("notAvailable")}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export const LineChart = memo(LineChartImpl);
