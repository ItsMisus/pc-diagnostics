import { useMemo } from "react";
import { Panel } from "@/components/common/Panel";
import { DefinitionList, type DefinitionItem } from "@/components/common/DefinitionList";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useIsSimple } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { formatBytes, sourceLabel } from "@/lib/format";
import { gpuVendorLabel } from "@/features/monitoring/labels";
import { hasUnifiedMemory, orderGpus } from "@/features/monitoring/selection";
import type { GpuInfo } from "@/types/generated/GpuInfo";
import { metricItem } from "./metricItem";

const text = (v: string) => v;

/** Graphics adapters' specs, physical ones first. Live values are on Overview. */
export function GpuSection({ gpus }: { gpus: readonly GpuInfo[] }) {
  const t = useT("hardware");
  const ordered = useMemo(() => orderGpus(gpus), [gpus]);
  return (
    <Panel title="GPU" icon="gpu" level={3}>
      {ordered.length === 0 ? (
        <StateView compact kind="empty" title={t("noGpuAdapters")} />
      ) : (
        // Its own size container: the panel can be half the page wide, so adapters go side
        // by side only when the panel itself has the room.
        <div className="@container">
          <div className="grid grid-cols-1 gap-x-10 gap-y-6 @2xl:grid-cols-2">
            {ordered.map((gpu) => (
              <GpuBlock key={gpu.id} gpu={gpu} />
            ))}
          </div>
        </div>
      )}
    </Panel>
  );
}

function GpuBlock({ gpu }: { gpu: GpuInfo }) {
  const t = useT("hardware");
  const tm = useT("monitoring");
  const simple = useIsSimple();
  const unified = hasUnifiedMemory(gpu);
  const items: DefinitionItem[] = [
    { label: t("manufacturer"), value: gpuVendorLabel(gpu.vendor, gpu.vendorId, tm) },
    // Apple silicon: no VRAM at all (unified memory), said in words instead of "0 B".
    unified ? { label: t("dedicatedVram"), value: t("unifiedMemory") } : metricItem(t("dedicatedVram"), gpu.dedicatedVramBytes, formatBytes),
    // With unified memory a missing budget says nothing useful: shown only when reported.
    ...(unified && gpu.sharedMemoryBytes.status === "unavailable" ? [] : [metricItem(t("sharedMemory"), gpu.sharedMemoryBytes, formatBytes)]),
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
      <header className="mb-2 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1.5">
        <h4 className="min-w-0 text-sm leading-5 font-semibold break-words text-fg">{gpu.name}</h4>
        {gpu.isSoftware && <StatusBadge tone="neutral" label={t("softwareAdapter")} />}
        {gpu.isVirtual && <StatusBadge tone="neutral" label={t("virtualDisplay")} />}
      </header>
      <DefinitionList divided items={items} />
    </article>
  );
}
