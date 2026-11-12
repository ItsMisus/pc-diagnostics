import type { ReactNode } from "react";
import { Icon } from "./Icon";
import { useT } from "@/i18n";

interface TableProps {
  /** Accessible caption (visually hidden: the surrounding heading already names it). */
  caption: string;
  /** Plain column headers. Ignored when `head` is given. */
  headers?: readonly string[];
  /** Custom header cells (e.g. `SortHeader`, `Th`), rendered inside the single header row. */
  head?: ReactNode;
  children: ReactNode;
  className?: string;
  /** Keeps the header row visible while the table scrolls inside a fixed-height container. */
  stickyHeader?: boolean;
}

const TH = "pr-4 pb-2 text-2xs font-medium whitespace-nowrap text-fg-subtle";

/**
 * Compact data table on a solid surface: quiet header, hairline row separators, tabular
 * figures. Scrolls horizontally on narrow widths. Numbers are right-aligned (`Td
 * className="text-right"`), names left-aligned. Rows that open something use `Tr interactive`.
 */
export function Table({ caption, headers = [], head, children, className = "", stickyHeader = false }: TableProps) {
  return (
    <div className={`overflow-x-auto ${className}`}>
      <table className="w-full border-collapse text-sm">
        <caption className="sr-only">{caption}</caption>
        <thead className={stickyHeader ? "sticky top-0 z-[1] bg-surface" : undefined}>
          <tr>
            {head ??
              headers.map((h) => (
                <th key={h} scope="col" className={`${TH} text-left`}>
                  {h}
                </th>
              ))}
          </tr>
        </thead>
        <tbody className="tabular">{children}</tbody>
      </table>
    </div>
  );
}

/** Plain column header matching `Table`'s own headers, for custom `head` rows. */
export function Th({ children, align = "left", className = "" }: { children?: ReactNode; align?: "left" | "right"; className?: string }) {
  return (
    <th scope="col" className={`${TH} ${align === "right" ? "text-right" : "text-left"} ${className}`}>
      {children}
    </th>
  );
}

export type SortDirection = "asc" | "desc";

interface SortHeaderProps {
  label: string;
  /** Direction when this column is the active sort, otherwise null. */
  direction: SortDirection | null;
  onSort: () => void;
  align?: "left" | "right";
  /** Explanation shown as a tooltip and appended to the accessible name. */
  title?: string;
  className?: string;
}

/**
 * Sortable column header: a real button (keyboard and screen reader friendly) inside a
 * `th` carrying `aria-sort`. The arrow is decorative; the state is announced via aria-sort.
 */
export function SortHeader({ label, direction, onSort, align = "left", title, className = "" }: SortHeaderProps) {
  const ariaSort = direction === "asc" ? "ascending" : direction === "desc" ? "descending" : "none";
  return (
    <th scope="col" aria-sort={ariaSort} className={`${TH} ${align === "right" ? "text-right" : "text-left"} ${className}`}>
      <button
        type="button"
        onClick={onSort}
        title={title}
        className={`inline-flex items-center gap-1 rounded-xs transition-colors duration-150 hover:text-fg focus-visible:outline-offset-1 ${direction ? "text-fg" : ""} ${
          align === "right" ? "flex-row-reverse" : ""
        }`}
      >
        {label}
        {title && <span className="sr-only">. {title}</span>}
        <Icon
          name="chevronDown"
          size={12}
          strokeWidth={1.75}
          className={`transition-[opacity,rotate] duration-200 ease-out ${direction ? "opacity-100" : "opacity-0"} ${direction === "asc" ? "rotate-180" : ""}`}
        />
      </button>
    </th>
  );
}

/**
 * Body row. `interactive` adds the hover fill (for rows that open a detail on click; the
 * row must also be reachable from the keyboard, e.g. through a button in its first cell);
 * `selected` marks the row whose detail is open.
 */
export function Tr({
  children,
  interactive = false,
  selected = false,
  onClick,
  className = "",
}: {
  children: ReactNode;
  interactive?: boolean;
  selected?: boolean;
  onClick?: () => void;
  className?: string;
}) {
  return (
    <tr
      onClick={onClick}
      aria-selected={interactive ? selected : undefined}
      className={`${interactive ? "cursor-pointer transition-colors duration-150 hover:bg-control" : ""} ${selected ? "bg-accent-soft hover:bg-accent-soft" : ""} ${className}`}
    >
      {children}
    </tr>
  );
}

/** Body cell. Dense by default (dashboard tables); pass `pr-0` etc. via `className` for the last column. */
export function Td({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <td className={`border-t border-line py-2 align-middle ${/(^|\s)pr-/.test(className) ? "" : "pr-4"} ${className}`}>{children}</td>;
}

/** A field the OS did not report. Visible dash, spoken explanation. */
export function NotReported() {
  const t = useT("common");
  return (
    <span className="text-fg-subtle">
      —<span className="sr-only">{t("notReported")}</span>
    </span>
  );
}
