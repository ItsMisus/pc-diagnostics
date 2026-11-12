import { useEffect, useId, useMemo, useState } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { Modal } from "@/components/common/Modal";
import { Panel } from "@/components/common/Panel";
import { Segmented } from "@/components/common/Segmented";
import { Skeleton } from "@/components/common/Skeleton";
import { Spinner, StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { SortHeader, Td, Table, Th, Tr } from "@/components/common/Table";
import { PageHeader } from "@/components/layout/PageHeader";
import { useBench } from "@/features/benchmarks/benchStore";
import { ComparisonPanel } from "@/features/benchmarks/ComparisonPanel";
import { headlineMetric, isDebugBuild, kindLabel, stateLabel, stateTone } from "@/features/benchmarks/model";
import { ResultView } from "@/features/benchmarks/ResultView";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime, formatElapsed, formatUnitValue } from "@/lib/format";
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
  type OutcomeFilter,
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
  "rounded-sm text-left font-medium text-fg focus-visible:outline-none after:absolute after:inset-0 after:rounded-md focus-visible:after:outline-2 focus-visible:after:outline-solid focus-visible:after:-outline-offset-2 focus-visible:after:outline-(--focus)";

/**
 * History: every stored benchmark run on this machine. The run list is the focal panel; a run
 * opens in a trailing sheet with the same result view as Benchmarks and the comparison picker.
 */
export function HistoryPage() {
  const t = useT("history");
  const [state, setState] = useState<LoadState>({ status: "loading", data: null });
  const [nonce, setNonce] = useState(0);
  const [kind, setKind] = useState<KindFilter>("all");
  const [outcome, setOutcome] = useState<OutcomeFilter>("all");
  const [sort, setSort] = useState<HistorySort>(DEFAULT_SORT);
  // `openId` drives the sheet; `shownId` keeps its content during the exit animation.
  const [openId, setOpenId] = useState<string | null>(null);
  const [shownId, setShownId] = useState<string | null>(null);
  // Reload when a run finishes (the backend has stored it by then).
  const lastFinishedId = useBench((s) => s.lastFinished?.runId ?? null);

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

  const open = (id: string) => {
    setShownId(id);
    setOpenId(id);
  };

  const all = state.data;
  const rows = useMemo(
    () => (all ? sortHistory(filterHistory(all, kind, outcome), sort) : []),
    [all, kind, outcome, sort],
  );
  const kinds = useMemo(() => (all ? kindsInHistory(all) : []), [all]);
  const shown = useMemo(() => all?.find((r) => r.runId === shownId) ?? null, [all, shownId]);
  const loading = state.status === "loading";
  const reload = () => setNonce((n) => n + 1);
  const errorKind = state.status === "error" ? stateKindForError(state.error) : null;

  return (
    <>
      <PageHeader
        title="History"
        actions={
          <Button onClick={reload} disabled={loading} aria-busy={loading}>
            {loading ? <Spinner /> : <Icon name="refresh" />}
            {t("refresh")}
          </Button>
        }
      />

      <div className="flex flex-col gap-4">
        {state.status === "error" && (
          <Panel variant={state.data === null ? "primary" : "secondary"}>
            <StateView
              layout={state.data === null ? "hero" : "inline"}
              kind={errorKind ?? "error"}
              title={state.data ? t("refreshFailed") : t("unavailable")}
              {...(errorKind === "unsupported" ? { description: t("unsupportedDesc") } : {})}
              error={state.error}
              onRetry={reload}
            />
          </Panel>
        )}

        {all === null ? (
          loading && <HistorySkeleton />
        ) : all.length === 0 ? (
          <EmptyHistory />
        ) : (
          <Panel
            variant="primary"
            flush
            title={t("runs")}
            aside={
              <span className="tabular text-xs whitespace-nowrap text-fg-subtle" aria-live="polite">
                {t.plural("runCount", rows.length)}
              </span>
            }
          >
            <HistoryFilters
              kinds={kinds}
              kind={kind}
              onKind={setKind}
              outcome={outcome}
              onOutcome={setOutcome}
            />
            {rows.length === 0 ? (
              <div className="px-5 pb-5">
                <StateView
                  kind="empty"
                  title={kind !== "all" && outcome === "all" ? t("noRunsOfKind") : t("noRunsMatch")}
                  description={t("noRunsMatchDesc")}
                  action={
                    <Button
                      size="sm"
                      onClick={() => {
                        setKind("all");
                        setOutcome("all");
                      }}
                    >
                      {t("resetFilters")}
                    </Button>
                  }
                />
              </div>
            ) : (
              <HistoryTable
                rows={rows}
                selectedId={openId}
                onOpen={open}
                sort={sort}
                onSort={(key) => setSort((s) => nextSort(s, key))}
              />
            )}
            {all.length >= HISTORY_LIMIT && (
              <p className="border-t border-line px-5 py-3 text-xs text-fg-subtle">
                {t("limitNote", { count: HISTORY_LIMIT })}
              </p>
            )}
          </Panel>
        )}
      </div>

      <RunSheet
        run={shown}
        all={all ?? []}
        open={openId !== null && shown !== null}
        onClose={() => setOpenId(null)}
        onExitComplete={() => setShownId(null)}
      />
    </>
  );
}

/** Test type (native select: the list grows with the kinds present) and outcome (segmented). */
function HistoryFilters({
  kinds,
  kind,
  onKind,
  outcome,
  onOutcome,
}: {
  kinds: readonly BenchmarkResult["kind"][];
  kind: KindFilter;
  onKind: (k: KindFilter) => void;
  outcome: OutcomeFilter;
  onOutcome: (o: OutcomeFilter) => void;
}) {
  const t = useT("history");
  const tb = useT("benchmarks");
  const kindId = useId();
  return (
    <div className="flex flex-wrap items-center gap-x-5 gap-y-3 px-5 pb-4">
      <label htmlFor={kindId} className="flex min-w-0 items-center gap-2 text-xs text-fg-muted">
        <span className="shrink-0">{t("testType")}</span>
        <select
          id={kindId}
          className={`${SELECT} min-w-0 max-w-64`}
          value={kind}
          onChange={(e) => onKind(e.target.value as KindFilter)}
        >
          <option value="all">{t("filterAll")}</option>
          {kinds.map((k) => (
            <option key={k} value={k}>
              {kindLabel(k, tb)}
            </option>
          ))}
        </select>
      </label>
      <div className="flex min-w-0 items-center gap-2 text-xs text-fg-muted">
        <span aria-hidden="true" className="shrink-0">
          {t("outcomeLabel")}
        </span>
        <Segmented
          size="sm"
          label={t("outcomeLabel")}
          value={outcome}
          onChange={onOutcome}
          options={[
            { value: "all", label: t("outcomeAll") },
            { value: "completed", label: t("outcomeCompleted") },
            { value: "stopped", label: t("outcomeStopped") },
          ]}
        />
      </div>
    </div>
  );
}

/** First load only: rows shaped like the table. Later reloads keep the table on screen. */
function HistorySkeleton() {
  const t = useT("history");
  return (
    <Panel variant="primary" title={t("runs")}>
      <div role="status" aria-busy="true">
        <span className="sr-only">{t("loading")}</span>
        <div className="mb-4 flex gap-3">
          <Skeleton className="h-8 w-44" round />
          <Skeleton className="h-8 w-56" round />
        </div>
        {["w-24", "w-32", "w-20", "w-28", "w-24"].map((w, i) => (
          <div key={i} className="flex items-center gap-8 border-t border-line py-3.5">
            <Skeleton className="h-3 w-28" />
            <Skeleton className={`h-3 ${w}`} />
            <Skeleton className="h-3 w-16" />
            <Skeleton className="ml-auto h-3 w-12" />
          </div>
        ))}
      </div>
    </Panel>
  );
}

/** No run stored yet: the whole page is one calm invitation to the Benchmarks screen. */
function EmptyHistory() {
  const t = useT("history");
  const setSection = useAppStore((s) => s.setSection);
  return (
    <Panel variant="primary">
      <StateView
        layout="hero"
        kind="empty"
        title={t("emptyTitle")}
        description={t("emptyDesc")}
        action={
          <Button variant="primary" icon="benchmarks" onClick={() => setSection("benchmarks")}>
            {t("goToBenchmarks")}
          </Button>
        }
      />
    </Panel>
  );
}

function HistoryTable({
  rows,
  selectedId,
  onOpen,
  sort,
  onSort,
}: {
  rows: readonly BenchmarkResult[];
  selectedId: string | null;
  onOpen: (id: string) => void;
  sort: HistorySort;
  onSort: (key: HistorySortKey) => void;
}) {
  const t = useT("history");
  const tb = useT("benchmarks");
  const direction = (key: HistorySortKey) => (sort.key === key ? sort.direction : null);
  return (
    <Table
      caption={t("tableCaption")}
      className="pb-2"
      head={
        <>
          <SortHeader label={t("colDate")} direction={direction("date")} onSort={() => onSort("date")} className="pl-5" />
          <Th>{t("colTest")}</Th>
          <Th>{t("colState")}</Th>
          <Th>{t("colHeadline")}</Th>
          <SortHeader
            label={t("colDuration")}
            align="right"
            direction={direction("duration")}
            onSort={() => onSort("duration")}
            className="pr-5"
          />
        </>
      }
    >
      {rows.map((r) => {
        const head = headlineMetric(r);
        const selected = r.runId === selectedId;
        return (
          <Tr key={r.runId} interactive selected={selected} className="relative">
            <Td className={`py-3 pl-5 whitespace-nowrap ${selected ? "shadow-[inset_2px_0_0_var(--accent)]" : ""}`}>
              <button type="button" onClick={() => onOpen(r.runId)} aria-haspopup="dialog" className={ROW_BUTTON}>
                {formatDateTime(r.startedAt)}
                <span className="sr-only">{t("openDetail")}</span>
              </button>
            </Td>
            <Td className="py-3">
              <span className="block min-w-24">{kindLabel(r.kind, tb)}</span>
            </Td>
            <Td className="py-3">
              <span className="flex flex-wrap items-center gap-1">
                <StatusBadge tone={stateTone(r.state)} label={stateLabel(r.state, tb)} glyph={r.state !== "completed"} />
                {isDebugBuild(r) && <StatusBadge tone="warn" label={r.buildProfile} />}
              </span>
            </Td>
            <Td className="py-3">
              {head ? (
                <span className="flex min-w-0 flex-col">
                  <span className="font-medium whitespace-nowrap text-fg">{formatUnitValue(head.value, head.unit)}</span>
                  <span className="text-xs text-fg-subtle">{head.label}</span>
                </span>
              ) : (
                <span className="text-fg-subtle">
                  <span aria-hidden="true">—</span>
                  <span className="sr-only">{t("noResult")}</span>
                </span>
              )}
            </Td>
            <Td className="py-3 pr-5 text-right whitespace-nowrap text-fg-muted">{formatElapsed(r.durationMs)}</Td>
          </Tr>
        );
      })}
    </Table>
  );
}

/** The selected run in a trailing sheet: state, result (same view as Benchmarks) and comparison. */
function RunSheet({
  run,
  all,
  open,
  onClose,
  onExitComplete,
}: {
  run: BenchmarkResult | null;
  all: readonly BenchmarkResult[];
  open: boolean;
  onClose: () => void;
  onExitComplete: () => void;
}) {
  const t = useT("history");
  const tb = useT("benchmarks");
  return (
    <Modal
      open={open}
      onClose={onClose}
      onExitComplete={onExitComplete}
      presentation="sheet"
      size="lg"
      icon="benchmarks"
      closeOnBackdrop
      title={run ? kindLabel(run.kind, tb) : undefined}
      description={
        run ? (
          <span className="tabular">
            {t("detailMeta", { date: formatDateTime(run.startedAt), duration: formatElapsed(run.durationMs) })}
          </span>
        ) : undefined
      }
    >
      {run && (
        <div className="flex flex-col gap-4">
          <div className="flex flex-wrap items-center gap-1.5">
            <StatusBadge tone={stateTone(run.state)} label={stateLabel(run.state, tb)} glyph={run.state !== "completed"} />
            {isDebugBuild(run) && <StatusBadge tone="warn" label={run.buildProfile} />}
          </div>
          <ResultView
            key={run.runId}
            result={run}
            meta={false}
            comparison={<HistoryComparison all={all} current={run} />}
          />
        </div>
      )}
    </Modal>
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
