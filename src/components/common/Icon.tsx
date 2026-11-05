/**
 * 16px stroke icon set (inline SVG, no icon font, no network). One visual language:
 * 1.5 stroke, round caps and joins, drawn on a 16-unit grid with a 1.5-unit margin.
 */
const PATHS = {
  overview: "M2.75 2.75h4v4h-4zM9.25 2.75h4v4h-4zM2.75 9.25h4v4h-4zM9.25 9.25h4v4h-4z",
  hardware:
    "M4.75 4.75h6.5v6.5h-6.5zM6.75 6.75h2.5v2.5h-2.5zM2.5 6.5h2.25M2.5 9.5h2.25M11.25 6.5h2.25M11.25 9.5h2.25M6.5 2.5v2.25M9.5 2.5v2.25M6.5 11.25v2.25M9.5 11.25v2.25",
  benchmarks: "M2.5 13.25h11M4.5 13.25v-4M8 13.25v-8.5M11.5 13.25v-6",
  processes: "M2.75 4h10.5M2.75 8h10.5M2.75 12h6",
  drivers: "M6 2.25v3M10 2.25v3M4.25 5.25h7.5v2.5a3.75 3.75 0 0 1-7.5 0zM8 11.5v2.25",
  diagnostics: "M1.75 8h2.75l1.75-4.5 3.5 9 1.75-4.5h2.75",
  history: "M8 4.75V8l2.25 1.5M2.6 8a5.4 5.4 0 1 0 1.58-3.82M2.5 2.5v2.75h2.75",
  settings:
    "M8 10.25a2.25 2.25 0 1 0 0-4.5 2.25 2.25 0 0 0 0 4.5zM8 1.5v1.75M8 12.75v1.75M1.5 8h1.75M12.75 8h1.75M3.4 3.4l1.24 1.24M11.36 11.36l1.24 1.24M3.4 12.6l1.24-1.24M11.36 4.64l1.24-1.24",
  collapse: "M2.75 2.75h10.5v10.5H2.75zM6.25 2.75v10.5M10.5 6.25L8.75 8l1.75 1.75",
  expand: "M2.75 2.75h10.5v10.5H2.75zM6.25 2.75v10.5M9 6.25L10.75 8 9 9.75",
  info: "M8 14.25a6.25 6.25 0 1 0 0-12.5 6.25 6.25 0 0 0 0 12.5zM8 7.25v4M8 4.85v.01",
  alert: "M8 2.25l6.25 11H1.75zM8 6.5v3M8 11.4v.01",
  lock: "M4.25 7.25h7.5v6.5h-7.5zM5.75 7.25v-2a2.25 2.25 0 0 1 4.5 0v2",
  refresh: "M13.25 8a5.25 5.25 0 1 1-1.54-3.71M13.25 2.5v2.75H10.5",
  check: "M3.25 8.25l3 3 6.5-6.5",
  close: "M4.25 4.25l7.5 7.5M11.75 4.25l-7.5 7.5",
  chevronDown: "M4.5 6.25L8 9.75l3.5-3.5",
  chevronRight: "M6.25 4.5L9.75 8l-3.5 3.5",
  search: "M7.25 12.25a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM13.75 13.75l-3-3",
  external: "M9.5 2.75h3.75V6.5M13.25 2.75L7.5 8.5M11.25 9.5v3.75h-8.5v-8.5H6.5",
  sparkle: "M8 1.75l1.4 3.85 3.85 1.4-3.85 1.4L8 12.25l-1.4-3.85L2.75 7l3.85-1.4zM12.75 11.25v2.5M11.5 12.5H14",
  stop: "M4.25 4.25h7.5v7.5h-7.5z",
  play: "M5 3.25l7.5 4.75L5 12.75z",
  download: "M8 2.5v8M4.75 7.25L8 10.5l3.25-3.25M2.75 13.25h10.5",
  key: "M5.5 13a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM7.6 7.9l5.15-5.15M10.75 4.75l1.75 1.75",
  pulse: "M8 8m-2.25 0a2.25 2.25 0 1 0 4.5 0 2.25 2.25 0 1 0-4.5 0",
} as const;

export type IconName = keyof typeof PATHS;

interface IconProps {
  name: IconName;
  size?: number;
  className?: string;
  strokeWidth?: number;
}

export function Icon({ name, size = 16, className, strokeWidth = 1.5 }: IconProps) {
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
      aria-hidden="true"
      focusable="false"
      className={className}
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
