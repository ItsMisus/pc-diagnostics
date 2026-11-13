import { memo, type ReactNode, useId } from "react";
import type { BenchComponent } from "./model";

/**
 * Isometric illustrations of the four benchmarkable parts (CPU, graphics card, memory module,
 * NVMe SSD), drawn as boxes in a true isometric projection so they read as 3D objects. Generic
 * shapes: no brand, no real product.
 *
 * Colours come from the theme tokens (faces are `--fg` mixed into the surface at three
 * strengths: lit top, mid left, dark right), with the accent only on the "active" detail
 * (CPU core, RAM chips, fan hubs, SSD light). Motion lives in index.css (`.bench-art`): on hover
 * or while `active` the art lifts, fans spin and the chips light in sequence; nothing moves
 * with reduced motion.
 */

type Vec = readonly [number, number];
const COS = Math.cos(Math.PI / 6);
const SIN = 0.5;

/** World (x right-down, y left-down, z up) → SVG point, unit `s` px, origin at (ox, oy). */
function iso(x: number, y: number, z: number, s: number, ox: number, oy: number): Vec {
  return [ox + (x - y) * COS * s, oy + (x + y) * SIN * s - z * s];
}

const pts = (p: readonly Vec[]) => p.map(([a, b]) => `${a.toFixed(1)},${b.toFixed(1)}`).join(" ");

type Shade = "metal" | "dark" | "board" | "accent" | "gold";

const FILL: Record<Shade, readonly [top: string, left: string, right: string]> = {
  metal: ["var(--art-metal-top)", "var(--art-metal-left)", "var(--art-metal-right)"],
  dark: ["var(--art-dark-top)", "var(--art-dark-left)", "var(--art-dark-right)"],
  board: ["var(--art-board-top)", "var(--art-board-left)", "var(--art-board-right)"],
  accent: ["var(--accent)", "color-mix(in srgb, var(--accent) 70%, black)", "color-mix(in srgb, var(--accent) 55%, black)"],
  gold: ["var(--art-gold)", "color-mix(in srgb, var(--art-gold) 80%, black)", "color-mix(in srgb, var(--art-gold) 65%, black)"],
};

interface BoxProps {
  at: readonly [number, number, number];
  size: readonly [number, number, number];
  shade: Shade;
  className?: string;
  /** Draw only the top face (decals, flat parts). */
  flat?: boolean;
}

function makeBox(s: number, ox: number, oy: number) {
  return function Box({ at: [x, y, z], size: [dx, dy, dz], shade, className, flat = false }: BoxProps) {
    const p = (a: number, b: number, c: number) => iso(a, b, c, s, ox, oy);
    const [top, left, right] = FILL[shade];
    const t = z + dz;
    return (
      <g className={className}>
        {!flat && (
          <>
            {/* Front-left face (y = y + dy) and front-right face (x = x + dx). */}
            <polygon points={pts([p(x, y + dy, z), p(x + dx, y + dy, z), p(x + dx, y + dy, t), p(x, y + dy, t)])} fill={left} />
            <polygon points={pts([p(x + dx, y, z), p(x + dx, y + dy, z), p(x + dx, y + dy, t), p(x + dx, y, t)])} fill={right} />
          </>
        )}
        <polygon
          points={pts([p(x, y, t), p(x + dx, y, t), p(x + dx, y + dy, t), p(x, y + dy, t)])}
          fill={top}
          stroke="var(--art-edge)"
          strokeWidth={0.6}
        />
      </g>
    );
  };
}

/** A circle lying on a horizontal plane at height z: an ellipse 0.577 as tall as wide. */
function isoCircle(cx: number, cy: number, z: number, r: number, s: number, ox: number, oy: number) {
  const [x, y] = iso(cx, cy, z, s, ox, oy);
  return { cx: x, cy: y, rx: r * s * COS * Math.SQRT2, ry: r * s * SIN * Math.SQRT2 };
}

const VIEW_W = 200;
const VIEW_H = 130;

function Frame({ label, shadowW, children }: { label: string; shadowW: number; children: ReactNode }) {
  const id = `art-shadow-${useId().replace(/:/g, "")}`;
  return (
    <svg viewBox={`22 6 ${VIEW_W - 44} ${VIEW_H - 6}`} role="img" aria-label={label || undefined} aria-hidden={label ? undefined : true} className="bench-art block h-full w-full overflow-visible">
      <defs>
        <radialGradient id={id}>
          <stop offset="0%" stopColor="var(--art-shadow)" />
          <stop offset="100%" stopColor="var(--art-shadow)" stopOpacity={0} />
        </radialGradient>
      </defs>
      <ellipse cx={VIEW_W / 2} cy={VIEW_H - 12} rx={shadowW / 2} ry={8} className="bench-art-shadow" fill={`url(#${id})`} />
      <g className="bench-art-body">{children}</g>
    </svg>
  );
}

function Cpu() {
  const s = 7.2;
  const Box = makeBox(s, 100, 26);
  return (
    <>
      <Box at={[0, 0, 0]} size={[10, 10, 0.7]} shade="board" />
      {/* Contact pads along the two visible edges. */}
      {Array.from({ length: 9 }, (_, i) => (
        <Box key={`a${i}`} at={[0.6 + i * 1.0, 9.6, 0.7]} size={[0.5, 0.3, 0]} shade="gold" flat />
      ))}
      {Array.from({ length: 9 }, (_, i) => (
        <Box key={`b${i}`} at={[9.6, 0.6 + i * 1.0, 0.7]} size={[0.3, 0.5, 0]} shade="gold" flat />
      ))}
      <Box at={[1.6, 1.6, 0.7]} size={[6.8, 6.8, 0.9]} shade="metal" />
      <Box at={[3.6, 3.6, 1.6]} size={[2.8, 2.8, 0]} shade="accent" flat className="bench-art-glow" />
    </>
  );
}

function Gpu() {
  const s = 5.4;
  const ox = 77;
  const oy = 32;
  const Box = makeBox(s, ox, oy);
  const fans = [4.4, 11.6].map((cx) => isoCircle(cx, 3, 2.2, 2.6, s, ox, oy));
  return (
    <>
      <Box at={[0, 0, 0]} size={[16, 6, 0.4]} shade="board" />
      <Box at={[0, 5.6, 0.4]} size={[3.2, 0.4, 0]} shade="gold" flat />
      <Box at={[0.3, 0.3, 0.4]} size={[15.4, 5.4, 1.8]} shade="dark" />
      {fans.map((f, i) => (
        <g key={i}>
          <ellipse {...f} fill="var(--art-fan)" />
          <ellipse {...f} rx={f.rx * 0.82} ry={f.ry * 0.82} fill="none" stroke="var(--art-dark-top)" strokeWidth={2.4} strokeDasharray="5 3" className="bench-art-spin" />
          <ellipse {...f} rx={f.rx * 0.25} ry={f.ry * 0.25} fill="var(--accent)" className="bench-art-glow" />
        </g>
      ))}
      <Box at={[15.7, 0.3, 0.4]} size={[0.3, 5.4, 1.8]} shade="metal" />
    </>
  );
}

function Memory() {
  const s = 6.4;
  const ox = 55;
  const oy = 44;
  const Box = makeBox(s, ox, oy);
  return (
    <>
      <Box at={[0, 0, 0]} size={[17, 0.5, 4.6]} shade="board" />
      {/* Gold contacts along the bottom edge of the visible face. */}
      {Array.from({ length: 22 }, (_, i) => (
        <polygon
          key={i}
          points={pts([
            iso(0.5 + i * 0.73, 0.5, 0.15, s, ox, oy),
            iso(0.95 + i * 0.73, 0.5, 0.15, s, ox, oy),
            iso(0.95 + i * 0.73, 0.5, 0.75, s, ox, oy),
            iso(0.5 + i * 0.73, 0.5, 0.75, s, ox, oy),
          ])}
          fill="var(--art-gold)"
        />
      ))}
      {[0, 1, 2, 3].map((i) => (
        <Box key={i} at={[1 + i * 4, 0.5, 1.4]} size={[3, 0.35, 2.4]} shade="dark" className={`bench-art-chip bench-art-chip-${i}`} />
      ))}
    </>
  );
}

function Ssd() {
  const s = 6;
  const Box = makeBox(s, 67, 27);
  return (
    <>
      <Box at={[0, 0, 0]} size={[17, 4.4, 0.35]} shade="board" />
      <Box at={[0, 0.6, 0.35]} size={[1.4, 3.2, 0]} shade="gold" flat />
      <Box at={[2.2, 0.7, 0.35]} size={[3, 3, 0.5]} shade="dark" />
      <Box at={[6.2, 0.6, 0.35]} size={[4.2, 3.2, 0.45]} shade="dark" />
      <Box at={[11.2, 0.6, 0.35]} size={[4.2, 3.2, 0.45]} shade="dark" />
      <Box at={[6.6, 1.2, 0.8]} size={[3.4, 2, 0]} shade="metal" flat />
      <Box at={[3.2, 1.7, 0.85]} size={[1, 1, 0]} shade="accent" flat className="bench-art-glow" />
      <Box at={[16.2, 1.7, 0.35]} size={[0.5, 1, 0.2]} shade="metal" />
    </>
  );
}

const ART: Record<BenchComponent, { draw: () => ReactNode; shadowW: number }> = {
  cpu: { draw: Cpu, shadowW: 120 },
  gpu: { draw: Gpu, shadowW: 130 },
  memory: { draw: Memory, shadowW: 130 },
  storage: { draw: Ssd, shadowW: 130 },
};

/** `active`: plays the hover motion permanently (e.g. while this component's test runs). */
export const BenchIllustration = memo(function BenchIllustration({
  component,
  label,
  active = false,
  className = "",
}: {
  component: BenchComponent;
  label: string;
  active?: boolean;
  className?: string;
}) {
  const { draw: Art, shadowW } = ART[component];
  return (
    <div className={`bench-art-wrap h-full ${active ? "is-active" : ""} ${className}`}>
      <Frame label={label} shadowW={shadowW}>
        <Art />
      </Frame>
    </div>
  );
});
