import { useViewMode } from "@/hooks/useViewMode";
import { memo, useMemo } from "react";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { StateView } from "@/components/common/StateView";
import { SortHeader, Table, Td, Th } from "@/components/common/Table";
import { UsageBar } from "@/components/common/UsageBar";
import { Tag } from "@/components/common/Tag";
import { type Translator, useT } from "@/i18n";
import { formatBytes, formatBytesPerSec, formatPercent, formatNumber } from "@/lib/format";
import type { ProcessRow } from "@/types/generated/ProcessRow";
import { useProcesses } from "./processStore";
import { filterRows, ioTotal, sameRowDisplay, type SortColumn, sortRows } from "./rows";
import { rankingKey, type RankingMode, useProcessView } from "./viewStore";

const EMPTY: readonly ProcessRow[] = [];

interface Column {
  id: SortColumn;
  label: string;
  align: "left" | "right";
  title?: string;
}

/** Column headers. CPU, RAM, I/O and Impact Score are the same in every language. */
function columns(t: Translator<"processes">): readonly Column[] {
  return [
    { id: "name", label: t("colName"), align: "left" },
    { id: "cpu", label: "CPU", align: "right" },
    { id: "ram", label: "RAM", align: "right" },
    { id: "io", label: "I/O", align: "right", title: t("ioNote") },
    { id: "impact", label: "Impact Score", align: "right" },
  ];
}

const pct = (v: number) => formatPercent(v, 1);
const bytes = (v: number) => formatBytes(v);
const rate = (v: number) => formatBytesPerSec(v);

/**
 * Table chrome, set from the wrapper so the shared `Table` stays untouched:
 * - it scrolls inside the panel (the header stays visible: `stickyHeader`), tall enough to
 *   fill the window but never below ~8 rows;
 * - the sticky header takes the primary surface's colour and a hairline under it;
 * - the first and last cells carry the panel's 20 px inset, so row fills run edge to edge.
 */
const TABLE_CLASS = [
  "max-h-[max(20rem,calc(100dvh-20rem))] overflow-y-auto overscroll-contain",
  "[&_thead]:bg-surface-raised [&_thead]:shadow-[inset_0_-1px_0_0_var(--line)]",
  "[&_thead_th]:pt-1",
  "[&_tr>:first-child]:pl-5 [&_tr>:last-child]:pr-5",
  "[&_tbody>tr:first-child>td]:border-t-0",
].join(" ");

/** Neutral tag for the app's own process (a classification, not a status: no `StatusBadge`). */
export function SelfTag() {
  const t = useT("processes");
  return <Tag>{t("selfTag")}</Tag>;
}

/**
 * Re-renders when the visible ranking (new snapshot), the mode, the sort, the filters or
 * the selection change. Rows are memoized on their displayed values, so unchanged
 * processes are not re-rendered on each 2 s snapshot.
 */
export function ProcessTable() {
  const t = useT("processes");
  const mode = useProcessView((s) => s.mode);
  const rows = useProcesses((s) => s.snapshot?.[rankingKey(mode)] ?? EMPTY);
  const sort = useProcessView((s) => s.sort);
  const query = useProcessView((s) => s.query);
  const category = useProcessView((s) => s.category);
  const sortBy = useProcessView((s) => s.sortBy);
  const selectedKey = useProcessView((s) => s.selected?.key ?? null);
  const select = useProcessView((s) => s.select);

  // `t` changes with the language, and so do the name collator and lowercasing.
  const visible = useMemo(() => sortRows(filterRows(rows, query, category), sort), [rows, query, category, sort, t]);
  const cols = useMemo(() => columns(t), [t]);

  if (rows.length === 0) {
    return (
      <div className="border-t border-line px-5 pb-2">
        <StateView kind="empty" title={t("emptyRanking")} />
      </div>
    );
  }
  if (visible.length === 0) {
    return <NoMatch query={query.trim()} filtered={category !== "all"} />;
  }

  return (
    <Table
      stickyHeader
      className={TABLE_CLASS}
      caption={t("tableCaption", { column: cols.find((c) => c.id === sort.column)?.label ?? "" })}
      head={cols
        .map((c) => (
          <SortHeader
            key={c.id}
            label={c.label}
            align={c.align}
            direction={sort.column === c.id ? sort.direction : null}
            onSort={() => sortBy(c.id)}
            {...(c.title ? { title: c.title } : {})}
          />
        ))
        .concat(
          mode === "window"
            ? [
                <Th key="peak" align="right">
                  {t("colPeak")}
                </Th>,
              ]
            : [],
        )}
    >
      {visible.map((row) => (
        <ProcessTableRow key={row.key} row={row} mode={mode} selected={row.key === selectedKey} onSelect={select} />
      ))}
    </Table>
  );
}

/** Search and/or category left nothing: say which, and offer the way back. */
function NoMatch({ query, filtered }: { query: string; filtered: boolean }) {
  const t = useT("processes");
  const clearFilters = useProcessView((s) => s.clearFilters);
  const title = filtered
    ? query
      ? t("noMatchCategoryQuery", { query })
      : t("noMatchCategory")
    : t("noMatch", { query });
  return (
    <div className="border-t border-line px-5 pb-2">
      <StateView
        kind="empty"
        title={title}
        description={t("noMatchHint")}
        action={
          <Button size="sm" onClick={clearFilters}>
            {t("clearFilters")}
          </Button>
        }
      />
    </div>
  );
}

interface RowProps {
  row: ProcessRow;
  mode: RankingMode;
  selected: boolean;
  onSelect: (row: ProcessRow | null) => void;
}

const ProcessTableRow = memo(
  function ProcessTableRow({ row, mode, selected, onSelect }: RowProps) {
    // Byte labels follow the view mode (lib/format.ts): re-render when it changes.
    useViewMode();
    const t = useT("processes");
    const io = ioTotal(row);
    const impact = row.pressure.impact;
    // The whole row is clickable; the name button makes it reachable by keyboard and its
    // click bubbles here, so there is a single handler. It opens the detail sheet (which
    // makes the page inert until it closes). Rows keep their key across snapshots and only
    // their text changes: nothing re-enters or restarts on refresh. The selected row (the
    // one whose sheet is open) gets the accent tint plus a 2 px accent bar on its edge.
    return (
      <tr
        onClick={() => onSelect(row)}
        className={`cursor-pointer transition-colors duration-150 ${selected ? "bg-accent-soft" : "hover:bg-control"} ${
          row.accessible ? "" : "text-fg-muted"
        }`}
      >
        <Td className={`w-[40%] max-w-0 ${selected ? "shadow-[inset_2px_0_0_var(--accent)]" : ""}`}>
          <button
            type="button"
            aria-haspopup="dialog"
            aria-label={t("rowShowDetails", { name: row.name, pid: row.pid })}
            className="flex w-full min-w-0 items-center gap-1.5 rounded-sm text-left"
          >
            {!row.accessible && (
              <span className="shrink-0">
                <Icon name="lock" size={12} className="text-fg-subtle" />
                <span className="sr-only">{t("protectedShort")}</span>
              </span>
            )}
            <span className={`truncate ${selected ? "font-medium text-fg" : ""}`}>{row.name}</span>
            {row.isSelf && <SelfTag />}
          </button>
        </Td>
        <Td className="text-right whitespace-nowrap">
          <MetricValue metric={row.cpuPercent} format={pct} />
        </Td>
        <Td className="text-right whitespace-nowrap">
          <MetricValue metric={row.workingSetBytes} format={bytes} />
        </Td>
        <Td className="text-right whitespace-nowrap">
          <MetricValue metric={io} format={rate} />
        </Td>
        <Td className="w-32 text-right">
          <div className="flex items-center justify-end gap-2">
            <UsageBar
              percent={impact}
              label={t("impactOf", { name: row.name })}
              valueText={t("outOf100", { value: formatNumber(impact, 1) })}
              className="w-14"
            />
            <span className="w-9 text-right" aria-hidden="true">
              {formatNumber(impact, 1)}
            </span>
          </div>
        </Td>
        {mode === "window" && <Td className="text-right whitespace-nowrap text-fg-muted">{formatNumber(row.impactPeak, 1)}</Td>}
      </tr>
    );
  },
  (prev, next) =>
    prev.mode === next.mode &&
    prev.selected === next.selected &&
    prev.onSelect === next.onSelect &&
    sameRowDisplay(prev.row, next.row),
);
