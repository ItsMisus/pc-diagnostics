import { Card } from "@/components/common/Card";
import { DefinitionList } from "@/components/common/DefinitionList";
import { MetricValue } from "@/components/common/MetricValue";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { NotReported, Table, Td } from "@/components/common/Table";
import { useT } from "@/i18n";
import { formatBytes, formatMts } from "@/lib/format";
import type { MemoryInfo } from "@/types/generated/MemoryInfo";
import type { MemoryModule } from "@/types/generated/MemoryModule";
import type { Metric } from "@/types/generated/Metric";

/** Installed memory and modules. Memory in use is on Overview. */
export function MemorySection({ memory }: { memory: MemoryInfo }) {
  const t = useT("hardware");
  return (
    <Card title={t("memoryTitle")}>
      <DefinitionList aligned items={[{ label: t("totalInstalled"), value: formatBytes(memory.totalBytes) }]} />
      <h3 className="eyebrow mt-5 mb-2.5">{t("modules")}</h3>
      <ModulesTable modules={memory.modules} />
    </Card>
  );
}

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
        headers={[t("slot"), t("capacity"), t("ratedSpeed"), t("configuredSpeed"), t("manufacturer"), t("partNumber")]}
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
      <p className="mt-2 empty:hidden">
        <SourceCaption metric={modules} />
      </p>
    </>
  );
}
