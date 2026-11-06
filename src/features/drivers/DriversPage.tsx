import { useEffect, useMemo } from "react";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { Spinner, StateView } from "@/components/common/StateView";
import { PageHeader } from "@/components/layout/PageHeader";
import { useIsSimple } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime } from "@/lib/format";
import type { DeviceRecord } from "@/types/generated/DeviceRecord";
import type { DriverSummary } from "@/types/generated/DriverSummary";
import { DeviceList, ProblemExplanationView } from "./DeviceList";
import { useDrivers } from "./driverStore";
import { categoryLabel, isProblem } from "./model";
import { deviceProblemExplanation, simpleDeviceName, simpleDriverSummary, simpleNoInstallNote } from "./simple";
import { UpdateCheckPanel } from "./UpdateCheckPanel";

/**
 * Device and driver inventory. Strictly read-only: nothing is installed or removed. The
 * counts live once, on the device list's filter tabs ("Avanzata") or in the plain summary
 * ("Semplice"); the reading notes once, under the device list's title.
 */
export function DriversPage() {
  const t = useT("drivers");
  const status = useDrivers((s) => s.status);
  const data = useDrivers((s) => s.data);
  const ensureLoaded = useDrivers((s) => s.ensureLoaded);
  useEffect(() => ensureLoaded(), [ensureLoaded]);
  const simple = useIsSimple();

  return (
    <>
      <PageHeader title="Drivers" actions={<RefreshButton disabled={status !== "ready"} />} />
      <RefreshError />

      {status !== "ready" || !data ? (
        <Card>
          <InventoryStateView />
        </Card>
      ) : (
        <div className="flex flex-col gap-4">
          <p className="text-xs text-fg-subtle tabular">{t("collectedAt", { date: formatDateTime(data.collectedAt) })}</p>
          {simple && (
            <>
              <SimpleSummary summary={data.summary} />
              <ProblemDevices devices={data.devices} />
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

/** "Semplice": the inventory in a few plain sentences instead of counters. */
function SimpleSummary({ summary }: { summary: DriverSummary }) {
  const t = useT("drivers");
  const lines = useMemo(() => simpleDriverSummary(summary, t), [summary, t]);
  return (
    <Card title={t("summaryAria")}>
      <p className="text-[15px] leading-6 font-medium text-fg">{lines[0]}</p>
      {lines.slice(1).map((line) => (
        <p key={line} className="mt-1 text-sm text-fg-muted">
          {line}
        </p>
      ))}
    </Card>
  );
}

/** "Semplice": each device with a problem, what Windows is saying and safe things to try. */
function ProblemDevices({ devices }: { devices: readonly DeviceRecord[] }) {
  const t = useT("drivers");
  const problems = useMemo(() => devices.filter(isProblem), [devices]);
  if (problems.length === 0) return null;
  return (
    <Card title={t("problemDevicesTitle")} description={simpleNoInstallNote(t)}>
      <ul className="flex flex-col gap-3">
        {problems.map((d) => {
          const explanation = deviceProblemExplanation(d, t);
          return (
            <li key={d.instanceId} className="rounded-md border border-line bg-surface-2/40 p-3">
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
