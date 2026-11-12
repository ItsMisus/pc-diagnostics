import { ICON_PATHS, type IconName } from "./iconPaths";

export { ICON_NAMES, isIconName, type IconName } from "./iconPaths";

interface IconProps {
  name: IconName;
  /** Rendered size in px (the grid is 16 units). 16 in body text and controls, 14 in small controls, 20 in headers. */
  size?: number;
  className?: string;
  strokeWidth?: number;
  /**
   * Accessible name. Omit it (the default) for decorative icons next to text: the icon is
   * then hidden from assistive technology.
   */
  title?: string;
}

/**
 * The app's single icon family (`iconPaths.ts`). Stroke icons in `currentColor`, so they take
 * the text color of their context.
 */
export function Icon({ name, size = 16, className, strokeWidth = 1.5, title }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      focusable="false"
      className={className}
      {...(title ? { role: "img", "aria-label": title } : { "aria-hidden": true })}
    >
      {title && <title>{title}</title>}
      <path d={ICON_PATHS[name]} />
    </svg>
  );
}
