import { useEffect } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { Skeleton } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { Table, Td } from "@/components/common/Table";
import { useT } from "@/i18n";
import { formatBytes, formatBytesPerSec, formatPercent } from "@/lib/format";
import { processStore, useProcesses } from "@/features/processes/processStore";
import { ioTotal } from "@/features/processes/rows";
import type { ProcessRow } from "@/types/generated/ProcessRow";

/** Rows shown on the dashboard; the full ranking is on Processes. */
const ROWS = 5;
const EMPTY: readonly ProcessRow[] = [];
const TH = "pr-4 pb-2 text-2xs font-medium whitespace-nowrap text-fg-subtle";

const pct = (v: number) => formatPercent(v, 1);
const bytes = (v: number) => formatBytes(v);
const rate = (v: number) => formatBytesPerSec(v);

/**
 * The few processes with the highest impact right now. Reuses the Processes page store: the
 * backend samples processes regardless (every 2 s), so this only attaches a listener while
 * Overview is shown; no extra polling.
 */
export function TopProcesses({ className = "" }: { className?: string }) {
  const t = useT("overview");
  const setSection = useAppStore((s) => s.setSection);
  useEffect(() => processStore.getState().start(), []);
  return (
    <Card
      title={t("processesTitle")}
      className={className}
      aside={
        <Button variant="ghost" size="sm" className="-my-1 -mr-2" onClick={() => setSection("processes")}>
          {t("openProcesses")}
          <Icon name="chevronRight" size={14} className="-mr-0.5" />
        </Button>
      }
    >
      <TopProcessesBody />
    </Card>
  );
}

function TopProcessesBody() {
  const t = useT("overview");
  const status = useProcesses((s) => s.status);
  const error = useProcesses((s) => s.error);
  const retry = useProcesses((s) => s.retry);
  const ranking = useProcesses((s) => s.snapshot?.rankingInstant ?? EMPTY);

  if (status === "unsupported") {
    return <StateView compact kind="unsupported" title={t("processesUnavailable")} {...(error ? { error } : {})} />;
  }
  if (status === "permission" || status === "consent" || status === "error") {
    return (
      <StateView compact kind={status} title={t("processesUnavailable")} {...(error ? { error } : {})} onRetry={retry} />
    );
  }
  if (status !== "live") {
    return (
      <div role="status" aria-busy="true">
        <span className="sr-only">{t("processesWaiting")}</span>
        <span aria-hidden="true" className="flex flex-col gap-3 pt-1">
          {Array.from({ length: ROWS }, (_, i) => (
            <Skeleton key={i} className="h-3 w-full" />
          ))}
        </span>
      </div>
    );
  }
  if (ranking.length === 0) return <p className="text-sm text-fg-muted">{t("noProcesses")}</p>;

  return (
    <Table
      caption={t("topProcessesCaption")}
      head={
        <>
          <th scope="col" className={`${TH} text-left`}>
            {t("colName")}
          </th>
          <th scope="col" className={`${TH} text-right`}>
            CPU
          </th>
          <th scope="col" className={`${TH} text-right`}>
            RAM
          </th>
          <th scope="col" className={`${TH} pr-0 text-right`} title={t("ioNote")}>
            I/O<span className="sr-only">. {t("ioNote")}</span>
          </th>
        </>
      }
    >
      {ranking.slice(0, ROWS).map((row) => (
        <tr key={row.key} className={row.accessible ? "" : "text-fg-muted"}>
          <Td className="w-1/2 max-w-0 truncate">
            <span title={row.name}>{row.name}</span>
          </Td>
          <Td className="text-right">
            <MetricValue metric={row.cpuPercent} format={pct} />
          </Td>
          <Td className="text-right">
            <MetricValue metric={row.workingSetBytes} format={bytes} />
          </Td>
          <Td className="pr-0 text-right">
            <MetricValue metric={ioTotal(row)} format={rate} />
          </Td>
        </tr>
      ))}
    </Table>
  );
}
