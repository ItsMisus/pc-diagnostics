import { useMemo } from "react";
import { Card } from "@/components/common/Card";
import { DefinitionList, type DefinitionItem } from "@/components/common/DefinitionList";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useIsSimple } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { formatBytes, sourceLabel } from "@/lib/format";
import { gpuVendorLabel } from "@/features/monitoring/labels";
import { orderGpus } from "@/features/monitoring/selection";
import type { GpuInfo } from "@/types/generated/GpuInfo";
import { metricItem } from "./metricItem";

const text = (v: string) => v;

/** Graphics adapters' specs, physical ones first. Live values are on Overview. */
export function GpuSection({ gpus }: { gpus: readonly GpuInfo[] }) {
  const t = useT("hardware");
  const ordered = useMemo(() => orderGpus(gpus), [gpus]);
  return (
    <Card title="GPU">
      {ordered.length === 0 ? (
        <StateView compact kind="empty" title={t("noGpuAdapters")} />
      ) : (
        // Its own size container: the card can be half the page wide (next to Memory), so
        // adapters go side by side only when the card itself has the room.
        <div className="@container">
          <div className="grid grid-cols-1 gap-x-8 gap-y-5 @2xl:grid-cols-2">
            {ordered.map((gpu) => (
              <GpuBlock key={gpu.id} gpu={gpu} />
            ))}
          </div>
        </div>
      )}
    </Card>
  );
}

function GpuBlock({ gpu }: { gpu: GpuInfo }) {
  const t = useT("hardware");
  const tm = useT("monitoring");
  const simple = useIsSimple();
  const items: DefinitionItem[] = [
    { label: t("manufacturer"), value: gpuVendorLabel(gpu.vendor, gpu.vendorId, tm) },
    { label: t("dedicatedVram"), value: formatBytes(gpu.dedicatedVramBytes) },
    { label: t("sharedMemory"), value: formatBytes(gpu.sharedMemoryBytes) },
    metricItem(t("driverVersion"), gpu.driverVersion, text),
  ];
  // Which providers measure this adapter: technical, like sources (Advanced view only).
  if (!simple) {
    items.push({
      label: t("telemetry"),
      value:
        gpu.telemetrySources.length > 0 ? (
          gpu.telemetrySources.map((s) => sourceLabel(s)).join(", ")
        ) : (
          <span className="text-fg-subtle">{t("noProvider")}</span>
        ),
    });
  }
  return (
    <article className="min-w-0" aria-label={gpu.name}>
      <header className="mb-2.5 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1.5">
        <h3 className="min-w-0 truncate text-sm leading-5 font-medium text-fg" title={gpu.name}>
          {gpu.name}
        </h3>
        {gpu.isSoftware && <StatusBadge tone="neutral" label={t("softwareAdapter")} />}
        {gpu.isVirtual && <StatusBadge tone="neutral" label={t("virtualDisplay")} />}
      </header>
      <DefinitionList aligned items={items} />
    </article>
  );
}
