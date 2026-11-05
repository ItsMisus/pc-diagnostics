import type { Domain } from "@/components/charts/scale";
import type { SortDirection } from "@/components/common/Table";
import { localeTag, type Translator, translatorFor } from "@/i18n";
import { niceCeil } from "@/lib/format";
import type { Metric } from "@/types/generated/Metric";
import type { ProcessCategory } from "@/types/generated/ProcessCategory";
import type { ProcessRow } from "@/types/generated/ProcessRow";

export type SortColumn = "name" | "cpu" | "ram" | "io" | "impact";

export interface SortState {
  column: SortColumn;
  direction: SortDirection;
}

/** Backend ranking order: highest impact first. */
export const DEFAULT_SORT: SortState = { column: "impact", direction: "desc" };

/** Name sorts A→Z first; numeric columns start with the largest value. */
export function nextSort(current: SortState, column: SortColumn): SortState {
  if (current.column === column) {
    return { column, direction: current.direction === "asc" ? "desc" : "asc" };
  }
  return { column, direction: column === "name" ? "asc" : "desc" };
}

const value = (m: Metric<number>): number | null => (m.status === "available" ? m.value : null);

/**
 * Logical I/O (read + write) as one metric. Available only when both directions are:
 * a partial sum would understate the rate. The unavailable side explains the gap.
 */
export function ioTotal(row: ProcessRow): Metric<number> {
  const r = row.ioReadBytesPerSec;
  const w = row.ioWriteBytesPerSec;
  if (r.status !== "available") return r;
  if (w.status !== "available") return w;
  return { status: "available", value: r.value + w.value, unit: "bytesPerSecond", source: r.source, at: Math.max(r.at, w.at) };
}

/**
 * Sort key of a row. `null` = not measured (unavailable metric): such rows always go last,
 * whatever the direction. Protected processes (`accessible: false`) still have real metrics
 * from the kernel snapshot, so their score is sorted normally.
 */
export function sortValue(row: ProcessRow, column: SortColumn): number | string | null {
  switch (column) {
    case "name":
      return row.name;
    case "cpu":
      return value(row.cpuPercent);
    case "ram":
      return value(row.workingSetBytes);
    case "io":
      return value(ioTotal(row));
    case "impact":
      return row.pressure.impact;
  }
}

const collators = new Map<string, Intl.Collator>();

/** Name collator of the current language (cached per locale). */
function collator(): Intl.Collator {
  const tag = localeTag();
  let c = collators.get(tag);
  if (!c) {
    c = new Intl.Collator(tag, { sensitivity: "base", numeric: true });
    collators.set(tag, c);
  }
  return c;
}

/**
 * Stable sort: ties (and two unmeasured rows) keep the incoming order, which is the
 * backend ranking. Returns a new array; the input is never mutated.
 */
export function sortRows(rows: readonly ProcessRow[], sort: SortState): ProcessRow[] {
  const dir = sort.direction === "asc" ? 1 : -1;
  const names = collator();
  return rows
    .map((row, index) => ({ row, index, key: sortValue(row, sort.column) }))
    .sort((a, b) => {
      if (a.key === null || b.key === null) {
        if (a.key === b.key) return a.index - b.index;
        return a.key === null ? 1 : -1;
      }
      const c =
        typeof a.key === "string" && typeof b.key === "string"
          ? names.compare(a.key, b.key)
          : (a.key as number) - (b.key as number);
      return c !== 0 ? dir * c : a.index - b.index;
    })
    .map((x) => x.row);
}

/** Case-insensitive substring match on the process name. Empty query: same array. */
export function filterRows(rows: readonly ProcessRow[], query: string): readonly ProcessRow[] {
  const tag = localeTag();
  const q = query.trim().toLocaleLowerCase(tag);
  if (!q) return rows;
  return rows.filter((r) => r.name.toLocaleLowerCase(tag).includes(q));
}

function sameMetric(a: Metric<number>, b: Metric<number>): boolean {
  if (a.status === "available" && b.status === "available") return a.value === b.value;
  return a.status === b.status;
}

/**
 * True when two rows would render identically in the table. Every snapshot carries new
 * objects, so `React.memo` rows use this to skip processes whose values did not change.
 */
export function sameRowDisplay(a: ProcessRow, b: ProcessRow): boolean {
  return (
    a.key === b.key &&
    a.name === b.name &&
    a.category === b.category &&
    a.accessible === b.accessible &&
    a.isSelf === b.isSelf &&
    a.pressure.impact === b.pressure.impact &&
    a.impactPeak === b.impactPeak &&
    sameMetric(a.cpuPercent, b.cpuPercent) &&
    sameMetric(a.workingSetBytes, b.workingSetBytes) &&
    sameMetric(a.ioReadBytesPerSec, b.ioReadBytesPerSec) &&
    sameMetric(a.ioWriteBytesPerSec, b.ioWriteBytesPerSec)
  );
}

/** Category label in the current language; components pass their `useT("processes")`. */
export function categoryLabel(category: ProcessCategory, t: Translator<"processes"> = translatorFor("processes")): string {
  return t(`category_${category}`);
}

/**
 * y domain starting at zero with a "nice" ceiling above the largest value, never below
 * `minCeil` (so an idle process does not turn noise into a full-height line).
 */
export function zeroBasedDomain(values: readonly (number | null)[], minCeil: number): Domain {
  let max = 0;
  for (const v of values) if (v !== null && Number.isFinite(v) && v > max) max = v;
  return [0, Math.max(minCeil, niceCeil(max))];
}

/** Weight as a share of the total, in percent (the backend's weights sum to 1). */
export function weightShare(weight: number, all: readonly number[]): number {
  const sum = all.reduce((a, b) => a + b, 0);
  return sum > 0 ? (weight / sum) * 100 : 0;
}
