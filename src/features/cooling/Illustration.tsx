import type { ReactNode } from "react";
import type { CoolingType } from "./model";

const BLADES = Array.from({ length: 9 }, (_, i) => (i * 360) / 9);

function FanArt({ cx, cy, r }: { cx: number; cy: number; r: number }) {
  return (
    <g>
      <rect x={cx - r} y={cy - r} width={2 * r} height={2 * r} rx={r * 0.18} fill="currentColor" fillOpacity={0.12} stroke="currentColor" strokeOpacity={0.3} />
      <circle cx={cx} cy={cy} r={r * 0.92} fill="currentColor" fillOpacity={0.08} />
      {BLADES.map((a) => (
        <path
          key={a}
          transform={`rotate(${a} ${cx} ${cy})`}
          d={`M${cx + r * 0.25} ${cy - r * 0.06} Q${cx + r * 0.6} ${cy - r * 0.22} ${cx + r * 0.88} ${cy} Q${cx + r * 0.7} ${cy + r * 0.28} ${cx + r * 0.3} ${cy + r * 0.12} Z`}
          fill="currentColor"
          fillOpacity={0.32}
        />
      ))}
      <circle cx={cx} cy={cy} r={r * 0.25} fill="currentColor" fillOpacity={0.45} />
    </g>
  );
}

const ART: Record<CoolingType, ReactNode> = {
  fan: <FanArt cx={100} cy={75} r={55} />,
  tower: (
    <g>
      {Array.from({ length: 14 }, (_, i) => (
        <rect key={i} x={55} y={22 + i * 7} width={90} height={3} rx={1} fill="currentColor" fillOpacity={0.3} />
      ))}
      {[70, 85, 115, 130].map((x) => (
        <rect key={x} x={x} y={20} width={4} height={112} rx={2} fill="currentColor" fillOpacity={0.5} />
      ))}
      <rect x={78} y={130} width={44} height={8} rx={2} fill="currentColor" fillOpacity={0.55} />
      <FanArt cx={100} cy={70} r={30} />
    </g>
  ),
  aio: (
    <g>
      <rect x={20} y={28} width={160} height={52} rx={6} fill="currentColor" fillOpacity={0.14} stroke="currentColor" strokeOpacity={0.3} />
      <FanArt cx={62} cy={54} r={22} />
      <FanArt cx={138} cy={54} r={22} />
      <path d="M172 80 C185 105 160 118 128 122 M164 80 C172 100 150 110 124 114" fill="none" stroke="currentColor" strokeOpacity={0.45} strokeWidth={5} strokeLinecap="round" />
      <ellipse cx={110} cy={124} rx={20} ry={9} fill="currentColor" fillOpacity={0.5} />
    </g>
  ),
  passive: (
    <g>
      <path d="M22 100 L60 50 L182 50 L150 100 Z" fill="currentColor" fillOpacity={0.14} stroke="currentColor" strokeOpacity={0.3} strokeLinejoin="round" />
      <path d="M85 70 L95 60 L135 60 L127 70 Z" fill="currentColor" fillOpacity={0.35} />
      {[16, 26, 36].map((r) => (
        <ellipse key={r} cx={110} cy={65} rx={r * 1.6} ry={r * 0.55} fill="none" stroke="currentColor" strokeOpacity={0.5 - r / 100} />
      ))}
    </g>
  ),
};

/** Flat drawing of a cooling type: the stage fallback without WebGL and while loading. Decorative. */
export function CoolingIllustration({ type, pulse = false }: { type: CoolingType; pulse?: boolean }) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 200 150" preserveAspectRatio="xMidYMid meet" className={`absolute inset-0 h-full w-full p-4 text-fg ${pulse ? "animate-skeleton" : ""}`}>
      {ART[type]}
    </svg>
  );
}
