import { useId, useRef, type KeyboardEvent, type PointerEvent } from "react";
import { useElementWidth } from "@/hooks/useElementWidth";
import { useT } from "@/i18n";
import { formatCelsius, formatPercent } from "@/lib/format";
import type { CoolingCurvePoint } from "@/types/generated/CoolingCurvePoint";
import { type ArrowKey, curvePath, DUTY_MAX, DUTY_MIN, dutyAt, dutyToY, movePoint, nudgePoint, type Plot, TEMP_MAX, TEMP_MIN, tempToX, xToTemp, yToDuty } from "./curve";

const HEIGHT = 260;
const PAD = { left: 44, right: 14, top: 14, bottom: 30 };
const T_TICKS = [20, 30, 40, 50, 60, 70, 80, 90, 100];
const D_TICKS = [0, 25, 50, 75, 100];
const ARROWS: readonly string[] = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"];

/**
 * Interactive fan curve: temperature 20–100 °C across, fan speed 0–100 % up. Points are
 * dragged with the pointer (snapped to 1 °C / 1 %, kept between their neighbours so the curve
 * is always valid) or moved with the arrow keys once focused (Shift: steps of 5, Delete
 * removes). The CPU temperature, when the system reports it, is a dashed vertical marker with
 * the speed the curve gives there. Chart tokens only; nothing animates.
 */
export function CurveChart({
  points,
  onChange,
  focused,
  onFocusPoint,
  onRemovePoint,
  markerC,
}: {
  points: readonly CoolingCurvePoint[];
  onChange: (points: CoolingCurvePoint[]) => void;
  focused: number | null;
  onFocusPoint: (index: number | null) => void;
  onRemovePoint: (index: number) => void;
  markerC: number | null;
}) {
  const t = useT("cooling");
  const [ref, width] = useElementWidth<HTMLDivElement>();
  const svgRef = useRef<SVGSVGElement>(null);
  const dragging = useRef<number | null>(null);
  const helpId = useId();
  const fillId = `curve-fill-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const plot: Plot = { x: PAD.left, y: PAD.top, w: Math.max(0, width - PAD.left - PAD.right), h: HEIGHT - PAD.top - PAD.bottom };
  const line = curvePath(plot, points);
  const area = line ? `${line} L${(plot.x + plot.w).toFixed(1)},${(plot.y + plot.h).toFixed(1)} L${plot.x.toFixed(1)},${(plot.y + plot.h).toFixed(1)} Z` : "";

  const toLocal = (e: PointerEvent) => {
    const box = svgRef.current?.getBoundingClientRect();
    return box ? { x: e.clientX - box.left, y: e.clientY - box.top } : null;
  };

  const onPointerDown = (i: number) => (e: PointerEvent<SVGGElement>) => {
    e.preventDefault();
    dragging.current = i;
    e.currentTarget.setPointerCapture(e.pointerId);
    e.currentTarget.focus();
    onFocusPoint(i);
  };
  const onPointerMove = (e: PointerEvent<SVGGElement>) => {
    const i = dragging.current;
    if (i === null) return;
    const p = toLocal(e);
    if (p) onChange(movePoint(points, i, xToTemp(plot, p.x), yToDuty(plot, p.y)));
  };
  const onPointerUp = (e: PointerEvent<SVGGElement>) => {
    dragging.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
  };
  const onKeyDown = (i: number) => (e: KeyboardEvent<SVGGElement>) => {
    if (ARROWS.includes(e.key)) {
      e.preventDefault();
      onChange(nudgePoint(points, i, e.key as ArrowKey, e.shiftKey));
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      onRemovePoint(i);
    }
  };

  const marker = markerC !== null && Number.isFinite(markerC) ? Math.min(TEMP_MAX, Math.max(TEMP_MIN, markerC)) : null;
  const markerDuty = marker !== null ? dutyAt(points, marker) : null;

  return (
    <div ref={ref} className="relative w-full select-none" style={{ height: HEIGHT }}>
      <span id={helpId} className="sr-only">
        {t("curvePointHelp")}
      </span>
      {width > 0 && (
        <svg ref={svgRef} width={width} height={HEIGHT} className="block overflow-visible touch-none" role="group" aria-label={t("curveChartLabel")} aria-describedby={helpId}>
          <defs>
            <linearGradient id={fillId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--chart-accent)" stopOpacity={0.22} />
              <stop offset="100%" stopColor="var(--chart-accent)" stopOpacity={0.02} />
            </linearGradient>
          </defs>
          {D_TICKS.map((d) => (
            <g key={`d${d}`}>
              <line x1={plot.x} x2={plot.x + plot.w} y1={dutyToY(plot, d)} y2={dutyToY(plot, d)} stroke={d === DUTY_MIN ? "var(--line-strong)" : "var(--chart-grid)"} shapeRendering="crispEdges" />
              <text x={plot.x - 8} y={dutyToY(plot, d)} dy="0.32em" textAnchor="end" className="fill-fg-subtle text-2xs tabular">
                {formatPercent(d)}
              </text>
            </g>
          ))}
          {T_TICKS.map((v) => (
            <g key={`t${v}`}>
              <line x1={tempToX(plot, v)} x2={tempToX(plot, v)} y1={plot.y} y2={plot.y + plot.h} stroke="var(--chart-grid)" shapeRendering="crispEdges" />
              <text x={tempToX(plot, v)} y={plot.y + plot.h + 18} textAnchor="middle" className="fill-fg-subtle text-2xs tabular">
                {formatCelsius(v)}
              </text>
            </g>
          ))}
          {area && <path d={area} fill={`url(#${fillId})`} stroke="none" />}
          {line && <path d={line} fill="none" stroke="var(--chart-accent)" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />}

          {marker !== null && (
            <g className="pointer-events-none" aria-hidden="true">
              <line x1={tempToX(plot, marker)} x2={tempToX(plot, marker)} y1={plot.y} y2={plot.y + plot.h} stroke="var(--fg-muted)" strokeDasharray="3 4" shapeRendering="crispEdges" />
              {markerDuty !== null && <circle cx={tempToX(plot, marker)} cy={dutyToY(plot, markerDuty)} r={3.5} fill="var(--fg-muted)" />}
              <text
                x={tempToX(plot, marker)}
                y={plot.y - 2}
                textAnchor={marker > 85 ? "end" : marker < 30 ? "start" : "middle"}
                className="fill-fg-muted text-2xs tabular"
              >
                {t("curveNow", { temp: formatCelsius(markerC ?? marker) })}
                {markerDuty !== null ? ` · ${formatPercent(markerDuty)}` : ""}
              </text>
            </g>
          )}

          {points.map((p, i) => {
            const cx = tempToX(plot, p.temperatureC);
            const cy = dutyToY(plot, p.dutyPercent);
            const isFocused = focused === i;
            return (
              <g
                key={i}
                role="slider"
                tabIndex={0}
                aria-label={t("curvePoint", { n: i + 1, temp: formatCelsius(p.temperatureC), duty: formatPercent(p.dutyPercent) })}
                aria-valuemin={DUTY_MIN}
                aria-valuemax={DUTY_MAX}
                aria-valuenow={p.dutyPercent}
                aria-valuetext={`${formatCelsius(p.temperatureC)}, ${formatPercent(p.dutyPercent)}`}
                aria-describedby={helpId}
                onPointerDown={onPointerDown(i)}
                onPointerMove={onPointerMove}
                onPointerUp={onPointerUp}
                onPointerCancel={onPointerUp}
                onKeyDown={onKeyDown(i)}
                onFocus={() => onFocusPoint(i)}
                className="cursor-grab outline-none active:cursor-grabbing [&:focus-visible>.ring]:opacity-100"
              >
                {/* Large invisible hit area for the pointer. */}
                <circle cx={cx} cy={cy} r={14} fill="transparent" />
                <circle className="ring opacity-0 transition-opacity duration-150" cx={cx} cy={cy} r={10} fill="none" stroke="var(--focus)" strokeWidth={2} />
                <circle cx={cx} cy={cy} r={isFocused ? 6.5 : 5.5} fill="var(--surface-raised)" stroke="var(--chart-accent)" strokeWidth={2.5} />
              </g>
            );
          })}
        </svg>
      )}
    </div>
  );
}
