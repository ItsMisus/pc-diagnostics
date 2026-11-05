import { m } from "motion/react";
import { useId, useRef, type KeyboardEvent } from "react";
import { spring } from "@/lib/motion";

interface SegmentedProps<T extends string | number> {
  label: string;
  options: readonly { value: T; label: string; disabled?: boolean }[];
  value: T;
  onChange: (value: T) => void;
  size?: "sm" | "md";
}

/**
 * Single-choice control rendered as a radio group. The selection thumb slides between
 * options (shared layout animation); arrow keys move the selection as in a native radio
 * group, with roving tabindex.
 */
export function Segmented<T extends string | number>({ label, options, value, onChange, size = "md" }: SegmentedProps<T>) {
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
      className="inline-flex shrink-0 rounded-lg border border-line bg-surface-2 p-0.5"
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
            tabIndex={selected ? 0 : -1}
            disabled={o.disabled}
            onClick={() => !selected && onChange(o.value)}
            className={`relative rounded-md font-medium whitespace-nowrap tabular transition-colors duration-150 disabled:opacity-40 ${
              size === "sm" ? "h-6 px-2.5 text-2xs" : "h-7 px-3 text-xs"
            } ${selected ? "text-fg" : "text-fg-muted hover:text-fg"}`}
          >
            {selected && (
              <m.span
                layoutId={layoutId}
                transition={spring.indicator}
                aria-hidden="true"
                className="absolute inset-0 rounded-md border border-line-strong bg-surface-3"
              />
            )}
            <span className="relative">{o.label}</span>
          </button>
        );
      })}
    </div>
  );
}
