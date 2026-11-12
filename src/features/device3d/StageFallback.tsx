import { useId, useMemo } from "react";
import { illustrationFor, type Tone } from "./illustration";
import type { DeviceKind } from "./types";

/** Fill opacity of each tone over `currentColor` (the theme's foreground). */
const TONE_OPACITY: Record<Tone, number> = {
  top: 0.2,
  front: 0.13,
  side: 0.08,
  glass: 0.035,
  dark: 0.42,
  line: 0.22,
  light: 0.75,
  screen: 1,
};

/**
 * The device as a flat illustration in the hero pose: shown when WebGL is unavailable or was
 * lost (`variant="illustration"`), and as the loading silhouette (`variant="silhouette"`),
 * a soft single-tone shape with the skeleton pulse (stopped by reduced motion).
 * Decorative: the stage element carries the accessible name.
 */
export function StageFallback({ kind, variant }: { kind: DeviceKind; variant: "illustration" | "silhouette" }) {
  const art = useMemo(() => illustrationFor(kind), [kind]);
  // SVG url(#…) references need a plain id (useId contains punctuation).
  const id = `d3d${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const silhouette = variant === "silhouette";
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      viewBox={art.viewBox.join(" ")}
      preserveAspectRatio="xMidYMid meet"
      className={`absolute inset-0 h-full w-full text-fg ${silhouette ? "animate-skeleton" : ""}`}
    >
      <defs>
        <radialGradient id={`${id}-shadow`}>
          <stop offset="0%" stopColor="currentColor" stopOpacity={silhouette ? 0.1 : 0.2} />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0" />
        </radialGradient>
        <linearGradient id={`${id}-screen`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.5" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0.3" />
        </linearGradient>
      </defs>
      <ellipse cx={art.shadow.cx} cy={art.shadow.cy} rx={art.shadow.rx} ry={art.shadow.ry} fill={`url(#${id}-shadow)`} />
      <g opacity={silhouette ? 0.09 : 1}>
        {art.shapes.map((s, i) => (
          <polygon
            key={i}
            points={s.points.map((p) => `${p[0].toFixed(3)},${p[1].toFixed(3)}`).join(" ")}
            fill={!silhouette && s.tone === "screen" ? `url(#${id}-screen)` : "currentColor"}
            fillOpacity={silhouette ? 1 : TONE_OPACITY[s.tone]}
            stroke={silhouette || s.tone === "line" ? "none" : "currentColor"}
            strokeOpacity={0.18}
            strokeWidth={1}
            strokeLinejoin="round"
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </g>
    </svg>
  );
}
