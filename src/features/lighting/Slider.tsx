import { useRef, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";

/**
 * A capsule slider (role="slider"): drag or click the track, arrows step by `step` (Shift ×10),
 * Home/End jump to the ends. `onChange` follows the thumb; `onCommit` fires once when the drag
 * or key press ends, for actions that should not run on every pixel (a device write).
 * `track` replaces the default fill (e.g. the hue spectrum).
 */
export function Slider({
  label,
  value,
  min,
  max,
  step = 1,
  onChange,
  onCommit,
  disabled = false,
  valueText,
  track,
  thumbColor,
  describedBy,
  className = "",
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
  disabled?: boolean;
  valueText?: string | undefined;
  track?: string | undefined;
  thumbColor?: string | undefined;
  describedBy?: string | undefined;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const last = useRef(value);
  const fraction = max > min ? (value - min) / (max - min) : 0;

  const snap = (v: number) => {
    const clamped = Math.min(max, Math.max(min, v));
    const stepped = Math.round((clamped - min) / step) * step + min;
    return Number(stepped.toFixed(6));
  };
  const fromPointer = (clientX: number) => {
    const rect = ref.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return value;
    return snap(min + ((clientX - rect.left) / rect.width) * (max - min));
  };
  const emit = (v: number) => {
    last.current = v;
    if (v !== value) onChange(v);
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (disabled || e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    emit(fromPointer(e.clientX));
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (disabled || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
    emit(fromPointer(e.clientX));
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    e.currentTarget.releasePointerCapture(e.pointerId);
    onCommit?.(last.current);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (disabled) return;
    const big = e.shiftKey ? step * 10 : step;
    let next: number | null = null;
    if (e.key === "ArrowRight" || e.key === "ArrowUp") next = snap(value + big);
    else if (e.key === "ArrowLeft" || e.key === "ArrowDown") next = snap(value - big);
    else if (e.key === "Home") next = min;
    else if (e.key === "End") next = max;
    else if (e.key === "PageUp") next = snap(value + step * 10);
    else if (e.key === "PageDown") next = snap(value - step * 10);
    if (next === null) return;
    e.preventDefault();
    emit(next);
  };
  const onKeyUp = (e: KeyboardEvent<HTMLDivElement>) => {
    if (["ArrowRight", "ArrowUp", "ArrowLeft", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(e.key)) onCommit?.(last.current);
  };

  const thumbStyle: CSSProperties = { left: `calc(${fraction * 100}% - ${fraction * 18}px)` };
  if (thumbColor) thumbStyle.background = thumbColor;

  return (
    <div
      ref={ref}
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label={label}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      aria-valuetext={valueText}
      aria-disabled={disabled || undefined}
      aria-describedby={describedBy}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onKeyDown={onKeyDown}
      onKeyUp={onKeyUp}
      className={`relative h-[18px] w-full touch-none rounded-full outline-none select-none focus-visible:shadow-[0_0_0_2px_var(--focus)] ${
        disabled ? "cursor-not-allowed opacity-45" : "cursor-pointer"
      } ${className}`}
    >
      <span
        aria-hidden="true"
        className="absolute inset-x-0 top-1/2 h-1.5 -translate-y-1/2 rounded-full bg-control-track shadow-[inset_0_0_0_1px_var(--line)]"
        style={track ? { background: track } : undefined}
      />
      {!track && (
        <span
          aria-hidden="true"
          className="absolute top-1/2 left-0 h-1.5 -translate-y-1/2 rounded-full bg-accent-solid"
          style={{ width: `calc(${fraction * 100}% - ${fraction * 18}px + 9px)` }}
        />
      )}
      <span
        aria-hidden="true"
        className="absolute top-0 size-[18px] rounded-full bg-white shadow-[0_1px_3px_rgb(0_0_0/0.3),0_0_0_0.5px_rgb(0_0_0/0.08)] transition-[scale] duration-150 active:scale-110"
        style={thumbStyle}
      />
    </div>
  );
}
