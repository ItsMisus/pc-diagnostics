import { m } from "motion/react";
import { useId, useRef, type KeyboardEvent } from "react";
import { spring } from "@/lib/motion";
import { Icon, type IconName } from "./Icon";

export interface SegmentedOption<T extends string | number> {
  value: T;
  label: string;
  disabled?: boolean;
  /** Optional leading glyph. With `iconOnly` the label becomes the accessible name. */
  icon?: IconName;
}

interface SegmentedProps<T extends string | number> {
  /** Accessible name of the group. */
  label: string;
  options: readonly SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  size?: "sm" | "md";
  /** Stretches to the container width, options share it equally. */
  fullWidth?: boolean;
  /** Shows only the icons (each option needs `icon`); labels stay as accessible names. */
  iconOnly?: boolean;
  className?: string;
}

/**
 * Single-choice control rendered as a radio group: a capsule track with a raised glass thumb
 * that slides between options (shared layout animation, spring without overshoot). Arrow keys
 * move the selection as in a native radio group, with roving tabindex. For 2–5 short options;
 * longer lists use a native `<select>`.
 */
export function Segmented<T extends string | number>({
  label,
  options,
  value,
  onChange,
  size = "md",
  fullWidth = false,
  iconOnly = false,
  className = "",
}: SegmentedProps<T>) {
  const layoutId = `segmented-${useId()}`;
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const dir = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (dir === 0) return;
    e.preventDefault();
    const enabled = options.map((o, i) => ({ o, i })).filter(({ o }) => !o.disabled);
    const at = enabled.findIndex(({ o }) => o.value === value);
    const next = enabled[(at + dir + enabled.length) % enabled.length];
    if (!next) return;
    onChange(next.o.value);
    refs.current[next.i]?.focus();
  };

  return (
    <div
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKeyDown}
      className={`${fullWidth ? "flex w-full" : "inline-flex"} shrink-0 rounded-full bg-control-track p-0.5 shadow-[inset_0_0_0_1px_var(--line)] ${className}`}
    >
      {options.map((o, i) => {
        const selected = o.value === value;
        return (
          <button
            key={String(o.value)}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={iconOnly ? o.label : undefined}
            tabIndex={selected ? 0 : -1}
            disabled={o.disabled}
            onClick={() => !selected && onChange(o.value)}
            className={`relative inline-flex items-center justify-center gap-1.5 rounded-full font-medium whitespace-nowrap tabular transition-colors duration-150 focus-visible:outline-offset-0 disabled:opacity-40 ${
              fullWidth ? "min-w-0 flex-1" : ""
            } ${size === "sm" ? `h-6 text-2xs ${iconOnly ? "w-7" : "px-2.5"}` : `h-7 text-xs ${iconOnly ? "w-8" : "px-3"}`} ${
              selected ? "text-fg" : "text-fg-muted hover:text-fg"
            }`}
          >
            {selected && (
              <m.span
                layoutId={layoutId}
                transition={spring.indicator}
                aria-hidden="true"
                className="absolute inset-0 rounded-full border border-control-edge bg-[var(--control-thumb)] shadow-[inset_0_1px_0_0_var(--glass-highlight),var(--elev-1)]"
              />
            )}
            {o.icon && <Icon name={o.icon} size={size === "sm" ? 12 : 14} className="relative" />}
            {!iconOnly && <span className="relative truncate">{o.label}</span>}
          </button>
        );
      })}
    </div>
  );
}
