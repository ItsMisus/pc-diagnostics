import { memo, useId, useMemo, useState, type PointerEvent } from "react";
import { useElementWidth } from "@/hooks/useElementWidth";
import { areaPath, type Domain, linear, linePath, nearestIndex, type Point, ticks } from "./scale";
import { localeTag, useT } from "@/i18n";

export interface Series {
  id: string;
  label: string;
  points: readonly Point[];
  /** Primary series use the accent with a soft area fill, secondary ones a muted line. */
  tone?: "primary" | "secondary";
}

interface LineChartProps {
  series: readonly Series[];
  xDomain: Domain;
  /** Fixed y domain: never auto-scaled, to avoid exaggerating small variations. */
  yDomain: Domain;
  formatY: (v: number) => string;
  formatX?: (t: number) => string;
  height?: number;
  ariaLabel: string;
}

/** Wide enough for the longest tick labels, e.g. "25.0 TFLOP/s" at 11px; narrower clips digits. */
const AXIS_W = 88;
const PAD_TOP = 6;
const PAD_BOTTOM = 6;
const TOOLTIP_W = 168;

/**
 * Static SVG line chart. Live updates redraw the path in place: there is deliberately no
 * per-sample animation (a 1 s cadence would keep the chart permanently in motion). Only
 * the hover layer animates.
 */
function LineChartImpl({
  series,
  xDomain,
  yDomain,
  formatY,
  formatX = (t) => new Date(t).toLocaleTimeString(localeTag()),
  height = 160,
  ariaLabel,
}: LineChartProps) {
  const [ref, width] = useElementWidth<HTMLDivElement>();
  const tc = useT("common");
  const [hoverT, setHoverT] = useState<number | null>(null);
  const gradientId = `chart-fill-${useId().replace(/:/g, "")}`;
  const plotW = Math.max(0, width - AXIS_W);
  const plotH = height - PAD_TOP - PAD_BOTTOM;

  const paths = useMemo(
    () =>
      series.map((s) => {
        const line = linePath(s.points, xDomain, yDomain, plotW, plotH);
        return { line, area: s.tone === "secondary" ? "" : areaPath(line, plotH) };
      }),
    [series, xDomain, yDomain, plotW, plotH],
  );
  const yTicks = useMemo(() => ticks(yDomain, 3), [yDomain]);
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
  const flip = AXIS_W + hoverX + 12 + TOOLTIP_W > width;

  return (
    <div ref={ref} className="relative w-full select-none" style={{ height }}>
      {width > 0 && (
        <svg width={width} height={height} role="img" aria-label={ariaLabel} className="block overflow-visible">
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--accent)" stopOpacity={0.22} />
              <stop offset="100%" stopColor="var(--accent)" stopOpacity={0} />
            </linearGradient>
          </defs>
          <g transform={`translate(${AXIS_W},${PAD_TOP})`}>
            {yTicks.map((v, i) => (
              <g key={v} transform={`translate(0,${yScale(v)})`}>
                <line
                  x1={0}
                  x2={plotW}
                  stroke="var(--line)"
                  strokeWidth={1}
                  strokeDasharray={i === 0 ? undefined : "2 3"}
                  shapeRendering="crispEdges"
                />
                <text x={-10} dy="0.32em" textAnchor="end" className="fill-fg-subtle text-2xs tabular">
                  {formatY(v)}
                </text>
              </g>
            ))}
            {paths.map(({ area }, i) =>
              area ? <path key={`a-${series[i]?.id ?? i}`} d={area} fill={`url(#${gradientId})`} stroke="none" /> : null,
            )}
            {paths.map(({ line }, i) => (
              <path
                key={series[i]?.id ?? i}
                d={line}
                fill="none"
                stroke={series[i]?.tone === "secondary" ? "var(--fg-subtle)" : "var(--accent)"}
                strokeWidth={series[i]?.tone === "secondary" ? 1.25 : 1.5}
                strokeDasharray={series[i]?.tone === "secondary" ? "3 2" : undefined}
                strokeLinejoin="round"
                strokeLinecap="round"
              />
            ))}
            {hovered && (
              <g className="pointer-events-none">
                <line x1={hoverX} x2={hoverX} y1={0} y2={plotH} stroke="var(--line-strong)" strokeWidth={1} />
                {series.map((s) => {
                  const p = s.points[nearestIndex(s.points, hovered.t)];
                  if (!p || p.v === null) return null;
                  return (
                    <circle
                      key={s.id}
                      cx={hoverX}
                      cy={Math.min(plotH, Math.max(0, yScale(p.v)))}
                      r={3}
                      fill="var(--surface)"
                      stroke={s.tone === "secondary" ? "var(--fg-subtle)" : "var(--accent)"}
                      strokeWidth={1.5}
                    />
                  );
                })}
              </g>
            )}
            <rect
              width={plotW}
              height={plotH}
              fill="transparent"
              onPointerMove={onMove}
              onPointerLeave={() => setHoverT(null)}
            />
          </g>
        </svg>
      )}
      {hovered && (
        <div
          className="pointer-events-none absolute top-0 z-10 rounded-lg border border-line-strong bg-surface-2 px-2.5 py-1.5 text-xs shadow-md"
          style={{
            width: TOOLTIP_W,
            left: flip ? Math.max(0, AXIS_W + hoverX - 12 - TOOLTIP_W) : AXIS_W + hoverX + 12,
          }}
        >
          <div className="mb-0.5 text-2xs text-fg-subtle tabular">{formatX(hovered.t)}</div>
          {series.map((s) => {
            const p = s.points[nearestIndex(s.points, hovered.t)];
            return (
              <div key={s.id} className="flex items-center justify-between gap-3 tabular">
                <span className="flex min-w-0 items-center gap-1.5 text-fg-muted">
                  <span
                    aria-hidden="true"
                    className={`h-0.5 w-2.5 shrink-0 rounded-full ${s.tone === "secondary" ? "bg-fg-subtle" : "bg-accent"}`}
                  />
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
