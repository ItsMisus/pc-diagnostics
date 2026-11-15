import { m } from "motion/react";
import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";
import { Icon, type IconName } from "@/components/common/Icon";
import { Tooltip } from "@/components/common/Tooltip";
import { spring } from "@/lib/motion";

export interface ProfileOption {
  id: string;
  name: string;
  /** One short line under the name. */
  hint?: string;
  icon: IconName;
  /** Cannot be selected here; `disabledReason` says why (tooltip and accessible description). */
  disabled?: boolean;
  disabledReason?: string;
  /** Small trailing mark, e.g. "Personalizzato". */
  badge?: ReactNode;
}

/**
 * The profile selector of the control centers (Performance, Cooling, Lighting, Gaming): large
 * satin tiles in a radio group. The selected tile lifts onto the widget surface with an accent
 * ring and its glyph lights up; the ring slides between tiles with the indicator spring (no
 * motion with reduced motion, MotionProvider). Arrow keys move the selection like a native
 * radio group.
 *
 * Selecting does not apply anything by itself unless the page says so: the page decides
 * whether a choice applies at once (an OS mode) or needs "Applica".
 */
export function ProfileSwitcher({
  label,
  options,
  value,
  onChange,
  columns = 3,
  trailing,
}: {
  /** Accessible name of the group. */
  label: string;
  options: readonly ProfileOption[];
  /** Selected id, or null when the current state matches no profile. */
  value: string | null;
  onChange: (id: string) => void;
  /** Tiles per row from 640 px up (one or two below). */
  columns?: 2 | 3 | 4;
  /** Extra tile at the end, e.g. "Nuovo profilo". */
  trailing?: ReactNode;
}) {
  const layoutId = `profile-ring-${useId()}`;
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const cols = { 2: "sm:grid-cols-2", 3: "sm:grid-cols-2 lg:grid-cols-3", 4: "sm:grid-cols-2 lg:grid-cols-4" }[columns];

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const dir = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (dir === 0) return;
    e.preventDefault();
    const enabled = options.map((o, i) => ({ o, i })).filter(({ o }) => !o.disabled);
    if (enabled.length === 0) return;
    const at = enabled.findIndex(({ o }) => o.id === value);
    const next = enabled[(at + dir + enabled.length) % enabled.length];
    if (!next) return;
    onChange(next.o.id);
    refs.current[next.i]?.focus();
  };

  const focusable = options.some((o) => o.id === value && !o.disabled) ? value : (options.find((o) => !o.disabled)?.id ?? null);

  return (
    <div role="radiogroup" aria-label={label} onKeyDown={onKeyDown} className={`grid grid-cols-1 gap-3 ${cols}`}>
      {options.map((o, i) => {
        const selected = o.id === value;
        const tile = (
          <button
            key={o.id}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-disabled={o.disabled || undefined}
            aria-description={o.disabled ? o.disabledReason : undefined}
            tabIndex={o.id === focusable ? 0 : -1}
            onClick={() => !o.disabled && !selected && onChange(o.id)}
            className={`group relative flex w-full min-w-0 items-start gap-3 rounded-2xl p-4 text-start transition-[background-color,box-shadow,opacity,scale] duration-200 ease-out active:scale-[0.985] ${
              selected ? "surface-primary" : "satin hover:bg-control-hover"
            } ${o.disabled ? "cursor-not-allowed opacity-55 active:scale-100" : ""}`}
          >
            {selected && (
              <m.span
                layoutId={layoutId}
                aria-hidden="true"
                className="pointer-events-none absolute inset-0 rounded-2xl shadow-[inset_0_0_0_1.5px_var(--accent)]"
                transition={spring.indicator}
              />
            )}
            <span
              aria-hidden="true"
              className={`grid size-10 shrink-0 place-items-center rounded-full transition-colors duration-200 ${
                selected ? "bg-accent-solid text-accent-fg shadow-[inset_0_1px_0_0_rgb(255_255_255/0.22)]" : "satin text-fg-muted group-hover:text-fg"
              }`}
            >
              <Icon name={o.icon} size={20} />
            </span>
            <span className="min-w-0 flex-1">
              <span className="flex items-center gap-2">
                <span className="truncate font-display text-md font-semibold text-fg">{o.name}</span>
                {o.badge}
              </span>
              {o.hint && <span className="mt-0.5 block text-xs text-fg-muted">{o.hint}</span>}
            </span>
            {selected && <Icon name="check" size={16} className="mt-0.5 shrink-0 text-accent-text" aria-hidden="true" />}
          </button>
        );
        return o.disabled && o.disabledReason ? (
          <Tooltip key={o.id} content={o.disabledReason} multiline className="w-full">
            {tile}
          </Tooltip>
        ) : (
          tile
        );
      })}
      {trailing}
    </div>
  );
}
