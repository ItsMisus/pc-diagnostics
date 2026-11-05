import { memo, useMemo } from "react";
import { Card } from "@/components/common/Card";
import { DefinitionList } from "@/components/common/DefinitionList";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useT } from "@/i18n";
import { formatBytes, formatCelsius, formatMhz, formatPercent, formatRpm, sourceLabel } from "@/lib/format";
import { gpuPowerReading } from "@/features/monitoring/gpuPower";
import { gpuVendorLabel } from "@/features/monitoring/labels";
import { useLive } from "@/features/monitoring/liveStore";
import { orderGpus } from "@/features/monitoring/selection";
import { DefinitionSkeleton } from "@/features/monitoring/StatusViews";
import type { GpuInfo } from "@/types/generated/GpuInfo";
import { SPLIT } from "./layout";
import { LiveSlot, SubHeading } from "./LiveSlot";
import { metricItem } from "./metricItem";

const pct = (v: number) => formatPercent(v);
const text = (v: string) => v;

export function GpuSection({ gpus }: { gpus: readonly GpuInfo[] }) {
  const t = useT("hardware");
  const ordered = useMemo(() => orderGpus(gpus), [gpus]);
  return (
    <Card title="GPU">
      {ordered.length === 0 ? (
        <StateView compact kind="empty" title={t("noGpuAdapters")} />
      ) : (
        <div className="flex flex-col divide-y divide-line">
          {ordered.map((gpu) => (
            <GpuBlock key={gpu.id} gpu={gpu} />
          ))}
        </div>
      )}
    </Card>
  );
}

function GpuBlock({ gpu }: { gpu: GpuInfo }) {
  const t = useT("hardware");
  const tm = useT("monitoring");
  return (
    <article className="py-5 first:pt-0 last:pb-0" aria-label={gpu.name}>
      <header className="mb-4 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1.5">
        <h3 className="min-w-0 truncate text-sm leading-5 font-medium text-fg" title={gpu.name}>
          {gpu.name}
        </h3>
        {gpu.isSoftware && <StatusBadge tone="neutral" label={t("softwareAdapter")} />}
        {gpu.isVirtual && <StatusBadge tone="neutral" label={t("virtualDisplay")} />}
      </header>
      <div className={SPLIT}>
        <div className="min-w-0">
          <SubHeading level={4}>{t("specs")}</SubHeading>
          <DefinitionList
            items={[
              { label: t("manufacturer"), value: gpuVendorLabel(gpu.vendor, gpu.vendorId, tm) },
              { label: t("dedicatedVram"), value: formatBytes(gpu.dedicatedVramBytes) },
              { label: t("sharedMemory"), value: formatBytes(gpu.sharedMemoryBytes) },
              metricItem(t("driverVersion"), gpu.driverVersion, text),
              {
                label: t("telemetry"),
                value:
                  gpu.telemetrySources.length > 0 ? (
                    gpu.telemetrySources.map((s) => sourceLabel(s)).join(", ")
                  ) : (
                    <span className="text-fg-subtle">{t("noProvider")}</span>
                  ),
              },
            ]}
          />
        </div>
        <div className="min-w-0">
          <SubHeading level={4}>{t("live")}</SubHeading>
          <LiveSlot placeholder={<DefinitionSkeleton rows={6} />}>
            <GpuLive id={gpu.id} dedicatedVramBytes={gpu.dedicatedVramBytes} />
          </LiveSlot>
        </div>
      </div>
    </article>
  );
}

const GpuLive = memo(function GpuLive({ id, dedicatedVramBytes }: { id: string; dedicatedVramBytes: number }) {
  const t = useT("hardware");
  const tm = useT("monitoring");
  const sample = useLive((s) => s.latest?.gpus.find((g) => g.id === id));
  if (!sample) return <p className="text-sm text-fg-subtle">{t("noSample")}</p>;
  const power = gpuPowerReading(sample, tm);
  const vram = (v: number) =>
    dedicatedVramBytes > 0 ? t("ofTotal", { used: formatBytes(v), total: formatBytes(dedicatedVramBytes) }) : formatBytes(v);
  return (
    <DefinitionList
      items={[
        metricItem(t("usage"), sample.usagePercent, pct),
        metricItem(t("vramInUse"), sample.dedicatedUsedBytes, vram),
        metricItem(tm("temperature"), sample.temperatureC, formatCelsius),
        metricItem(tm("coreClock"), sample.coreClockMhz, formatMhz),
        metricItem(tm("power"), power.metric, power.format),
        metricItem(tm("fan"), sample.fanRpm, formatRpm),
      ]}
    />
  );
});
