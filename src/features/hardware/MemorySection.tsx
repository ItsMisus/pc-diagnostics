import { DefinitionList, type DefinitionItem } from "@/components/common/DefinitionList";
import { MetricValue } from "@/components/common/MetricValue";
import { Panel } from "@/components/common/Panel";
import { SourceCaption } from "@/components/common/SourceCaption";
import { NotReported, Table, Td } from "@/components/common/Table";
import { useT } from "@/i18n";
import { formatBytes, formatMts } from "@/lib/format";
import type { MemoryInfo } from "@/types/generated/MemoryInfo";
import type { MemoryModule } from "@/types/generated/MemoryModule";
import type { Metric } from "@/types/generated/Metric";

/** True when the memory panel has a modules table (it needs the full page width). */
export function hasModulesTable(memory: MemoryInfo): boolean {
  return memory.modules.status === "available" && memory.modules.value.length > 0;
}

/** Installed memory and modules. Memory in use is on Overview. */
export function MemorySection({ memory }: { memory: MemoryInfo }) {
  const t = useT("hardware");
  const modules = memory.modules;
  const items: DefinitionItem[] = [
    { label: t("totalInstalled"), value: formatBytes(memory.totalBytes) },
    {
      label: t("modules"),
      value:
        modules.status === "unavailable" ? (
          <MetricValue metric={modules} format={() => ""} />
        ) : modules.value.length === 0 ? (
          <span className="text-fg-subtle">{t("noModules")}</span>
        ) : (
          modules.value.length
        ),
    },
  ];
  return (
    <Panel title={t("memoryTitle")} icon="memory">
      <DefinitionList divided items={items} />
      {modules.status === "available" && modules.value.length > 0 && <ModulesTable modules={modules} />}
    </Panel>
  );
}

function ModulesTable({ modules }: { modules: Extract<Metric<MemoryModule[]>, { status: "available" }> }) {
  const t = useT("hardware");
  return (
    <>
      <h3 className="eyebrow mt-6 mb-2">{t("moduleDetails")}</h3>
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
