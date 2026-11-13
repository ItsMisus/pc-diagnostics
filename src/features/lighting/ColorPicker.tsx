import { useEffect, useId, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { useT } from "@/i18n";
import type { LightingColor } from "@/types/generated/LightingColor";
import { hsvToRgb, parseChannel, parseHex, rgbCss, rgbToHsv, sameColor, stepSv, svFromPoint, SWATCHES, toHex, type Hsv } from "./colorMath";
import { Slider } from "./Slider";

/**
 * Advanced color picker: a saturation/value square, a hue slider, hex and R G B fields,
 * preset swatches and the recent colors. Fully keyboard accessible (the square is a 2D slider:
 * arrows, Shift for big steps). The hue is kept while saturation or value is 0, so dragging
 * through black or white does not lose it. Colors the user picks are data: they are drawn as
 * they are, the chrome around them stays on the design tokens.
 */
export function ColorPicker({
  value,
  onChange,
  onCommit,
  recent,
  disabled = false,
}: {
  value: LightingColor;
  onChange: (c: LightingColor) => void;
  /** End of a gesture (drag released, field confirmed, swatch picked): remember it. */
  onCommit: (c: LightingColor) => void;
  recent: readonly LightingColor[];
  disabled?: boolean;
}) {
  const t = useT("lighting");
  const [hsv, setHsv] = useState<Hsv>(() => rgbToHsv(value));
  const [hexText, setHexText] = useState(toHex(value));
  const squareRef = useRef<HTMLDivElement>(null);
  const hintId = useId();

  // Follow outside changes (another slot, a profile) without losing the hue of greys.
  useEffect(() => {
    if (!sameColor(hsvToRgb(hsv), value)) {
      const next = rgbToHsv(value);
      setHsv((h) => (next.s === 0 || next.v === 0 ? { ...next, h: h.h } : next));
    }
    setHexText(toHex(value));
  }, [value.r, value.g, value.b]);

  const update = (next: Hsv) => {
    setHsv(next);
    onChange(hsvToRgb(next));
  };

  const fromPointer = (e: PointerEvent<HTMLDivElement>) => {
    const rect = squareRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0 || rect.height === 0) return;
    update({ ...hsv, ...svFromPoint((e.clientX - rect.left) / rect.width, (e.clientY - rect.top) / rect.height) });
  };

  const onSquareKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const next = stepSv(hsv, e.key, e.shiftKey);
    if (!next) return;
    e.preventDefault();
    update(next);
  };

  const commit = () => onCommit(hsvToRgb(hsv));
  const hueColor = rgbCss(hsvToRgb({ h: hsv.h, s: 1, v: 1 }));
  const channels: { key: "r" | "g" | "b"; label: string }[] = [
    { key: "r", label: "R" },
    { key: "g", label: "G" },
    { key: "b", label: "B" },
  ];

  return (
    <div className={`flex flex-col gap-4 ${disabled ? "pointer-events-none opacity-45" : ""}`} aria-disabled={disabled || undefined}>
      <div
        ref={squareRef}
        role="slider"
        tabIndex={disabled ? -1 : 0}
        aria-label={t("pickerSquare")}
        aria-valuetext={t("pickerSquareValue", { s: Math.round(hsv.s * 100), v: Math.round(hsv.v * 100) })}
        aria-describedby={hintId}
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          e.currentTarget.setPointerCapture(e.pointerId);
          fromPointer(e);
        }}
        onPointerMove={(e) => e.currentTarget.hasPointerCapture(e.pointerId) && fromPointer(e)}
        onPointerUp={(e) => {
          if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
          e.currentTarget.releasePointerCapture(e.pointerId);
          commit();
        }}
        onKeyDown={onSquareKey}
        onKeyUp={(e) => e.key.startsWith("Arrow") && commit()}
        className="relative aspect-[16/10] w-full cursor-crosshair touch-none rounded-lg shadow-[inset_0_0_0_1px_var(--line)] outline-none select-none focus-visible:shadow-[0_0_0_2px_var(--focus)]"
        style={{ background: `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, ${hueColor})` }}
      >
        <span
          aria-hidden="true"
          className="pointer-events-none absolute size-5 -translate-x-1/2 -translate-y-1/2 rounded-full shadow-[0_0_0_2px_white,0_1px_4px_rgb(0_0_0/0.45)]"
          style={{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%`, background: rgbCss(value) }}
        />
      </div>
      <span id={hintId} className="sr-only">
        {t("pickerSquareHint")}
      </span>

      <Slider
        label={t("pickerHue")}
        value={Math.round(hsv.h * 360) % 360}
        min={0}
        max={359}
        valueText={t("pickerHueValue", { deg: Math.round(hsv.h * 360) % 360 })}
        onChange={(deg) => update({ ...hsv, h: deg / 360 })}
        onCommit={commit}
        track="linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00)"
        thumbColor={hueColor}
      />

      <div className="flex flex-wrap items-end gap-3">
        <span
          aria-hidden="true"
          className="size-10 shrink-0 rounded-full shadow-[inset_0_0_0_1px_var(--line-strong)] transition-[background-color] duration-200"
          style={{ background: rgbCss(value) }}
        />
        <label className="flex min-w-0 flex-col gap-1 text-2xs text-fg-subtle">
          {t("pickerHex")}
          <input
            type="text"
            inputMode="text"
            spellCheck={false}
            value={hexText}
            maxLength={7}
            onChange={(e) => {
              setHexText(e.target.value);
              const c = parseHex(e.target.value);
              if (c) onChange(c);
            }}
            onBlur={() => {
              const c = parseHex(hexText);
              if (c) onCommit(c);
              else setHexText(toHex(value));
            }}
            onKeyDown={(e) => e.key === "Enter" && (e.currentTarget as HTMLInputElement).blur()}
            className="h-8 w-24 font-mono text-sm uppercase"
            aria-invalid={parseHex(hexText) === null || undefined}
          />
        </label>
        {channels.map((ch) => (
          <ChannelField key={ch.key} label={ch.label} value={value[ch.key]} onChange={(v) => onChange({ ...value, [ch.key]: v })} onCommit={() => onCommit(value)} />
        ))}
      </div>

      <SwatchRow label={t("pickerSwatches")} colors={SWATCHES} value={value} onPick={(c) => (onChange(c), onCommit(c))} />
      {recent.length > 0 && <SwatchRow label={t("pickerRecent")} colors={recent} value={value} onPick={(c) => (onChange(c), onCommit(c))} />}
    </div>
  );
}

function ChannelField({ label, value, onChange, onCommit }: { label: string; value: number; onChange: (v: number) => void; onCommit: () => void }) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  return (
    <label className="flex flex-col gap-1 text-2xs text-fg-subtle">
      {label}
      <input
        type="text"
        inputMode="numeric"
        value={text}
        maxLength={3}
        onChange={(e) => {
          setText(e.target.value);
          const v = parseChannel(e.target.value);
          if (v !== null) onChange(v);
        }}
        onBlur={() => {
          if (parseChannel(text) === null) setText(String(value));
          else onCommit();
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            const v = Math.min(255, Math.max(0, value + (e.key === "ArrowUp" ? 1 : -1) * (e.shiftKey ? 10 : 1)));
            onChange(v);
          } else if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
        }}
        className="h-8 w-14 text-sm tabular"
      />
    </label>
  );
}

function SwatchRow({ label, colors, value, onPick }: { label: string; colors: readonly LightingColor[]; value: LightingColor; onPick: (c: LightingColor) => void }) {
  const t = useT("lighting");
  return (
    <div className="flex flex-col gap-2">
      <p className="eyebrow">{label}</p>
      <div role="group" aria-label={label} className="flex flex-wrap gap-2">
        {colors.map((c) => {
          const selected = sameColor(c, value);
          return (
            <button
              key={toHex(c)}
              type="button"
              aria-pressed={selected}
              aria-label={t("pickerSwatch", { hex: toHex(c).toUpperCase() })}
              onClick={() => onPick(c)}
              className={`pressable size-7 rounded-full shadow-[inset_0_0_0_1px_var(--line-strong)] transition-[box-shadow,scale] duration-150 hover:scale-110 ${
                selected ? "shadow-[0_0_0_2px_var(--surface),0_0_0_3.5px_var(--accent)]" : ""
              }`}
              style={{ background: rgbCss(c) }}
            />
          );
        })}
      </div>
    </div>
  );
}
