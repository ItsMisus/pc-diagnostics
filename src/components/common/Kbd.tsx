import type { ReactNode } from "react";

/** A keyboard shortcut label ("⌘1", "Ctrl+B"), e.g. in tooltips and menus. Use `shortcutLabel()` for the text. */
export function Kbd({ children, className = "" }: { children: ReactNode; className?: string }) {
  return (
    <kbd className={`inline-flex h-[18px] items-center rounded-xs border border-line bg-control px-1 text-2xs font-medium text-fg-subtle tabular ${className}`}>
      {children}
    </kbd>
  );
}
