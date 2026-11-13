import { Panel } from "@/components/common/Panel";
import { DefinitionList } from "@/components/common/DefinitionList";
import { NotReported } from "@/components/common/Table";
import { useT } from "@/i18n";
import { formatMhz } from "@/lib/format";
import type { CpuInfo } from "@/types/generated/CpuInfo";
import { metricItem } from "./metricItem";

/** Processor specs. Live values (usage, clock, per-core load) are on Overview. */
export function CpuSection({ cpu }: { cpu: CpuInfo }) {
  const t = useT("hardware");
  return (
    <Panel title="CPU" icon="cpu" level={3}>
      <DefinitionList
        divided
        items={[
          { label: t("model"), value: cpu.brand ?? <NotReported /> },
          { label: t("manufacturer"), value: cpu.vendor ?? <NotReported /> },
          { label: t("architecture"), value: cpu.arch },
          { label: t("physicalCores"), value: cpu.physicalCores ?? <NotReported /> },
          { label: t("logicalThreads"), value: cpu.logicalCores },
          metricItem(t("maxNominalFrequency"), cpu.maxMhz, formatMhz),
        ]}
      />
    </Panel>
  );
}
