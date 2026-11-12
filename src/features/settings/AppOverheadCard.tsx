import { Panel } from "@/components/common/Panel";
import { SkeletonLines } from "@/components/common/Skeleton";
import { DefinitionList } from "@/components/common/DefinitionList";
import { MetricValue } from "@/components/common/MetricValue";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { Table, Td } from "@/components/common/Table";
import { useId } from "react";
import { usePolling } from "@/hooks/usePolling";
import { stateKindForError } from "@/lib/errors";
import { formatBytes, formatDuration, formatPercent } from "@/lib/format";
import { useT } from "@/i18n";
import { commands } from "@/services/commands";
import type { ResourceUsage } from "@/types/generated/ResourceUsage";

/** The backend refreshes its overhead figures about every 5 s; polling faster is pointless. */
const POLL_MS = 5_000;

const fetchOverview = () => Promise.all([commands.getAppOverhead(), commands.getMonitorStatus()]);
const pct = (v: number) => formatPercent(v, 1);
/** Same look as `<Table>`'s own headers. */
const TH = "pr-4 pb-2 text-2xs font-medium whitespace-nowrap text-fg-subtle";
const bytes = (v: number) => formatBytes(v);

export function AppOverheadCard() {
  const [state, retry] = usePolling(fetchOverview, POLL_MS);
  const t = useT("settings");

  return (
    <Panel title={t("overhead")} icon="gauge" level={3}>
      {state.status === "loading" && (
        <div role="status" aria-busy="true">
          <span className="sr-only">{t("overhead")}</span>
          <SkeletonLines lines={4} />
        </div>
      )}
      {state.status === "error" && (
        <StateView compact kind={stateKindForError(state.error)} error={state.error} onRetry={retry} />
      )}
      {state.status === "ready" && <OverheadDetails data={state.data} />}
    </Panel>
  );
}

function OverheadDetails({ data }: { data: Awaited<ReturnType<typeof fetchOverview>> }) {
  const [overhead, monitor] = data;
  const historySpanS = (monitor.historyCapacity * monitor.intervalMs) / 1000;
  const t = useT("settings");
  const cpuNote = t("overheadCpuNote", { window: formatDuration(overhead.windowMs / 1000) });
  const cpuNoteId = useId();
  return (
    <div className="flex flex-col">
      <Table
        caption={t("overheadCaption")}
        head={
          <>
            <th scope="col" className={`${TH} text-left`}>
              {t("colComponent")}
            </th>
            <th scope="col" className={`${TH} text-right`}>
              <span aria-describedby={cpuNoteId}>{t("colCpu")}</span>
            </th>
            <th scope="col" className={`${TH} text-right`}>
              {t("colMemory")}
            </th>
            <th scope="col" className={`${TH} text-right`}>
              {t("colProcesses")}
            </th>
          </>
        }
      >
        <UsageRow label={t("rowCore")} usage={overhead.core} />
        <UsageRow label={t("rowWebview")} usage={overhead.webview} />
      </Table>
      {/* Visible, not in a tooltip: the CPU share is easy to misread without it. */}
      <p id={cpuNoteId} className="mt-2 text-2xs text-fg-subtle">
        {t("colCpu")} · {cpuNote}
      </p>
      <DefinitionList
        className="mt-4 border-t border-line pt-3.5"
        items={[
          {
            label: t("historyCapacity"),
            value: t("historyCapacityValue", { count: monitor.historyCapacity, duration: formatDuration(historySpanS) }),
          },
          { label: t("samplesCollected"), value: monitor.samplesCollected },
        ]}
      />
    </div>
  );
}

function UsageRow({ label, usage }: { label: string; usage: ResourceUsage }) {
  return (
    <tr>
      <Td>{label}</Td>
      <Td className="text-right whitespace-nowrap">
        <MetricValue metric={usage.cpuPercent} format={pct} /> <SourceCaption metric={usage.cpuPercent} />
      </Td>
      <Td className="text-right whitespace-nowrap">
        <MetricValue metric={usage.workingSetBytes} format={bytes} />
      </Td>
      <Td className="text-right">{usage.processCount}</Td>
    </tr>
  );
}
