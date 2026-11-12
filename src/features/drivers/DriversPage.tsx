import { useEffect, useMemo } from "react";
import { Button } from "@/components/common/Button";
import { Panel } from "@/components/common/Panel";
import { Skeleton, SkeletonLines } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { Tooltip } from "@/components/common/Tooltip";
import { PageHeader } from "@/components/layout/PageHeader";
import { useIsSimple } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime } from "@/lib/format";
import type { DeviceRecord } from "@/types/generated/DeviceRecord";
import type { DriverSummary } from "@/types/generated/DriverSummary";
import { DeviceList, ProblemExplanationView } from "./DeviceList";
import { useDrivers } from "./driverStore";
import { categoryLabel, healthLabel, isProblem } from "./model";
import { deviceProblemExplanation, simpleDeviceName, simpleDriverSummary, simpleNoInstallNote } from "./simple";
import { UpdateCheckPanel } from "./UpdateCheckPanel";

/**
 * Device and driver inventory. Strictly read-only: nothing is installed or removed. Devices
 * with a problem come first, in their own panel; then the online update check; then the
 * full list (the page's focal panel). The counts live once, on the device list's filter
 * tabs ("Avanzata") or in the plain summary ("Semplice"); the reading notes once, under the
 * device list's title.
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
        <InventoryStateView />
      ) : (
        <div className="flex flex-col gap-4">
          <p className="px-1 text-xs text-fg-subtle tabular">{t("collectedAt", { date: formatDateTime(data.collectedAt) })}</p>
          {simple && <SimpleSummary summary={data.summary} />}
          <ProblemDevices devices={data.devices} simple={simple} />
          <UpdateCheckPanel />
          {data.devices.length === 0 ? (
            <Panel>
              <StateView layout="hero" kind="empty" title={t("emptyTitle")} description={t("summaryEmpty")} />
            </Panel>
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
    <Panel title={t("summaryAria")}>
      <p className="text-md leading-6 font-medium text-fg">{lines[0]}</p>
      {lines.slice(1).map((line) => (
        <p key={line} className="mt-1 text-sm text-fg-muted">
          {line}
        </p>
      ))}
    </Panel>
  );
}

/**
 * Devices with a problem, first on the page (both views): what Windows is saying and safe
 * things to try. Each one carries its state as a badge with words and a glyph.
 */
function ProblemDevices({ devices, simple }: { devices: readonly DeviceRecord[]; simple: boolean }) {
  const t = useT("drivers");
  const problems = useMemo(() => devices.filter(isProblem), [devices]);
  if (problems.length === 0) return null;
  return (
    <Panel title={t("problemDevicesTitle")} icon="warning" description={simpleNoInstallNote(t)}>
      <ul className="flex flex-col gap-3">
        {problems.map((d) => {
          const explanation = deviceProblemExplanation(d, t);
          const health = healthLabel(d.health, t);
          return (
            <li key={d.instanceId} className="surface-inset min-w-0 p-3">
              <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
                <div className="flex min-w-0 flex-wrap items-baseline gap-x-2">
                  <span className="min-w-0 text-sm font-medium break-words text-fg">
                    {simple ? simpleDeviceName(d, t) : d.name}
                  </span>
                  <span className="text-xs text-fg-subtle">{categoryLabel(d.category, t)}</span>
                </div>
                <StatusBadge tone={health.tone} label={health.label} glyph />
              </div>
              {explanation && <ProblemExplanationView device={d} explanation={explanation} />}
            </li>
          );
        })}
      </ul>
    </Panel>
  );
}

function InventoryStateView() {
  const t = useT("drivers");
  const status = useDrivers((s) => s.status);
  const error = useDrivers((s) => s.error);
  const reload = useDrivers((s) => s.reload);

  if (status !== "error" || !error) return <InventorySkeleton />;
  const kind = stateKindForError(error);
  if (kind === "unsupported") {
    return (
      <Panel>
        <StateView
          layout="hero"
          kind="unsupported"
          title={t("unsupportedTitle")}
          description={t("unsupportedDescription")}
          error={error}
        />
      </Panel>
    );
  }
  return (
    <Panel>
      <StateView layout="hero" kind={kind} title={t("unavailableTitle")} error={error} onRetry={reload} />
    </Panel>
  );
}

/**
 * First load (the inventory takes a few seconds): the shape of the page, with the loading
 * title and description as visible text so the wait is explained.
 */
function InventorySkeleton() {
  const t = useT("drivers");
  return (
    <div role="status" aria-busy="true" className="flex flex-col gap-4">
      <div className="surface-secondary p-4">
        <p className="text-sm font-medium text-fg">{t("loadingTitle")}</p>
        <p className="mt-0.5 text-xs text-fg-muted">{t("loadingDescription")}</p>
      </div>
      <div aria-hidden="true" className="surface-primary p-5">
        <Skeleton className="h-4 w-32" />
        <Skeleton round className="mt-4 h-7 w-80 max-w-full" />
        <div className="mt-6 flex flex-col gap-3">
          {Array.from({ length: 6 }, (_, i) => (
            <div key={i} className="flex items-center gap-4">
              <Skeleton className="h-3 w-1/3" />
              <Skeleton className="h-3 w-1/6" />
              <Skeleton round className="ml-auto h-5 w-24" />
            </div>
          ))}
        </div>
        <SkeletonLines lines={2} className="mt-6" />
      </div>
    </div>
  );
}

function RefreshButton({ disabled }: { disabled: boolean }) {
  const t = useT("drivers");
  const refreshing = useDrivers((s) => s.refreshing);
  const checking = useDrivers((s) => s.check.status === "running");
  const refresh = useDrivers((s) => s.refresh);
  // Disabled buttons get no hover: the tooltip sits on a wrapper. The reason is also the
  // button's description, so it is not tooltip-only for screen readers.
  return (
    <Tooltip content={t("refreshBlocked")} side="bottom-end" enabled={checking}>
      <span className="inline-flex">
        <Button
          icon="refresh"
          onClick={refresh}
          disabled={disabled || refreshing || checking}
          loading={refreshing}
          {...(checking ? { "aria-description": t("refreshBlocked") } : {})}
        >
          {refreshing ? t("refreshing") : t("refresh")}
        </Button>
      </span>
    </Tooltip>
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
