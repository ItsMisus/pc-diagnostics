import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "sm" | "md";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-accent-solid text-accent-fg shadow-xs hover:bg-accent-solid-hover",
  secondary: "border border-line bg-surface text-fg shadow-xs hover:border-line-strong hover:bg-surface-2",
  ghost: "text-fg-muted hover:bg-surface-2 hover:text-fg",
  danger: "border border-line bg-surface text-bad shadow-xs hover:border-bad/40 hover:bg-bad-soft",
};

const SIZES: Record<Size, string> = {
  sm: "h-7 gap-1.5 rounded-md px-2.5 text-xs",
  md: "h-8 gap-2 rounded-md px-3 text-[13px]",
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
}

/**
 * The one button. Press feedback is a 3% scale-down (transform only); disabled buttons
 * neither scale nor change on hover.
 */
export function Button({ variant = "secondary", size = "md", className = "", type = "button", ...rest }: ButtonProps) {
  return (
    <button
      type={type}
      className={`pressable inline-flex shrink-0 items-center justify-center font-medium whitespace-nowrap select-none disabled:pointer-events-none disabled:opacity-45 aria-busy:cursor-progress [&_svg]:shrink-0 ${SIZES[size]} ${VARIANTS[variant]} ${className}`}
      {...rest}
    />
  );
}
