import { useEffect } from "react";
import { useAppStore } from "@/app/store";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { Segmented } from "@/components/common/Segmented";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { PageHeader } from "@/components/layout/PageHeader";
import { formatTime } from "@/lib/format";
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
  const t = useT("processes");
  useEffect(() => processStore.getState().start(), []);

  return (
    <>
      <PageHeader
        title="Processes"
        description={t("pageDescription")}
        actions={<ProcessBadge />}
      />
      <ProcessContent />
    </>
  );
}

function ProcessContent() {
  const t = useT("processes");
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
      <p className="flex items-start gap-1.5 text-xs text-fg-subtle">
        <Icon name="info" size={14} className="mt-px shrink-0" />
        {t("scoreDisclaimer")}
      </p>
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <Card className="min-w-0">
          <Toolbar />
          <Summary />
          <ProcessTable />
          <p className="mt-3 text-2xs text-fg-subtle">{t("ioNote")}.</p>
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

function ProcessBadge() {
  const t = useT("processes");
  const status = useProcesses((s) => s.status);
  switch (status) {
    case "live":
      return <StatusBadge tone="ok" label={t("badgeLive")} />;
    case "idle":
    case "connecting":
      return <StatusBadge tone="neutral" label={t("badgeConnecting")} />;
    case "waiting":
      return <StatusBadge tone="neutral" label={t("badgeWaiting")} />;
    case "unsupported":
      return <StatusBadge tone="neutral" label={t("badgeUnsupported")} />;
    case "permission":
    case "consent":
      return <StatusBadge tone="warn" label={t("badgeNotAllowed")} />;
    case "error":
      return <StatusBadge tone="bad" label={t("badgeError")} />;
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
    <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
      <Segmented label={t("rankingLabel")} options={options} value={mode} onChange={setMode} />
      <SearchField
        label={t("searchLabel")}
        value={query}
        onChange={setQuery}
        placeholder={t("searchPlaceholder")}
        className="w-56"
      />
    </div>
  );
}

/** Totals line. Selects primitives only, so it re-renders only when a number changes. */
function Summary() {
  const t = useT("processes");
  const shell = useT("shell");
  const mode = useProcessView((s) => s.mode);
  const total = useProcesses((s) => s.snapshot?.totalProcesses ?? 0);
  const inaccessible = useProcesses((s) => s.snapshot?.inaccessibleProcesses ?? 0);
  const shown = useProcesses((s) => s.snapshot?.[rankingKey(mode)].length ?? 0);
  const at = useProcesses((s) => s.snapshot?.at ?? 0);
  const setSection = useAppStore((s) => s.setSection);

  return (
    <p className="mb-3 text-xs leading-5 text-fg-muted tabular" aria-live="off">
      {t.rich("summary", {
        total,
        inaccessible,
        shown,
        time: formatTime(at),
        settings: (
          <button
            type="button"
            onClick={() => setSection("settings")}
            className="rounded-sm underline decoration-line-strong underline-offset-2 transition-colors duration-150 hover:text-fg hover:decoration-fg-subtle"
          >
            {shell("section_settings")}
          </button>
        ),
      })}
    </p>
  );
}
