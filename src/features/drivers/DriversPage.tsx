import { useEffect, useMemo } from "react";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { Spinner, StateView } from "@/components/common/StateView";
import { ViewModeToggle } from "@/components/common/ViewModeToggle";
import { PageHeader } from "@/components/layout/PageHeader";
import { useIsSimple } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime } from "@/lib/format";
import type { DeviceRecord } from "@/types/generated/DeviceRecord";
import type { DriverSummary } from "@/types/generated/DriverSummary";
import { DeviceList, ProblemExplanationView } from "./DeviceList";
import { useDrivers } from "./driverStore";
import { categoryLabel, isProblem, summaryCards } from "./model";
import { deviceProblemExplanation, simpleDeviceName, simpleDriverSummary } from "./simple";
import { UpdateCheckPanel } from "./UpdateCheckPanel";

const DOT = { ok: "bg-ok", warn: "bg-warn", bad: "bg-bad", neutral: "" } as const;

/** Device and driver inventory. Strictly read-only: nothing is installed or removed. */
export function DriversPage() {
  const t = useT("drivers");
  const status = useDrivers((s) => s.status);
  const data = useDrivers((s) => s.data);
  const ensureLoaded = useDrivers((s) => s.ensureLoaded);
  useEffect(() => ensureLoaded(), [ensureLoaded]);
  const simple = useIsSimple();

  return (
    <>
      <PageHeader
        title="Drivers"
        description={t("pageDescription")}
        actions={
          <>
            <ViewModeToggle />
            <RefreshButton disabled={status !== "ready"} />
          </>
        }
      />
      <RefreshError />

      {status !== "ready" || !data ? (
        <Card>
          <InventoryStateView />
        </Card>
      ) : (
        <div className="flex flex-col gap-4">
          <p className="text-xs text-fg-subtle tabular">{t("collectedAt", { date: formatDateTime(data.collectedAt) })}</p>
          {simple ? (
            <>
              <SimpleSummary summary={data.summary} />
              <ProblemDevices devices={data.devices} />
            </>
          ) : (
            <>
              <SummaryCards summary={data.summary} />
              <Interpretation />
            </>
          )}
          <UpdateCheckPanel />
          {data.devices.length === 0 ? (
            <Card>
              <StateView kind="empty" title={t("emptyTitle")} description={t("summaryEmpty")} />
            </Card>
          ) : (
            <DeviceList devices={data.devices} />
          )}
        </div>
      )}
    </>
  );
}

/**
 * Five stat tiles. The grid follows the width of the content area (container query), not
 * the window: one row of five when there is room, otherwise two wide tiles over three.
 * Each tile is a three-row subgrid, so labels, values and hints line up across a row even
 * when a label wraps; tiles in a row share one height.
 */
function SummaryCards({ summary }: { summary: DriverSummary }) {
  const t = useT("drivers");
  const cards = useMemo(() => summaryCards(summary, t), [summary, t]);
  return (
    <div className="@container">
      <ul className="grid grid-cols-6 gap-4 @4xl:grid-cols-5">
        {cards.map((c, i) => (
          <li
            key={c.id}
            className={`row-span-3 grid grid-rows-subgrid gap-y-1 rounded-xl border border-line bg-surface p-4 shadow-[inset_0_1px_0_var(--highlight)] ${
              i < 2 ? "col-span-3" : "col-span-2"
            } @4xl:col-span-1`}
          >
            <div className="text-xs text-fg-muted">{c.label}</div>
            <div className="flex items-center gap-2 text-2xl font-semibold text-fg tabular">
              {c.tone !== "neutral" && <span aria-hidden="true" className={`size-2 shrink-0 rounded-full ${DOT[c.tone]}`} />}
              {c.value}
            </div>
            <div className="text-2xs text-fg-subtle">{c.hint}</div>
          </li>
        ))}
      </ul>
      <p className="mt-2 text-xs text-fg-subtle tabular">{t.plural("disabledNote", summary.disabled)}</p>
    </div>
  );
}

/** "Semplice": the inventory in a few plain sentences instead of counters. */
function SimpleSummary({ summary }: { summary: DriverSummary }) {
  const t = useT("drivers");
  const lines = useMemo(() => simpleDriverSummary(summary, t), [summary, t]);
  const problems = summary.problems + summary.driverMissing;
  return (
    <section
      aria-label={t("summaryAria")}
      className={`rounded-xl border bg-surface p-4 shadow-[inset_0_1px_0_var(--highlight)] ${problems > 0 ? "border-bad/40" : "border-line"}`}
    >
      <p className="text-base font-medium text-fg">{lines[0]}</p>
      {lines.slice(1).map((line) => (
        <p key={line} className="mt-1 text-sm text-fg-muted">
          {line}
        </p>
      ))}
    </section>
  );
}

/** "Semplice": each device with a problem, what Windows is saying and safe things to try. */
function ProblemDevices({ devices }: { devices: readonly DeviceRecord[] }) {
  const t = useT("drivers");
  const problems = useMemo(() => devices.filter(isProblem), [devices]);
  if (problems.length === 0) return null;
  return (
    <Card title={t("problemDevicesTitle")}>
      <ul className="flex flex-col gap-3">
        {problems.map((d) => {
          const explanation = deviceProblemExplanation(d, t);
          return (
            <li key={d.instanceId} className="rounded-lg border border-line bg-surface-2/40 p-3">
              <div className="flex flex-wrap items-baseline gap-x-2">
                <span className="text-sm font-medium text-fg">{simpleDeviceName(d, t)}</span>
                <span className="text-xs text-fg-subtle">{categoryLabel(d.category, t)}</span>
              </div>
              {explanation && <ProblemExplanationView device={d} explanation={explanation} />}
            </li>
          );
        })}
      </ul>
    </Card>
  );
}

function Interpretation() {
  const t = useT("drivers");
  return (
    <Card title={t("howToReadTitle")}>
      <ul className="list-disc space-y-1 pl-5 text-sm text-fg-muted marker:text-fg-subtle">
        <li>{t("howToRead1")}</li>
        <li>{t("howToRead2")}</li>
        <li>{t("howToRead3")}</li>
        <li>{t("howToRead4")}</li>
      </ul>
    </Card>
  );
}

function InventoryStateView() {
  const t = useT("drivers");
  const status = useDrivers((s) => s.status);
  const error = useDrivers((s) => s.error);
  const reload = useDrivers((s) => s.reload);

  if (status !== "error" || !error) {
    return (
      <StateView
        kind="loading"
        title={t("loadingTitle")}
        description={t("loadingDescription")}
      />
    );
  }
  const kind = stateKindForError(error);
  if (kind === "unsupported") {
    return (
      <StateView
        kind="unsupported"
        title={t("unsupportedTitle")}
        description={t("unsupportedDescription")}
        error={error}
      />
    );
  }
  return <StateView kind={kind} title={t("unavailableTitle")} error={error} onRetry={reload} />;
}

function RefreshButton({ disabled }: { disabled: boolean }) {
  const t = useT("drivers");
  const refreshing = useDrivers((s) => s.refreshing);
  const checking = useDrivers((s) => s.check.status === "running");
  const refresh = useDrivers((s) => s.refresh);
  return (
    <Button
      onClick={refresh}
      disabled={disabled || refreshing || checking}
      aria-busy={refreshing}
      {...(checking ? { title: t("refreshBlocked") } : {})}
    >
      {refreshing ? <Spinner /> : <Icon name="refresh" />}
      {refreshing ? t("refreshing") : t("refresh")}
    </Button>
  );
}

function RefreshError() {
  const t = useT("drivers");
  const error = useDrivers((s) => s.refreshError);
  const refresh = useDrivers((s) => s.refresh);
  if (!error) return null;
  return (
    <div className="mb-4">
      <StateView
        compact
        kind={stateKindForError(error)}
        title={t("refreshErrorTitle")}
        error={error}
        onRetry={refresh}
      />
    </div>
  );
}
