import type { ReactNode } from "react";
import { Icon } from "./Icon";
import { useT } from "@/i18n";

interface TableProps {
  /** Accessible caption (visually hidden: the surrounding heading already names it). */
  caption: string;
  /** Plain column headers. Ignored when `head` is given. */
  headers?: readonly string[];
  /** Custom header cells (e.g. `SortHeader`), rendered inside the single header row. */
  head?: ReactNode;
  children: ReactNode;
  className?: string;
}

const TH = "pr-4 pb-2 text-2xs font-medium whitespace-nowrap text-fg-subtle";

/** Compact data table with thin row separators; scrolls horizontally on narrow widths. */
export function Table({ caption, headers = [], head, children, className = "" }: TableProps) {
  return (
    <div className={`overflow-x-auto ${className}`}>
      <table className="w-full border-collapse text-sm">
        <caption className="sr-only">{caption}</caption>
        <thead>
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
        className={`inline-flex items-center gap-1 rounded transition-colors focus-visible:outline-offset-[-1px] duration-150 hover:text-fg ${direction ? "text-fg" : ""} ${
          align === "right" ? "flex-row-reverse" : ""
        }`}
      >
        {label}
        {title && <span className="sr-only">. {title}</span>}
        <Icon
          name="chevronDown"
          size={12}
          strokeWidth={1.75}
          className={`transition-[opacity,rotate] duration-200 ease-out ${direction ? "opacity-100" : "opacity-0"} ${
            direction === "asc" ? "rotate-180" : ""
          }`}
        />
      </button>
    </th>
  );
}

/** Body cell. Dense by default (dashboard tables); pass `pr-0` etc. via `className` for the last column. */
export function Td({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <td className={`border-t border-line py-1.5 align-middle ${/(^|\s)pr-/.test(className) ? "" : "pr-4"} ${className}`}>{children}</td>;
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
