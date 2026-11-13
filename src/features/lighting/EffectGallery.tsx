import { useRef, useState, type KeyboardEvent } from "react";
import { m } from "motion/react";
import { usePrefersReducedMotion } from "@/features/device3d/hooks";
import { useT } from "@/i18n";
import { spring } from "@/lib/motion";
import type { LightingEffect } from "@/types/generated/LightingEffect";
import { rgbCss } from "./colorMath";
import { colorAt, dim } from "./effect";
import { stillTime, useAnimationClock } from "./preview";
import { effectOfKind, type EffectKind } from "./store";

export const EFFECT_KINDS: readonly EffectKind[] = ["static", "breathing", "rainbow", "wave", "gradient", "colorCycle", "off"];

const CELLS = 18;

/** A strip of lamps showing `effect` (canvas, 2× for sharpness). Animated only while `active`. */
export function EffectStrip({ effect, brightness, active, className = "" }: { effect: LightingEffect; brightness: number; active: boolean; className?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const reduced = usePrefersReducedMotion();
  useAnimationClock(
    active && !reduced && effect.kind !== "static" && effect.kind !== "gradient" && effect.kind !== "off",
    stillTime(effect),
    (t) => {
      const canvas = ref.current;
      const ctx = canvas?.getContext("2d");
      if (!canvas || !ctx) return;
      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);
      const gap = 3;
      const cw = (w - gap * (CELLS - 1)) / CELLS;
      for (let i = 0; i < CELLS; i++) {
        const c = dim(colorAt(effect, t, i, CELLS), Math.max(brightness, 35));
        ctx.fillStyle = rgbCss(c);
        ctx.beginPath();
        ctx.roundRect(i * (cw + gap), 0, cw, h, Math.min(cw, h) / 2);
        ctx.fill();
      }
    },
    [JSON.stringify(effect), brightness],
  );
  return <canvas ref={ref} width={360} height={28} aria-hidden="true" className={`block h-3.5 w-full rounded-full bg-surface-3 ${className}`} />;
}

/**
 * Effect tiles in a radio group. Each tile previews its effect with the colors in use; the
 * selected and the hovered tile animate (never with reduced motion), the others show a still
 * frame. Choosing a tile only changes the draft: "Applica" sends it to the devices.
 */
export function EffectGallery({
  effect,
  brightness,
  onChange,
  colorless,
}: {
  effect: LightingEffect;
  brightness: number;
  onChange: (e: LightingEffect) => void;
  /** Only single-color lights detected: effects they cannot show get a note. */
  colorless: boolean;
}) {
  const t = useT("lighting");
  const [hover, setHover] = useState<EffectKind | null>(null);
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const dir = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!dir) return;
    e.preventDefault();
    const at = EFFECT_KINDS.indexOf(effect.kind);
    const next = EFFECT_KINDS[(at + dir + EFFECT_KINDS.length) % EFFECT_KINDS.length]!;
    onChange(effectOfKind(next, effect));
    refs.current[EFFECT_KINDS.indexOf(next)]?.focus();
  };

  return (
    <div role="radiogroup" aria-label={t("effectsLabel")} onKeyDown={onKeyDown} className="grid grid-cols-2 gap-2 @lg:grid-cols-3 @3xl:grid-cols-4">
      {EFFECT_KINDS.map((kind, i) => {
        const selected = effect.kind === kind;
        const preview = selected ? effect : effectOfKind(kind, effect);
        const whiteOnly = colorless && !["off", "static", "breathing"].includes(kind);
        return (
          <button
            key={kind}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            onClick={() => !selected && onChange(effectOfKind(kind, effect))}
            onPointerEnter={() => setHover(kind)}
            onPointerLeave={() => setHover((h) => (h === kind ? null : h))}
            className={`group relative flex min-w-0 flex-col gap-2.5 rounded-lg p-3 text-start transition-[background-color] duration-150 ${selected ? "bg-accent-soft" : "bg-surface-2 hover:bg-surface-3"}`}
          >
            {selected && (
              <m.span layoutId="lighting-effect-ring" aria-hidden="true" className="pointer-events-none absolute inset-0 rounded-lg shadow-[inset_0_0_0_1.5px_var(--accent)]" transition={spring.indicator} />
            )}
            <EffectStrip effect={preview} brightness={brightness} active={selected || hover === kind} />
            <span className="flex min-w-0 flex-col">
              <span className={`truncate text-sm font-semibold ${selected ? "text-accent-text" : "text-fg"}`}>{t(`effect_${kind}`)}</span>
              <span className="truncate text-2xs text-fg-subtle">{whiteOnly ? t("effectPreviewOnlyWhite") : t(`effectHint_${kind}`)}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}
