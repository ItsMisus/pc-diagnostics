import { useEffect } from "react";
import { Panel } from "@/components/common/Panel";
import { SearchField } from "@/components/common/SearchField";
import { Segmented } from "@/components/common/Segmented";
import { Skeleton } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { Tooltip } from "@/components/common/Tooltip";
import { PageHeader } from "@/components/layout/PageHeader";
import { useT } from "@/i18n";
import { ProcessDetailSheet } from "./ProcessDetail";
import { ProcessTable } from "./ProcessTable";
import { processStore, type ProcessStatus, useProcesses } from "./processStore";
import { CATEGORY_FILTERS, type CategoryFilter, filterRows } from "./rows";
import { ScoreExplanation } from "./ScoreExplanation";
import { rankingKey, type RankingMode, useProcessView } from "./viewStore";

/** Statuses in which a previously received snapshot may still be shown. */
const SHOWS_DATA: ReadonlySet<ProcessStatus> = new Set(["live", "connecting", "waiting"]);
/** Statuses that are only "not there yet": shown as a skeleton of the table. */
const PENDING: ReadonlySet<ProcessStatus> = new Set(["idle", "connecting", "live", "waiting"]);

/**
 * Page-scoped subscription: attached on mount, detached on unmount (the backend keeps
 * sampling regardless). Each block below selects its own slice of the store.
 *
 * Layout: the ranking table is the focal (primary) panel, with its controls in one row above
 * it; the Impact Score explanation follows once; a process's detail opens in a trailing sheet.
 */
export function ProcessesPage() {
  useEffect(() => processStore.getState().start(), []);

  return (
    <>
      <PageHeader title="Processes" />
      <ProcessContent />
    </>
  );
}

function ProcessContent() {
  const t = useT("processes");
  const status = useProcesses((s) => s.status);
  const hasSnapshot = useProcesses((s) => s.snapshot !== null);

  if (!hasSnapshot || !SHOWS_DATA.has(status)) {
    if (PENDING.has(status)) return <LoadingView />;
    return (
      <Panel variant="primary">
        <ProcessStatusView />
      </Panel>
    );
  }

  return (
    <div className="flex flex-col gap-8">
      <Panel variant="primary" flush title={t("panelTitle")} aside={<Summary />} className="min-w-0 overflow-hidden">
        <Toolbar />
        <ProcessTable />
      </Panel>
      <ScoreExplanation />
      <ProcessDetailSheet />
    </div>
  );
}

/** First load: the state in words, then a placeholder shaped like the toolbar and the table. */
function LoadingView() {
  return (
    <Panel variant="primary" flush className="min-w-0 overflow-hidden">
      <div className="px-5 pt-5">
        <ProcessStatusView />
      </div>
      <div aria-hidden="true" className="flex flex-wrap items-center gap-3 px-5 pt-2 pb-4">
        <Skeleton className="h-8 w-56 max-w-full" round />
        <Skeleton className="h-7 w-52" round />
        <Skeleton className="h-7 w-64" round />
      </div>
      <div aria-hidden="true" className="flex flex-col">
        {Array.from({ length: 8 }, (_, i) => (
          <div key={i} className="flex items-center gap-6 border-t border-line px-5 py-2.5">
            <Skeleton className={`h-3 ${i % 3 === 0 ? "w-40" : i % 3 === 1 ? "w-28" : "w-36"}`} />
            <span className="ml-auto flex items-center gap-6">
              <Skeleton className="h-3 w-10" />
              <Skeleton className="h-3 w-14" />
              <Skeleton className="hidden h-3 w-14 sm:block" />
              <Skeleton className="h-3 w-20" />
            </span>
          </div>
        ))}
      </div>
    </Panel>
  );
}

function ProcessStatusView() {
  const t = useT("processes");
  const status = useProcesses((s) => s.status);
  const error = useProcesses((s) => s.error);
  const retry = useProcesses((s) => s.retry);

  switch (status) {
    case "idle":
    case "connecting":
    case "live":
      return <StateView compact kind="loading" title={t("connectingTitle")} />;
    case "waiting":
      return <StateView compact kind="loading" title={t("waitingTitle")} description={t("waitingDescription")} />;
    case "unsupported":
      return (
        <StateView
          layout="hero"
          kind="unsupported"
          title={t("unsupportedTitle")}
          description={t("unsupportedDescription")}
          {...(error ? { error } : {})}
        />
      );
    default:
      return (
        <StateView
          layout="hero"
          kind={status}
          title={status === "permission" ? t("notAllowedTitle") : t("unavailableTitle")}
          {...(error ? { error } : {})}
          onRetry={retry}
        />
      );
  }
}

/**
 * One control row: search, ranking mode, category. Wraps on narrow windows; the search field
 * grows to fill what is left of its line.
 */
function Toolbar() {
  const t = useT("processes");
  const mode = useProcessView((s) => s.mode);
  const setMode = useProcessView((s) => s.setMode);
  const query = useProcessView((s) => s.query);
  const setQuery = useProcessView((s) => s.setQuery);
  const category = useProcessView((s) => s.category);
  const setCategory = useProcessView((s) => s.setCategory);
  const windowS = useProcesses((s) => Math.round(s.snapshot?.windowS ?? 0));
  const modes: { value: RankingMode; label: string }[] = [
    { value: "instant", label: t("modeInstant") },
    { value: "window", label: t("modeWindow", { seconds: windowS }) },
  ];
  const categories = CATEGORY_FILTERS.map((value) => ({ value, label: t(`filter_${value}`) }));

  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2 px-5 pb-4">
      <SearchField
        label={t("searchLabel")}
        value={query}
        onChange={setQuery}
        placeholder={t("searchPlaceholder")}
        className="min-w-48 flex-1 basis-56 sm:max-w-72"
      />
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <Segmented label={t("rankingLabel")} options={modes} value={mode} onChange={setMode} size="sm" />
        <Segmented<CategoryFilter>
          label={`${t("filterLabel")}. ${t("filterSystemNote")}`}
          options={categories}
          value={category}
          onChange={setCategory}
          size="sm"
        />
      </div>
      {category === "system" && <p className="basis-full text-xs text-fg-muted">{t("filterSystemNote")}</p>}
    </div>
  );
}

/**
 * The counts, once: total, protected, ranked (and how many the filters leave). Selects
 * primitives only, so it re-renders only when a number changes.
 */
function Summary() {
  const t = useT("processes");
  const mode = useProcessView((s) => s.mode);
  const total = useProcesses((s) => s.snapshot?.totalProcesses ?? 0);
  const inaccessible = useProcesses((s) => s.snapshot?.inaccessibleProcesses ?? 0);
  const shown = useProcesses((s) => s.snapshot?.[rankingKey(mode)].length ?? 0);
  const query = useProcessView((s) => s.query);
  const category = useProcessView((s) => s.category);
  const filtered = query.trim() !== "" || category !== "all";
  // A number, so the selector stays cheap to compare; computed only while a filter is on.
  const visible = useProcesses((s) =>
    filtered ? filterRows(s.snapshot?.[rankingKey(mode)] ?? [], query, category).length : shown,
  );

  return (
    <p className="flex flex-wrap items-center justify-end gap-x-1 text-xs text-fg-muted tabular" aria-live="off">
      <span>{t.plural("summaryTotal", total)}</span>
      {inaccessible > 0 && (
        <>
          <span aria-hidden="true">·</span>
          <Tooltip content={t("protectedNote")} side="bottom-end" multiline>
            <button
              type="button"
              className="rounded-xs underline decoration-line-strong decoration-dotted underline-offset-2 hover:text-fg"
              aria-label={`${t.plural("summaryProtected", inaccessible)}. ${t("protectedNote")}`}
            >
              {t.plural("summaryProtected", inaccessible)}
            </button>
          </Tooltip>
        </>
      )}
      <span aria-hidden="true">·</span>
      <span>{t("summaryShown", { shown })}</span>
      {filtered && (
        <>
          <span aria-hidden="true">·</span>
          <span className="text-fg">{t("summaryVisible", { visible })}</span>
        </>
      )}
    </p>
  );
}
