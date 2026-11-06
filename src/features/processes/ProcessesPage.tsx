import { useEffect } from "react";
import { Card } from "@/components/common/Card";
import { Segmented } from "@/components/common/Segmented";
import { StateView } from "@/components/common/StateView";
import { PageHeader } from "@/components/layout/PageHeader";
import { useT } from "@/i18n";
import { ProcessDetailPanel } from "./ProcessDetail";
import { ProcessTable } from "./ProcessTable";
import { processStore, type ProcessStatus, useProcesses } from "./processStore";
import { ScoreExplanation } from "./ScoreExplanation";
import { SearchField } from "@/components/common/SearchField";
import { rankingKey, type RankingMode, useProcessView } from "./viewStore";

/** Statuses in which a previously received snapshot may still be shown. */
const SHOWS_DATA: ReadonlySet<ProcessStatus> = new Set(["live", "connecting", "waiting"]);

/**
 * Page-scoped subscription: attached on mount, detached on unmount (the backend keeps
 * sampling regardless). Each block below selects its own slice of the store.
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
  const status = useProcesses((s) => s.status);
  const hasSnapshot = useProcesses((s) => s.snapshot !== null);

  if (!hasSnapshot || !SHOWS_DATA.has(status)) {
    return (
      <Card>
        <ProcessStatusView />
      </Card>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <Card className="min-w-0">
          <Toolbar />
          <ProcessTable />
        </Card>
        <div className="min-w-0">
          <ProcessDetailPanel />
        </div>
      </div>
      <ScoreExplanation />
    </div>
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
      return <StateView kind="loading" title={t("connectingTitle")} />;
    case "waiting":
      return (
        <StateView
          kind="loading"
          title={t("waitingTitle")}
          description={t("waitingDescription")}
        />
      );
    case "unsupported":
      return (
        <StateView
          kind="unsupported"
          title={t("unsupportedTitle")}
          description={t("unsupportedDescription")}
          {...(error ? { error } : {})}
        />
      );
    default:
      return (
        <StateView
          kind={status}
          title={status === "permission" ? t("notAllowedTitle") : t("unavailableTitle")}
          {...(error ? { error } : {})}
          onRetry={retry}
        />
      );
  }
}

function Toolbar() {
  const t = useT("processes");
  const mode = useProcessView((s) => s.mode);
  const setMode = useProcessView((s) => s.setMode);
  const query = useProcessView((s) => s.query);
  const setQuery = useProcessView((s) => s.setQuery);
  const windowS = useProcesses((s) => Math.round(s.snapshot?.windowS ?? 0));
  const options: { value: RankingMode; label: string }[] = [
    { value: "instant", label: t("modeInstant") },
    { value: "window", label: t("modeWindow", { seconds: windowS }) },
  ];

  return (
    <div className="mb-3 flex flex-wrap items-center gap-x-4 gap-y-2">
      <Segmented label={t("rankingLabel")} options={options} value={mode} onChange={setMode} />
      <Summary />
      <SearchField
        label={t("searchLabel")}
        value={query}
        onChange={setQuery}
        placeholder={t("searchPlaceholder")}
        className="ml-auto w-56"
      />
    </div>
  );
}

/**
 * The counts, once: total, protected, ranked. Selects primitives only, so it re-renders
 * only when a number changes.
 */
function Summary() {
  const t = useT("processes");
  const mode = useProcessView((s) => s.mode);
  const total = useProcesses((s) => s.snapshot?.totalProcesses ?? 0);
  const inaccessible = useProcesses((s) => s.snapshot?.inaccessibleProcesses ?? 0);
  const shown = useProcesses((s) => s.snapshot?.[rankingKey(mode)].length ?? 0);

  return (
    <p className="text-xs text-fg-muted tabular" aria-live="off">
      {t.plural("summaryTotal", total)}
      {inaccessible > 0 && (
        <>
          {" · "}
          <span title={t("protectedNote")}>{t.plural("summaryProtected", inaccessible)}</span>
        </>
      )}
      {" · "}
      {t("summaryShown", { shown })}
    </p>
  );
}
