import { forwardRef, type ButtonHTMLAttributes } from "react";
import { Icon, type IconName } from "./Icon";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

/**
 * Capsule buttons, Liquid Glass inspired:
 * - `primary`: solid azure with a top light, the one main action of a view.
 * - `secondary` (default): satin glass (translucent fill, luminous edge, top highlight).
 * - `ghost`: no chrome until hovered; toolbars, "Show all" links, close buttons.
 * - `danger`: satin with red text; destructive or stopping actions (Stop, Delete key).
 */
const VARIANTS: Record<ButtonVariant, string> = {
  primary:
    "border border-transparent bg-accent-solid text-accent-fg shadow-[inset_0_1px_0_0_rgb(255_255_255/0.22),var(--elev-1)] bg-[linear-gradient(180deg,rgb(255_255_255/0.14),transparent_60%)] hover:bg-accent-solid-hover active:brightness-95",
  secondary: "satin text-fg hover:bg-control-hover active:bg-control-active",
  ghost: "border border-transparent text-fg-muted hover:bg-control hover:text-fg active:bg-control-active",
  danger: "satin text-bad hover:bg-bad-soft active:bg-control-active",
};

const SIZES: Record<ButtonSize, string> = {
  sm: "h-7 gap-1.5 px-3 text-xs",
  md: "h-8 gap-2 px-3.5 text-sm",
  lg: "h-10 gap-2 px-5 text-base",
};

/** Square (round) icon-only buttons. */
const ICON_SIZES: Record<ButtonSize, string> = {
  sm: "size-7 text-xs",
  md: "size-8 text-sm",
  lg: "size-10 text-base",
};

const ICON_PX: Record<ButtonSize, number> = { sm: 14, md: 16, lg: 16 };

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Leading icon, sized for the button. Children can still contain their own `<Icon>`. */
  icon?: IconName;
  /**
   * Icon-only round button. Requires `aria-label` (and usually a `Tooltip`); `icon` gives
   * the glyph, or pass a single `<Icon>` as children.
   */
  iconOnly?: boolean;
  /** Work in progress: shows a spinner in place of the icon, sets aria-busy and blocks clicks. */
  loading?: boolean;
}

/**
 * The one button. States: hover lightens the fill, press scales to 97 % (transform only),
 * keyboard focus shows the accent ring, disabled fades to 45 % and ignores the pointer,
 * `loading` keeps the label and shows a spinner.
 */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "md", icon, iconOnly = false, loading = false, className = "", type = "button", children, disabled, onClick, "aria-busy": ariaBusy, ...rest },
  ref,
) {
  const shape = iconOnly ? ICON_SIZES[size] : SIZES[size];
  return (
    <button
      ref={ref}
      type={type}
      disabled={disabled}
      aria-busy={loading || ariaBusy || undefined}
      onClick={loading ? undefined : onClick}
      className={`pressable inline-flex shrink-0 items-center justify-center rounded-full font-medium whitespace-nowrap select-none disabled:pointer-events-none disabled:opacity-45 aria-busy:cursor-progress [&_svg]:shrink-0 ${shape} ${VARIANTS[variant]} ${className}`}
      {...rest}
    >
      {loading ? <ButtonSpinner size={ICON_PX[size]} /> : icon && <Icon name={icon} size={ICON_PX[size]} />}
      {children}
    </button>
  );
});

function ButtonSpinner({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true" className="shrink-0 animate-spin-slow">
      <circle cx="8" cy="8" r="6" stroke="currentColor" strokeOpacity={0.3} strokeWidth={1.75} />
      <path d="M14 8a6 6 0 0 0-6-6" stroke="currentColor" strokeWidth={1.75} strokeLinecap="round" />
    </svg>
  );
}
