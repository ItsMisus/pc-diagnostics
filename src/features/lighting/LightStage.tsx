import { useMemo, useRef } from "react";
import { Icon, type IconName } from "@/components/common/Icon";
import { ObjectStage } from "@/components/stage3d/ObjectStage";
import { usePrefersReducedMotion } from "@/features/device3d/hooks";
import { useT } from "@/i18n";
import type { LightingEffect } from "@/types/generated/LightingEffect";
import { averageColor, rgbCss } from "./colorMath";
import { dim, effectColors, isDynamic } from "./effect";
import { stillTime, useAnimationClock } from "./preview";
import { lampColors, modelLayout, type StageModel } from "./scene/layouts";

export interface StageEntry {
  key: string;
  model: StageModel;
  /** Shown name: the detected device, or the generic model. */
  name: string;
  /** A generic model used to preview the effect, not hardware of this computer. */
  preview: boolean;
}

export const MODEL_ICON: Record<StageModel, IconName> = { keyboard: "keyboard", mouse: "mouse", fan: "fan", strip: "ledStrip", backlight: "laptop" };

const loadScene = () => import("./scene/LightingScene");

/** Without WebGL: the model seen from above, lamps drawn with the same math (still frame). */
function SvgPreview({ model, effect, brightness, animate }: { model: StageModel; effect: LightingEffect; brightness: number; animate: boolean }) {
  const layout = useMemo(() => modelLayout(model), [model]);
  const ref = useRef<SVGGElement>(null);
  const half = Math.max(layout.width, layout.depth) / 2 + 0.2;
  useAnimationClock(
    animate,
    stillTime(effect),
    (t) => {
      const g = ref.current;
      if (!g) return;
      const colors = lampColors(model, layout, effect, brightness, t);
      colors.forEach((c, i) => g.children[i]?.setAttribute("fill", rgbCss(c)));
    },
    [model, JSON.stringify(effect), brightness],
  );
  return (
    <svg viewBox={`${-half} ${-half} ${half * 2} ${half * 2}`} className="absolute inset-0 m-auto h-full max-h-[80%] w-full max-w-[80%]" aria-hidden="true">
      {layout.ring ? (
        <circle r={1.2} className="fill-surface-3 stroke-line-strong" strokeWidth={0.02} />
      ) : (
        <rect x={-layout.width / 2} y={-layout.depth / 2} width={layout.width} height={layout.depth} rx={model === "mouse" ? 0.6 : 0.08} className="fill-surface-3 stroke-line-strong" strokeWidth={0.02} />
      )}
      <g ref={ref}>
        {layout.lamps.map((l, i) =>
          layout.ring || model === "mouse" || model === "strip" ? (
            <circle key={i} cx={l.x} cy={layout.ring ? -l.y : l.z} r={Math.max(l.w, l.d) / 2} />
          ) : (
            <rect key={i} x={l.x - l.w / 2} y={l.z - l.d / 2} width={l.w} height={l.d} rx={0.025} />
          ),
        )}
      </g>
    </svg>
  );
}

/**
 * The focal stage: the selected device as a procedural 3D model, lit live by the effect being
 * edited (shared effect math), with a soft colored glow behind it that the glass panels pick
 * up. The effect animates only while it is dynamic and motion is allowed; reduced motion shows
 * a still frame. Generic models are marked "Anteprima" so they never read as detected hardware.
 */
export function LightStage({
  entries,
  selected,
  onSelect,
  effect,
  brightness,
  paused,
}: {
  entries: readonly StageEntry[];
  selected: string;
  onSelect: (key: string) => void;
  effect: LightingEffect;
  brightness: number;
  paused: boolean;
}) {
  const t = useT("lighting");
  const reduced = usePrefersReducedMotion();
  const entry = entries.find((e) => e.key === selected) ?? entries[0];
  const playing = isDynamic(effect) && !reduced && !paused;
  const glow = useMemo(() => {
    const cs = (entry?.model === "backlight" ? (effect.kind === "off" ? [] : [{ r: 235, g: 240, b: 255 }]) : effectColors(effect)).map((c) => dim(c, Math.max(30, brightness)));
    if (cs.length === 0) return "transparent";
    const a = cs[0]!;
    const b = cs[Math.floor(cs.length / 2)] ?? a;
    const c = cs[cs.length - 1] ?? a;
    const avg = averageColor(cs);
    return `radial-gradient(60% 55% at 30% 60%, ${rgbCss(a, 0.42)}, transparent 70%), radial-gradient(55% 50% at 70% 55%, ${rgbCss(c, 0.4)}, transparent 70%), radial-gradient(70% 60% at 50% 75%, ${rgbCss(b, 0.3)}, transparent 75%), radial-gradient(90% 70% at 50% 50%, ${rgbCss(avg, 0.16)}, transparent 80%)`;
  }, [effect, brightness, entry?.model]);

  if (!entry) return null;
  const label = entry.preview ? t("stageAriaPreview", { model: entry.name, effect: t(`effect_${effect.kind}`) }) : t("stageAria", { device: entry.name, effect: t(`effect_${effect.kind}`) });

  return (
    <div className="relative flex h-full min-h-[340px] flex-col">
      {/* The light of the effect, behind everything: the glass over the stage picks it up. */}
      <div aria-hidden="true" className="pointer-events-none absolute -inset-6 -z-10 blur-2xl transition-[background] duration-500" style={{ background: glow }} />
      <ObjectStage
        label={label}
        loadScene={loadScene}
        sceneProps={{ model: entry.model, effect, brightness, playing }}
        animate={playing}
        paused={paused}
        camera={{ distance: entry.model === "fan" ? 7.2 : 7.6, height: entry.model === "fan" ? 1.2 : 3.4, fov: 30 }}
        aspectRatio={null}
        className="min-h-[260px] flex-1"
        fallback={<SvgPreview model={entry.model} effect={effect} brightness={brightness} animate={playing} />}
      >
        <div className="absolute start-3 top-3">
          {entry.preview ? (
            <span className="glass inline-flex h-7 items-center gap-1.5 rounded-full px-2.5 text-xs text-fg">
              <Icon name="sparkle" size={14} className="text-fg-subtle" />
              {t("stagePreviewBadge")}
            </span>
          ) : (
            <span className="glass inline-flex h-7 max-w-[16rem] items-center gap-1.5 rounded-full px-2.5 text-xs text-fg">
              <span aria-hidden="true" className="size-1.5 shrink-0 rounded-full bg-ok" />
              <span className="truncate">{t("stageThisComputer")}</span>
            </span>
          )}
        </div>
      </ObjectStage>
      <div role="radiogroup" aria-label={t("stageModels")} className="flex flex-wrap justify-center gap-1.5 px-3 pb-3">
        {entries.map((e) => {
          const on = e.key === entry.key;
          return (
            <button
              key={e.key}
              type="button"
              role="radio"
              aria-checked={on}
              onClick={() => onSelect(e.key)}
              className={`pressable inline-flex h-8 max-w-[14rem] items-center gap-1.5 rounded-full px-3 text-xs transition-colors duration-150 ${
                on ? "bg-accent-soft text-accent-text shadow-[inset_0_0_0_1px_var(--accent)]" : "satin text-fg-muted hover:text-fg"
              }`}
            >
              <Icon name={MODEL_ICON[e.model]} size={14} className="shrink-0" />
              <span className="truncate">{e.name}</span>
              {e.preview && <span className="text-2xs text-fg-subtle">· {t("stagePreviewTag")}</span>}
            </button>
          );
        })}
      </div>
    </div>
  );
}
