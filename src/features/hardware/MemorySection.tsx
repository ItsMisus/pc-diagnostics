import { memo } from "react";
import { Card } from "@/components/common/Card";
import { DefinitionList } from "@/components/common/DefinitionList";
import { MetricValue } from "@/components/common/MetricValue";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { NotReported, Table, Td } from "@/components/common/Table";
import { useT } from "@/i18n";
import { formatBytes, formatMts, formatPerSec } from "@/lib/format";
import { useLive } from "@/features/monitoring/liveStore";
import { DefinitionSkeleton } from "@/features/monitoring/StatusViews";
import type { MemoryInfo } from "@/types/generated/MemoryInfo";
import type { MemoryModule } from "@/types/generated/MemoryModule";
import type { Metric } from "@/types/generated/Metric";
import { SPLIT } from "./layout";
import { LiveSlot, SubHeading } from "./LiveSlot";
import { metricItem } from "./metricItem";

const bytes = (v: number) => formatBytes(v);

export function MemorySection({ memory }: { memory: MemoryInfo }) {
  const t = useT("hardware");
  return (
    <Card title="Memory">
      <div className={SPLIT}>
        <div className="min-w-0">
          <SubHeading>{t("specs")}</SubHeading>
          <DefinitionList items={[{ label: t("totalInstalled"), value: formatBytes(memory.totalBytes) }]} />
        </div>
        <div className="min-w-0">
          <SubHeading>{t("live")}</SubHeading>
          <LiveSlot placeholder={<DefinitionSkeleton rows={4} />}>
            <MemoryLive />
          </LiveSlot>
        </div>
      </div>
      <div className="mt-6">
        <SubHeading>{t("modules")}</SubHeading>
        <ModulesTable modules={memory.modules} />
      </div>
    </Card>
  );
}

const MemoryLive = memo(function MemoryLive() {
  const t = useT("hardware");
  const tm = useT("monitoring");
  const mem = useLive((s) => s.latest?.memory);
  if (!mem) return null;
  return (
    <DefinitionList
      items={[
        metricItem(t("inUse"), mem.usedBytes, bytes),
        metricItem(tm("available"), mem.availableBytes, bytes),
        {
          label: t("committedMemory"),
          value: t.rich("ofTotal", {
            used: <MetricValue metric={mem.commitUsedBytes} format={bytes} />,
            total: <MetricValue metric={mem.commitLimitBytes} format={bytes} />,
          }),
          caption: <SourceCaption metric={mem.commitUsedBytes} />,
        },
        metricItem(t("pagesFromDisk"), mem.pagesPerSec, formatPerSec),
      ]}
    />
  );
});

function ModulesTable({ modules }: { modules: Metric<MemoryModule[]> }) {
  const t = useT("hardware");
  if (modules.status === "unavailable") {
    return <MetricValue metric={modules} format={() => ""} className="text-sm" />;
  }
  if (modules.value.length === 0) {
    return <StateView compact kind="empty" title={t("noModules")} />;
  }
  return (
    <>
      <Table
        caption={t("modulesCaption")}
        headers={[
          t("slot"),
          t("capacity"),
          t("ratedSpeed"),
          t("configuredSpeed"),
          t("manufacturer"),
          t("partNumber"),
        ]}
      >
        {modules.value.map((m, i) => (
          <tr key={`${m.slot ?? "slot"}-${i}`}>
            <Td>{m.slot ?? <NotReported />}</Td>
            <Td>{formatBytes(m.capacityBytes)}</Td>
            <Td>{m.speedMts !== null ? formatMts(m.speedMts) : <NotReported />}</Td>
            <Td>{m.configuredSpeedMts !== null ? formatMts(m.configuredSpeedMts) : <NotReported />}</Td>
            <Td>{m.manufacturer ?? <NotReported />}</Td>
            <Td className="font-mono text-xs">{m.partNumber ?? <NotReported />}</Td>
          </tr>
        ))}
      </Table>
      <p className="mt-2">
        <SourceCaption metric={modules} />
      </p>
    </>
  );
}
