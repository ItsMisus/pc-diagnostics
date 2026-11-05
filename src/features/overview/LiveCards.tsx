import { memo, useMemo, type ReactNode } from "react";
import { Card } from "@/components/common/Card";
import { MetricValue, UnavailableReason } from "@/components/common/MetricValue";
import { Skeleton } from "@/components/common/Skeleton";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { UsageBar } from "@/components/common/UsageBar";
import { type Translator, useT } from "@/i18n";
import { formatBytes, formatCelsius, formatMhz, formatPercent, formatRpm, formatTime } from "@/lib/format";
import { gpuPowerReading } from "@/features/monitoring/gpuPower";
import { DiskHealth } from "@/features/monitoring/DiskHealth";
import { useInventoryData } from "@/features/monitoring/hooks";
import { useLive } from "@/features/monitoring/liveStore";
import { primaryGpu, systemVolume } from "@/features/monitoring/selection";
import { valueOf } from "@/features/monitoring/series";
import { InventoryState, WaitingForSample } from "@/features/monitoring/StatusViews";
import type { Metric } from "@/types/generated/Metric";
import type { PhysicalDiskInfo } from "@/types/generated/PhysicalDiskInfo";

const pct = (v: number) => formatPercent(v);
const bytes = (v: number) => formatBytes(v);

/**
 * Anatomy shared by the four cards, top to bottom: quiet title (+ source or time on the
 * right), headline metric, caption, meter, a few secondary rows. The card is a flex column
 * so a trailing footnote can sit at the bottom when the grid stretches cards to equal height.
 */
const CARD = "flex flex-col";

/** Header meta (source, collection time), sized to the title's line box so both align. */
function HeaderMeta({ children }: { children: ReactNode }) {
  return <span className="flex h-5 items-center text-2xs text-fg-subtle tabular">{children}</span>;
}

function Headline({ children, caption }: { children: ReactNode; caption: string }) {
  return (
    <div className="mb-3 min-w-0">
      <p className="text-2xl leading-8 font-semibold tracking-tight text-fg tabular">{children}</p>
      <p className="truncate text-xs text-fg-muted" title={caption}>
        {caption}
      </p>
    </div>
  );
}

interface Stat {
  label: string;
  value: ReactNode;
}

/** Compact secondary values: label left, value right-aligned and tabular. */
function StatRows({ items }: { items: readonly Stat[] }) {
  return (
    <dl className="mt-4 flex flex-col gap-1.5 text-[13px] leading-5">
      {items.map((item) => (
        <div key={item.label} className="flex min-w-0 items-baseline justify-between gap-3">
          <dt className="min-w-0 truncate text-fg-muted">{item.label}</dt>
          <dd className="shrink-0 text-right text-fg tabular">{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}

const ROW_LABEL_W = ["w-20", "w-24", "w-16", "w-14"];

/**
 * First-load placeholder with the card's exact geometry (headline 32 + caption 16, meter,
 * rows of 20), so the first sample replaces it without moving anything.
 */
function MetricSkeleton({ rows }: { rows: number }) {
  return (
    <span aria-hidden="true" className="block">
      <span className="mb-3 block">
        <span className="flex h-8 items-center">
          <Skeleton className="h-6 w-20" />
        </span>
        <span className="flex h-4 items-center">
          <Skeleton className="h-2.5 w-28" />
        </span>
      </span>
      <Skeleton className="h-1 w-full rounded-full" />
      {rows > 0 && (
        <span className="mt-4 flex flex-col gap-1.5">
          {Array.from({ length: rows }, (_, i) => (
            <span key={i} className="flex h-5 items-center justify-between gap-3">
              <Skeleton className={`h-2.5 ${ROW_LABEL_W[i % ROW_LABEL_W.length]}`} />
              <Skeleton className="h-2.5 w-12" />
            </span>
          ))}
        </span>
      )}
    </span>
  );
}

function percentText(metric: Metric<number>, tm: Translator<"monitoring">): string {
  return metric.status === "available" ? formatPercent(metric.value) : tm("notAvailableText");
}

export const CpuCard = memo(function CpuCard() {
  const t = useT("overview");
  const tm = useT("monitoring");
  const cpu = useLive((s) => s.latest?.cpu);
  return (
    <Card
      title="CPU"
      className={CARD}
      aside={
        cpu && (
          <HeaderMeta>
            <SourceCaption metric={cpu.usagePercent} />
          </HeaderMeta>
        )
      }
    >
      {!cpu ? (
        <WaitingForSample>
          <MetricSkeleton rows={1} />
        </WaitingForSample>
      ) : (
        <>
          <Headline caption={t("cpuCaption")}>
            <MetricValue metric={cpu.usagePercent} format={pct} />
          </Headline>
          <UsageBar
            label={tm("cpuUsage")}
            percent={valueOf(cpu.usagePercent)}
            valueText={percentText(cpu.usagePercent, tm)}
          />
          <StatRows
            items={[{ label: tm("temperature"), value: <MetricValue metric={cpu.temperatureC} format={formatCelsius} /> }]}
          />
        </>
      )}
    </Card>
  );
});

export const GpuCard = memo(function GpuCard() {
  const t = useT("overview");
  const tm = useT("monitoring");
  const { status, data } = useInventoryData();
  const gpu = useMemo(() => (data ? primaryGpu(data.gpus) : null), [data]);

  return (
    <Card title="GPU" className={CARD}>
      {status !== "ready" || !data ? (
        <InventoryState compact placeholder={<MetricSkeleton rows={4} />} />
      ) : !gpu ? (
        <StateView
          compact
          kind="empty"
          title={tm("noHardwareGpu")}
          {...(data.gpus.length > 0 ? { description: t("onlySoftwareAdapters") } : {})}
        />
      ) : (
        <GpuLive id={gpu.id} name={gpu.name} />
      )}
    </Card>
  );
});

function GpuLive({ id, name }: { id: string; name: string }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  const hasSample = useLive((s) => s.latest !== null);
  const gpu = useLive((s) => s.latest?.gpus.find((g) => g.id === id));
  if (!hasSample) {
    return (
      <WaitingForSample>
        <MetricSkeleton rows={4} />
      </WaitingForSample>
    );
  }
  if (!gpu) {
    return <StateView compact kind="empty" title={tm("noSampleForAdapter")} description={name} />;
  }
  const power = gpuPowerReading(gpu, tm);
  return (
    <>
      <Headline caption={t("gpuCaption")}>
        <MetricValue metric={gpu.usagePercent} format={pct} />
      </Headline>
      <UsageBar
        label={tm("gpuUsage")}
        percent={valueOf(gpu.usagePercent)}
        valueText={percentText(gpu.usagePercent, tm)}
      />
      <StatRows
        items={[
          { label: tm("temperature"), value: <MetricValue metric={gpu.temperatureC} format={formatCelsius} /> },
          { label: tm("coreClock"), value: <MetricValue metric={gpu.coreClockMhz} format={formatMhz} /> },
          { label: tm("power"), value: <MetricValue metric={power.metric} format={power.format} /> },
          { label: tm("fan"), value: <MetricValue metric={gpu.fanRpm} format={formatRpm} /> },
        ]}
      />
      {/* Pinned to the bottom edge when the row stretches the card. */}
      <p className="mt-auto truncate pt-3 text-xs text-fg-subtle" title={name}>
        {name}
      </p>
    </>
  );
}

export const RamCard = memo(function RamCard() {
  const t = useT("overview");
  const tm = useT("monitoring");
  const mem = useLive((s) => s.latest?.memory);
  if (!mem) {
    return (
      <Card title="RAM" className={CARD}>
        <WaitingForSample>
          <MetricSkeleton rows={1} />
        </WaitingForSample>
      </Card>
    );
  }
  const used = valueOf(mem.usedBytes);
  const total = valueOf(mem.totalBytes);
  const percent = used !== null && total !== null && total > 0 ? (used / total) * 100 : null;
  return (
    <Card
      title="RAM"
      className={CARD}
      aside={
        <HeaderMeta>
          <SourceCaption metric={mem.usedBytes} />
        </HeaderMeta>
      }
    >
      <Headline caption={total !== null ? t("ramCaptionOf", { total: formatBytes(total) }) : t("ramCaption")}>
        <MetricValue metric={mem.usedBytes} format={bytes} />
      </Headline>
      <UsageBar
        label={tm("memoryInUse")}
        percent={percent}
        valueText={percent !== null ? formatPercent(percent) : tm("notAvailableText")}
      />
      <StatRows items={[{ label: tm("available"), value: <MetricValue metric={mem.availableBytes} format={bytes} /> }]} />
    </Card>
  );
});

export const SsdCard = memo(function SsdCard() {
  const t = useT("overview");
  const tm = useT("monitoring");
  const { status, data } = useInventoryData();
  const volume = useMemo(() => (data ? systemVolume(data.volumes) : null), [data]);

  if (status !== "ready" || !data) {
    return (
      <Card title="SSD" className={CARD}>
        <InventoryState compact placeholder={<MetricSkeleton rows={2} />} />
      </Card>
    );
  }

  const usedPercent =
    volume && volume.totalBytes > 0 ? ((volume.totalBytes - volume.availableBytes) / volume.totalBytes) * 100 : null;

  return (
    <Card
      title="SSD"
      className={CARD}
      aside={<HeaderMeta>{t("detectedAt", { time: formatTime(data.collectedAt) })}</HeaderMeta>}
    >
      {volume ? (
        <>
          <Headline caption={t("ssdCaption", { mount: volume.mountPoint, total: formatBytes(volume.totalBytes) })}>
            {formatBytes(volume.availableBytes)}
          </Headline>
          <UsageBar
            label={tm("usedSpaceOn", { mount: volume.mountPoint })}
            percent={usedPercent}
            valueText={
              usedPercent !== null ? tm("percentUsed", { percent: formatPercent(usedPercent) }) : tm("notAvailableText")
            }
          />
        </>
      ) : (
        <StateView compact kind="empty" title={tm("noVolumes")} />
      )}
      <h3 className="eyebrow mt-4 mb-2">{tm("windowsHealth")}</h3>
      <DiskHealthList disks={data.physicalDisks} />
    </Card>
  );
});

function DiskHealthList({ disks }: { disks: Metric<PhysicalDiskInfo[]> }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  const tc = useT("common");
  if (disks.status === "unavailable") {
    return (
      <p className="text-[13px] text-fg-subtle">
        {tc("notAvailable")} — <UnavailableReason reason={disks.reason} />
      </p>
    );
  }
  if (disks.value.length === 0) return <p className="text-[13px] text-fg-subtle">{t("noPhysicalDisks")}</p>;
  return (
    <ul className="flex flex-col gap-1.5">
      {disks.value.map((d) => {
        const name = d.model ?? tm("diskFallback", { id: d.id });
        return (
          <li key={d.id} className="flex items-center justify-between gap-2 text-[13px]">
            <span className="min-w-0 truncate" title={name}>
              {name}
            </span>
            <DiskHealth health={d.health} />
          </li>
        );
      })}
    </ul>
  );
}
