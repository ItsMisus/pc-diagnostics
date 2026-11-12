import { memo, type ReactNode, useMemo } from "react";
import { Sparkline } from "@/components/charts/Sparkline";
import { Icon, type IconName } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { Readout } from "@/components/common/Readout";
import { Skeleton } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { UsageBar } from "@/components/common/UsageBar";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { useInventoryData, useSeries } from "@/features/monitoring/hooks";
import { useLive } from "@/features/monitoring/liveStore";
import { primaryGpu, systemVolume, volumeDisplayName } from "@/features/monitoring/selection";
import { cpuUsage, gpuUsage, memoryUsedPercent, PERCENT_DOMAIN, type Pick, valueOf } from "@/features/monitoring/series";
import { InventoryState, WaitingForSample } from "@/features/monitoring/StatusViews";
import { useViewMode } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { formatBytes, formatCelsius, formatPercent, formatStorageBytes } from "@/lib/format";
import type { Metric } from "@/types/generated/Metric";
import { type Level, memoryLevel, volumeLevel } from "./levels";
import { recentWindow } from "./model";
import { LevelLabel, splitUnit, toneOf, useLevelText } from "./parts";

/**
 * The essential readings in one row: CPU load, memory in use, GPU load and free space on the
 * system volume. Each tile writes its value as text; the sparkline next to it only shows the
 * last minute's shape (fixed 0–100 scale, never animated). Warn/bad tones only from levels.ts.
 */
export function EssentialMetrics({ headingId }: { headingId: string }) {
  const t = useT("overview");
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4">
      <SectionHeading id={headingId}>{t("essentialsTitle")}</SectionHeading>
      <div className="grid grid-cols-1 gap-4 @md:grid-cols-2 @4xl:grid-cols-4">
        <CpuTile />
        <MemoryTile />
        <GpuTile />
        <StorageTile />
      </div>
    </section>
  );
}

function Tile({ icon, label, children }: { icon: IconName; label: string; children: ReactNode }) {
  return (
    <div className="surface-secondary flex min-w-0 flex-col gap-2 p-4">
      <h3 className="flex min-w-0 items-center gap-2 text-sm font-medium text-fg-muted">
        <Icon name={icon} size={16} className="shrink-0 text-fg-subtle" />
        <span className="truncate">{label}</span>
      </h3>
      {children}
    </div>
  );
}

/** First-sample placeholder with the tile's geometry (value, line, sparkline). */
function TileSkeleton() {
  return (
    <span aria-hidden="true" className="flex flex-col gap-2">
      <Skeleton className="h-7 w-20" />
      <Skeleton className="h-3 w-28" />
      <Skeleton className="h-8 w-full" />
    </span>
  );
}

/**
 * A 0–100 value as a readout ("42 %"), or "n/d" with its reason. `metric` is the reading the
 * percent comes from; when the percent cannot be computed, `metric` must be an unavailable
 * one (its reason is shown), never an available value of another unit.
 */
function PercentReadout({ metric, percent, level }: { metric: Metric<number>; percent: number | null; level?: Level }) {
  if (percent === null || metric.status === "unavailable") {
    if (metric.status === "available") {
      // Nothing to state a reason with (e.g. a zero total): plain "n/d", never the raw value.
      return <NotAvailable />;
    }
    return <Readout size="md" value={<MetricValue<number> metric={metric} format={(v) => formatPercent(v)} />} />;
  }
  const [value, unit] = splitUnit(formatPercent(percent));
  return (
    <span className="flex flex-wrap items-baseline gap-x-2">
      <Readout size="md" value={value} {...(unit ? { unit } : {})} />
      <LevelLabel level={level} />
    </span>
  );
}

function NotAvailable() {
  const tc = useT("common");
  return <Readout size="md" value={<span className="text-fg-subtle">{tc("notAvailable")}</span>} />;
}

/** The last minute of a percent series, next to the written value. */
function Spark({ pick, label }: { pick: Pick; label: string }) {
  const t = useT("overview");
  const points = useSeries(pick);
  const w = useMemo(() => recentWindow(points), [points]);
  return <Sparkline points={w.points} xDomain={w.domain} yDomain={PERCENT_DOMAIN} ariaLabel={t("sparkAria", { label })} markLast />;
}

const Footer = ({ children }: { children: ReactNode }) => <p className="min-h-4 truncate text-xs text-fg-subtle tabular">{children}</p>;

// ---------------------------------------------------------------- CPU

const CpuTile = memo(function CpuTile() {
  const t = useT("overview");
  const usage = useLive((s) => s.latest?.cpu.usagePercent);
  const temp = useLive((s) => s.latest?.cpu.temperatureC);
  return (
    <Tile icon="cpu" label={t("cpuLoad")}>
      {!usage ? (
        <WaitingForSample>
          <TileSkeleton />
        </WaitingForSample>
      ) : (
        <>
          <PercentReadout metric={usage} percent={valueOf(usage)} />
          <Footer>{temp?.status === "available" ? t("temperatureValue", { value: formatCelsius(temp.value) }) : null}</Footer>
          <Spark pick={cpuUsage} label={t("cpuLoad")} />
        </>
      )}
    </Tile>
  );
});

// ---------------------------------------------------------------- Memory

const MemoryTile = memo(function MemoryTile() {
  // Byte labels follow the view mode (lib/format.ts).
  useViewMode();
  const t = useT("overview");
  const mem = useLive((s) => s.latest?.memory);
  if (!mem) {
    return (
      <Tile icon="memory" label={t("memoryUsed")}>
        <WaitingForSample>
          <TileSkeleton />
        </WaitingForSample>
      </Tile>
    );
  }
  const used = valueOf(mem.usedBytes);
  const total = valueOf(mem.totalBytes);
  const percent = used !== null && total !== null && total > 0 ? (used / total) * 100 : null;
  const level = memoryLevel(valueOf(mem.availableBytes), total);
  return (
    <Tile icon="memory" label={t("memoryUsed")}>
      <PercentReadout metric={mem.usedBytes.status === "unavailable" ? mem.usedBytes : mem.totalBytes} percent={percent} level={level} />
      <Footer>{used !== null && total !== null ? t("ofTotal", { used: formatBytes(used), total: formatBytes(total) }) : null}</Footer>
      <Spark pick={memoryUsedPercent} label={t("memoryUsed")} />
    </Tile>
  );
});

// ---------------------------------------------------------------- GPU

const GpuTile = memo(function GpuTile() {
  const t = useT("overview");
  const { status, data } = useInventoryData();
  const gpu = useMemo(() => (data ? primaryGpu(data.gpus) : null), [data]);
  return (
    <Tile icon="gpu" label={t("gpuLoad")}>
      {status !== "ready" || !data ? (
        <InventoryState compact placeholder={<TileSkeleton />} />
      ) : !gpu ? (
        <StateView compact kind="empty" title={t("noGpu")} />
      ) : (
        <GpuReading id={gpu.id} />
      )}
    </Tile>
  );
});

function GpuReading({ id }: { id: string }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  const hasSample = useLive((s) => s.latest !== null);
  const usage = useLive((s) => s.latest?.gpus.find((g) => g.id === id)?.usagePercent);
  const temp = useLive((s) => s.latest?.gpus.find((g) => g.id === id)?.temperatureC);
  const pick = useMemo(() => gpuUsage(id), [id]);
  if (!hasSample) {
    return (
      <WaitingForSample>
        <TileSkeleton />
      </WaitingForSample>
    );
  }
  if (!usage) return <StateView compact kind="empty" title={tm("noSampleForAdapter")} />;
  return (
    <>
      <PercentReadout metric={usage} percent={valueOf(usage)} />
      <Footer>{temp?.status === "available" ? t("temperatureValue", { value: formatCelsius(temp.value) }) : null}</Footer>
      <Spark pick={pick} label={t("gpuLoad")} />
    </>
  );
}

// ---------------------------------------------------------------- Storage (inventory, not live)

const StorageTile = memo(function StorageTile() {
  // Byte labels follow the view mode (lib/format.ts).
  useViewMode();
  const t = useT("overview");
  const tm = useT("monitoring");
  const { status, data } = useInventoryData();
  const volume = useMemo(() => (data ? systemVolume(data.volumes) : null), [data]);
  const used = volume ? volume.totalBytes - volume.availableBytes : 0;
  const percent = volume && volume.totalBytes > 0 ? (used / volume.totalBytes) * 100 : null;
  const level = volume ? volumeLevel(volume.availableBytes, volume.totalBytes) : undefined;
  const valueText = useLevelText(percent !== null ? tm("percentUsed", { percent: formatPercent(percent) }) : tm("notAvailableText"), level);

  return (
    <Tile icon="storage" label={t("storageFree")}>
      {status !== "ready" || !data ? (
        <InventoryState compact placeholder={<TileSkeleton />} />
      ) : !volume ? (
        <StateView compact kind="empty" title={tm("noVolumes")} />
      ) : (
        <>
          <StorageReadout available={volume.availableBytes} level={level} />
          <Footer>{t("freeOf", { total: formatStorageBytes(volume.totalBytes) })}</Footer>
          {/* Same height as a sparkline, so the tiles line up. */}
          <div className="flex h-8 flex-col justify-end gap-1.5">
            <p className="truncate text-2xs text-fg-subtle" title={volumeDisplayName(volume)}>
              {volumeDisplayName(volume)}
            </p>
            <UsageBar percent={percent} label={tm("usedSpaceOn", { mount: volume.mountPoint })} valueText={valueText} tone={toneOf(level)} />
          </div>
        </>
      )}
    </Tile>
  );
});

function StorageReadout({ available, level }: { available: number; level: Level | undefined }) {
  const [value, unit] = splitUnit(formatStorageBytes(available));
  return (
    <span className="flex flex-wrap items-baseline gap-x-2">
      <Readout size="md" value={value} {...(unit ? { unit } : {})} />
      <LevelLabel level={level} />
    </span>
  );
}
