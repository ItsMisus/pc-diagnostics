import { m } from "motion/react";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { Skeleton } from "@/components/common/Skeleton";
import { Spinner, StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { SortHeader, Td, Table } from "@/components/common/Table";
import { PageHeader } from "@/components/layout/PageHeader";
import { useBench } from "@/features/benchmarks/benchStore";
import { ComparisonPanel } from "@/features/benchmarks/ComparisonPanel";
import { headlineMetric, isDebugBuild, kindLabel, stateLabel, stateTone } from "@/features/benchmarks/model";
import { ResultView } from "@/features/benchmarks/ResultView";
import { Th } from "@/features/benchmarks/ui";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime, formatElapsed, formatUnitValue } from "@/lib/format";
import { duration, ease } from "@/lib/motion";
import { commands } from "@/services/commands";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";
import {
  compareCandidates,
  DEFAULT_SORT,
  filterHistory,
  type HistorySort,
  type HistorySortKey,
  type KindFilter,
  kindsInHistory,
  nextSort,
  sortHistory,
} from "./model";

const HISTORY_LIMIT = 200;

type LoadState =
  | { status: "loading"; data: BenchmarkResult[] | null }
  | { status: "ready"; data: BenchmarkResult[] }
  | { status: "error"; error: AppError; data: BenchmarkResult[] | null };

/** Native selects are styled globally: sizing only (left padding; the chevron owns the right). */
const SELECT = "h-8 pl-2.5";

/**
 * The whole row is the hit target: the date button's ::after covers the row (which is
 * `relative`), and the keyboard focus ring is drawn on that overlay, around the row.
 */
const ROW_BUTTON =
  "rounded-sm text-left text-fg focus-visible:outline-none after:absolute after:inset-0 after:rounded-md focus-visible:after:outline-2 focus-visible:after:outline-solid focus-visible:after:-outline-offset-2 focus-visible:after:outline-(--focus)";

const detailIn = {
  initial: { opacity: 0, y: 6 },
  animate: { opacity: 1, y: 0, transition: { duration: duration.component, ease: ease.out } },
} as const;

export function HistoryPage() {
  const t = useT("history");
  const tb = useT("benchmarks");
  const tc = useT("common");
  const [state, setState] = useState<LoadState>({ status: "loading", data: null });
  const [nonce, setNonce] = useState(0);
  const [filter, setFilter] = useState<KindFilter>("all");
  const [sort, setSort] = useState<HistorySort>(DEFAULT_SORT);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  // Reload when a run finishes (the backend has stored it by then).
  const lastFinishedId = useBench((s) => s.lastFinished?.runId ?? null);
  const filterId = useId();
  const detailRef = useRef<HTMLDivElement>(null);
  const revealOnSelect = useRef(false);

  useEffect(() => {
    let active = true;
    setState((s) => ({ status: "loading", data: s.data }));
    commands.getBenchmarkHistory(null, HISTORY_LIMIT).then(
      (data) => active && setState({ status: "ready", data }),
      (e: unknown) => active && setState((s) => ({ status: "error", error: toAppError(e), data: s.data })),
    );
    return () => {
      active = false;
    };
  }, [nonce, lastFinishedId]);

  // The detail sits under the table: bring it into view when the user picks a run.
  useEffect(() => {
    if (!revealOnSelect.current || selectedId === null) return;
    revealOnSelect.current = false;
    revealDetail(detailRef.current);
  }, [selectedId]);

  const select = (id: string) => {
    if (id === selectedId) {
      revealDetail(detailRef.current);
      return;
    }
    revealOnSelect.current = true;
    setSelectedId(id);
  };

  const all = state.data;
  const rows = useMemo(() => (all ? sortHistory(filterHistory(all, filter), sort) : []), [all, filter, sort]);
  const kinds = useMemo(() => (all ? kindsInHistory(all) : []), [all]);
  const selected = useMemo(() => all?.find((r) => r.runId === selectedId) ?? null, [all, selectedId]);
  const loading = state.status === "loading";

  return (
    <>
      <PageHeader
        title="History"
        description={t("pageDescription")}
        actions={
          <Button onClick={() => setNonce((n) => n + 1)} disabled={loading} aria-busy={loading}>
            {loading ? <Spinner /> : <Icon name="refresh" />}
            {t("refresh")}
          </Button>
        }
      />

      {state.status === "error" && (
        <Card className="mb-4">
          <StateView
            compact={state.data !== null}
            kind={stateKindForError(state.error)}
            title={state.data ? t("refreshFailed") : t("unavailable")}
            {...(stateKindForError(state.error) === "unsupported"
              ? { description: t("unsupportedDesc") }
              : {})}
            error={state.error}
            onRetry={() => setNonce((n) => n + 1)}
          />
        </Card>
      )}

      {all === null ? (
        loading && <HistorySkeleton />
      ) : all.length === 0 ? (
        <Card>
          <EmptyHistory />
        </Card>
      ) : (
        <div className="flex flex-col gap-4">
          <Card
            flush
            title={t("runs")}
            aside={
              <label htmlFor={filterId} className="flex items-center gap-2 text-xs text-fg-muted">
                {t("testType")}
                <select
                  id={filterId}
                  className={SELECT}
                  value={filter}
                  onChange={(e) => setFilter(e.target.value as KindFilter)}
                >
                  <option value="all">{t("filterAll")}</option>
                  {kinds.map((k) => (
                    <option key={k} value={k}>
                      {kindLabel(k, tb)}
                    </option>
                  ))}
                </select>
              </label>
            }
          >
            {rows.length === 0 ? (
              <div className="px-4 pb-4">
                <StateView compact kind="empty" title={t("noRunsOfKind")} />
              </div>
            ) : (
              <HistoryTable
                rows={rows}
                selectedId={selectedId}
                onSelect={select}
                sort={sort}
                onSort={(key) => setSort((s) => nextSort(s, key))}
              />
            )}
            {all.length >= HISTORY_LIMIT && (
              <p className="border-t border-line px-4 py-2.5 text-xs text-fg-subtle">
                {t("limitNote", { count: HISTORY_LIMIT })}
              </p>
            )}
          </Card>

          {selected && (
            <m.div key={selected.runId} ref={detailRef} className="scroll-mt-16" {...detailIn}>
              <Card
                title={t("detail")}
                aside={
                  <Button variant="ghost" size="sm" className="-mr-1.5" onClick={() => setSelectedId(null)}>
                    <Icon name="close" size={14} />
                    {tc("close")}
                  </Button>
                }
              >
                <ResultView
                  key={selected.runId}
                  result={selected}
                  comparison={<HistoryComparison all={all} current={selected} />}
                />
              </Card>
            </m.div>
          )}
        </div>
      )}
    </>
  );
}

/** Scrolls the detail card into view (minimal scroll; instant with reduced motion). */
function revealDetail(el: HTMLElement | null) {
  if (!el) return;
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  el.scrollIntoView({ block: "nearest", behavior: reduce ? "auto" : "smooth" });
}

/** First load only: rows shaped like the table. Later reloads keep the table on screen. */
function HistorySkeleton() {
  const t = useT("history");
  return (
    <Card title={t("runs")}>
      <div role="status" aria-busy="true">
        <span className="sr-only">{t("loading")}</span>
        {["w-24", "w-32", "w-20", "w-28", "w-24"].map((w, i) => (
          <div key={i} className="flex items-center gap-8 border-t border-line py-3 first:border-t-0 first:pt-1">
            <Skeleton className="h-3 w-32" />
            <Skeleton className={`h-3 ${w}`} />
            <Skeleton className="h-3 w-16" />
            <Skeleton className="h-3 w-20" />
          </div>
        ))}
      </div>
    </Card>
  );
}

function EmptyHistory() {
  const t = useT("history");
  const setSection = useAppStore((s) => s.setSection);
  return (
    <div className="flex flex-col items-start gap-1 pb-2">
      <StateView
        kind="empty"
        title={t("emptyTitle")}
        description={t("emptyDesc")}
      />
      <Button variant="primary" onClick={() => setSection("benchmarks")}>
        <Icon name="benchmarks" />
        {t("goToBenchmarks")}
      </Button>
    </div>
  );
}

function HistoryTable({
  rows,
  selectedId,
  onSelect,
  sort,
  onSort,
}: {
  rows: readonly BenchmarkResult[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  sort: HistorySort;
  onSort: (key: HistorySortKey) => void;
}) {
  const t = useT("history");
  const tb = useT("benchmarks");
  const direction = (key: HistorySortKey) => (sort.key === key ? sort.direction : null);
  return (
    <Table
      caption={t("tableCaption")}
      className="pb-1"
      head={
        <>
          <SortHeader label={t("colDate")} direction={direction("date")} onSort={() => onSort("date")} className="pl-4" />
          <Th>{t("colTest")}</Th>
          <Th>{t("colState")}</Th>
          <Th>{t("colHeadline")}</Th>
          <SortHeader
            label={t("colDuration")}
            align="right"
            direction={direction("duration")}
            onSort={() => onSort("duration")}
          />
          <Th>{t("colMethodology")}</Th>
        </>
      }
    >
      {rows.map((r) => {
        const head = headlineMetric(r);
        const selected = r.runId === selectedId;
        return (
          <tr
            key={r.runId}
            className={`relative transition-colors duration-150 ${selected ? "bg-accent-soft" : "hover:bg-surface-2/60"}`}
          >
            <Td className={`pl-4 whitespace-nowrap ${selected ? "shadow-[inset_2px_0_0_var(--accent)]" : ""}`}>
              <button type="button" onClick={() => onSelect(r.runId)} aria-pressed={selected} className={ROW_BUTTON}>
                {formatDateTime(r.startedAt)}
                <span className="sr-only">{t("openDetail")}</span>
              </button>
            </Td>
            <Td>{kindLabel(r.kind, tb)}</Td>
            <Td>
              <span className="flex flex-wrap items-center gap-1">
                <StatusBadge tone={stateTone(r.state)} label={stateLabel(r.state, tb)} />
                {isDebugBuild(r) && <StatusBadge tone="warn" label={r.buildProfile} />}
              </span>
            </Td>
            <Td>
              {head ? (
                <>
                  <span className="font-medium">{formatUnitValue(head.value, head.unit)}</span>{" "}
                  <span className="text-xs text-fg-subtle">{head.label}</span>
                </>
              ) : (
                <span className="text-fg-subtle">
                  <span aria-hidden="true">—</span>
                  <span className="sr-only">{t("noResult")}</span>
                </span>
              )}
            </Td>
            <Td className="text-right whitespace-nowrap">{formatElapsed(r.durationMs)}</Td>
            <Td className="text-fg-muted">v{r.methodologyVersion}</Td>
          </tr>
        );
      })}
    </Table>
  );
}

/** Compare with the previous comparable run (automatic) or a chosen run of the same kind. */
function HistoryComparison({ all, current }: { all: readonly BenchmarkResult[]; current: BenchmarkResult }) {
  const t = useT("history");
  const candidates = useMemo(() => compareCandidates(all, current), [all, current]);
  const [previousId, setPreviousId] = useState<string | null>(null);
  const id = useId();
  return (
    <ComparisonPanel
      currentRunId={current.runId}
      previousRunId={previousId}
      aside={
        candidates.length > 0 && (
          <label htmlFor={id} className="flex min-w-0 items-center gap-2 text-xs text-fg-muted">
            <span className="shrink-0">{t("compareWith")}</span>
            <select
              id={id}
              className={`${SELECT} min-w-0 max-w-72`}
              value={previousId ?? ""}
              onChange={(e) => setPreviousId(e.target.value === "" ? null : e.target.value)}
            >
              <option value="">{t("compareAuto")}</option>
              {candidates.map((c) => (
                <option key={c.runId} value={c.runId}>
                  {formatDateTime(c.startedAt)} · v{c.methodologyVersion}
                  {isDebugBuild(c) ? ` · ${c.buildProfile}` : ""}
                </option>
              ))}
            </select>
          </label>
        )
      }
    />
  );
}
