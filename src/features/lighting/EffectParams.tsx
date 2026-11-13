import { useState, type ReactNode } from "react";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { Segmented } from "@/components/common/Segmented";
import { useT } from "@/i18n";
import { formatNumber } from "@/lib/format";
import type { LightingColor } from "@/types/generated/LightingColor";
import type { LightingDirection } from "@/types/generated/LightingDirection";
import type { LightingEffect } from "@/types/generated/LightingEffect";
import { ColorPicker } from "./ColorPicker";
import { rgbCss, toHex } from "./colorMath";
import { Slider } from "./Slider";

const MAX_COLORS = 8;

function colorsOf(e: LightingEffect): LightingColor[] {
  if (e.kind === "static" || e.kind === "breathing") return [e.color];
  if (e.kind === "wave" || e.kind === "gradient" || e.kind === "colorCycle") return e.colors;
  return [];
}

function withColors(e: LightingEffect, colors: LightingColor[]): LightingEffect {
  if (e.kind === "static" || e.kind === "breathing") return { ...e, color: colors[0] ?? e.color };
  if (e.kind === "wave" || e.kind === "gradient" || e.kind === "colorCycle") return { ...e, colors };
  return e;
}

/**
 * The parameters of the chosen effect: its colors (one, or a palette of up to eight with an
 * active slot the picker edits), speed, breathing period and wave direction.
 */
export function EffectParams({
  effect,
  onChange,
  recent,
  onRemember,
  colorNote,
}: {
  effect: LightingEffect;
  onChange: (e: LightingEffect) => void;
  recent: readonly LightingColor[];
  onRemember: (c: LightingColor) => void;
  /** Set when every detected device is single-color: color is ignored there. */
  colorNote: string | null;
}) {
  const t = useT("lighting");
  const [slot, setSlot] = useState(0);
  const colors = colorsOf(effect);
  const multi = effect.kind === "wave" || effect.kind === "gradient" || effect.kind === "colorCycle";
  const active = Math.min(slot, Math.max(0, colors.length - 1));

  const setColor = (c: LightingColor) => {
    const next = [...colors];
    next[active] = c;
    onChange(withColors(effect, next));
  };

  if (effect.kind === "off") {
    return <p className="text-sm text-fg-muted">{t("paramsOff")}</p>;
  }

  return (
    <div className="flex flex-col gap-5">
      {multi && (
        <div className="flex flex-col gap-2">
          <p className="eyebrow">{t("paramsPalette")}</p>
          <div role="radiogroup" aria-label={t("paramsPalette")} className="flex flex-wrap items-center gap-2">
            {colors.map((c, i) => (
              <button
                key={`${i}-${toHex(c)}`}
                type="button"
                role="radio"
                aria-checked={i === active}
                aria-label={t("paramsSlot", { n: i + 1, hex: toHex(c).toUpperCase() })}
                onClick={() => setSlot(i)}
                className={`pressable size-8 rounded-full shadow-[inset_0_0_0_1px_var(--line-strong)] transition-[box-shadow,background-color] duration-200 ${
                  i === active ? "shadow-[0_0_0_2px_var(--surface),0_0_0_3.5px_var(--accent)]" : ""
                }`}
                style={{ background: rgbCss(c) }}
              />
            ))}
            <Button
              size="sm"
              variant="ghost"
              iconOnly
              icon="plus"
              aria-label={t("paramsAddColor")}
              disabled={colors.length >= MAX_COLORS}
              onClick={() => {
                onChange(withColors(effect, [...colors, colors[active] ?? { r: 255, g: 255, b: 255 }]));
                setSlot(colors.length);
              }}
            />
            <Button
              size="sm"
              variant="ghost"
              iconOnly
              icon="minus"
              aria-label={t("paramsRemoveColor")}
              disabled={colors.length <= 1}
              onClick={() => {
                onChange(withColors(effect, colors.filter((_, i) => i !== active)));
                setSlot(Math.max(0, active - 1));
              }}
            />
          </div>
        </div>
      )}

      {effect.kind !== "rainbow" && colors[active] && (
        <>
          {colorNote && (
            <p className="flex items-start gap-1.5 text-xs text-fg-muted">
              <Icon name="info" size={14} className="mt-px shrink-0 text-fg-subtle" />
              {colorNote}
            </p>
          )}
          <ColorPicker value={colors[active]} onChange={setColor} onCommit={onRemember} recent={recent} />
        </>
      )}

      {"speed" in effect && (
        <ParamRow label={t("paramsSpeed")} value={t("paramsSpeedValue", { n: effect.speed })}>
          <Slider label={t("paramsSpeed")} value={effect.speed} min={1} max={10} valueText={t("paramsSpeedValue", { n: effect.speed })} onChange={(speed) => onChange({ ...effect, speed })} />
        </ParamRow>
      )}

      {effect.kind === "breathing" && (
        <ParamRow label={t("paramsPeriod")} value={t("paramsSeconds", { s: formatNumber(effect.period / 1000, 1) })}>
          <Slider
            label={t("paramsPeriod")}
            value={effect.period}
            min={1000}
            max={12000}
            step={500}
            valueText={t("paramsSeconds", { s: formatNumber(effect.period / 1000, 1) })}
            onChange={(period) => onChange({ ...effect, period })}
          />
        </ParamRow>
      )}

      {effect.kind === "wave" && (
        <ParamRow label={t("paramsDirection")}>
          <Segmented<LightingDirection>
            label={t("paramsDirection")}
            size="sm"
            value={effect.direction}
            onChange={(direction) => onChange({ ...effect, direction })}
            options={[
              { value: "leftToRight", label: t("directionLeftToRight"), icon: "arrowRight" },
              { value: "rightToLeft", label: t("directionRightToLeft"), icon: "chevronLeft" },
            ]}
          />
        </ParamRow>
      )}
    </div>
  );
}

function ParamRow({ label, value, children }: { label: string; value?: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-sm text-fg">{label}</span>
        {value && <span className="text-xs text-fg-muted tabular">{value}</span>}
      </div>
      {children}
    </div>
  );
}
