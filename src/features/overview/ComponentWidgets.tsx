import { memo, type ReactNode, useMemo } from "react";
import { Sparkline } from "@/components/charts/Sparkline";
import type { Domain, Point } from "@/components/charts/scale";
import { Icon, type IconName } from "@/components/common/Icon";
import { MetricValue, UnavailableReason } from "@/components/common/MetricValue";
import { Readout } from "@/components/common/Readout";
import { Skeleton } from "@/components/common/Skeleton";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { Tooltip } from "@/components/common/Tooltip";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { DiskHealth } from "@/features/monitoring/DiskHealth";
import { gpuPowerReading } from "@/features/monitoring/gpuPower";
import { useInventoryData, useSeries } from "@/features/monitoring/hooks";
import { useLive } from "@/features/monitoring/liveStore";
import { dedicatedVram, hasUnifiedMemory, isFrequencyLimited, primaryGpu, showsDiskStatus, volumeDisplayName } from "@/features/monitoring/selection";
import {
  cpuUsage,
  diskRead,
  diskWrite,
  gpuUsage,
  memoryUsedPercent,
  PERCENT_DOMAIN,
  type Pick,
  throughputDomain,
  valueOf,
} from "@/features/monitoring/series";
import { InventoryState, WaitingForSample } from "@/features/monitoring/StatusViews";
import { useIsSimple, useViewMode } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import {
  formatBytes,
  formatBytesPerSec,
  formatCelsius,
  formatMhz,
  formatPercent,
  formatPerSec,
  formatRpm,
  formatStorageBytes,
  formatTime,
  formatWatts,
} from "@/lib/format";
import { useIsMac, usePlatform } from "@/lib/platform";
import type { CpuSample } from "@/types/generated/CpuSample";
import type { GpuSample } from "@/types/generated/GpuSample";
import type { Metric } from "@/types/generated/Metric";
import type { PhysicalDiskInfo } from "@/types/generated/PhysicalDiskInfo";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";
import { commitLevel, type Level, memoryLevel, pressureLevel, volumeLevel, vramLevel } from "./levels";
import { recentWindow } from "./model";
import { LevelLabel, splitUnit, toneOf, useLevelText } from "./parts";

/** Trend window of the widgets: the last two minutes. */
const TREND_MS = 120_000;
const TREND_HEIGHT = 64;

/**
 * The live components as widgets (docs/design-system.md, "Overview"): each one has the
 * current value written large, its last two minutes as an area trend, and its secondary
 * readings in a compact grid. Readings the platform cannot provide at all are not shown as a
 * row of "n/d": they are named once in a footnote with the reason ("Non disponibili su questo
 * Mac: …"), so the widget stays readable. Temporarily missing values still show "n/d".
 */
export function ComponentWidgets({ headingId, liveFailed }: { headingId: string; liveFailed: ReactNode | null }) {
  const t = useT("overview");
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4">
      <SectionHeading id={headingId}>{t("componentsTitle")}</SectionHeading>
      {liveFailed ?? (
        <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
          <CpuWidget />
          <MemoryWidget />
          <GpuWidget />
          <DiskWidget />
        </div>
      )}
      <StorageWidget />
    </section>
  );
}

// ---------------------------------------------------------------- Building blocks

function Widget({ icon, title, aside, children }: { icon: IconName; title: string; aside?: ReactNode; children: ReactNode }) {
  return (
    <article className="widget flex min-w-0 flex-col gap-4 p-5">
      <header className="flex min-w-0 items-center justify-between gap-3">
        <h3 className="flex min-w-0 items-center gap-2.5 text-md font-semibold text-fg">
          <span aria-hidden="true" className="satin grid size-8 shrink-0 place-items-center rounded-full text-fg-muted">
            <Icon name={icon} size={16} />
          </span>
          <span className="truncate">{title}</span>
        </h3>
        {aside && <div className="flex min-w-0 shrink items-center gap-2 text-xs text-fg-subtle">{aside}</div>}
      </header>
      {children}
    </article>
  );
}

/** Big value on the left, its trend filling the rest of the row. */
function Headline({ value, trend }: { value: ReactNode; trend: ReactNode }) {
  return (
    <div className="flex min-w-0 items-end gap-5">
      <div className="flex min-w-24 shrink-0 flex-col">{value}</div>
      <div className="min-w-0 flex-1">{trend}</div>
    </div>
  );
}

function BigPercent({ metric, percent, caption, level }: { metric: Metric<number>; percent: number | null; caption: string; level?: Level }) {
  const tc = useT("common");
  const text = useLevelText(percent === null ? "" : formatPercent(percent), level);
  const tone = toneOf(level);
  if (percent === null || metric.status === "unavailable") {
    return (
      <>
        <Readout
          size="lg"
          value={metric.status === "unavailable" ? <MetricValue<number> metric={metric} format={(v) => formatPercent(v)} /> : <span className="text-fg-subtle">{tc("notAvailable")}</span>}
        />
        <span className="text-xs text-fg-muted">{caption}</span>
      </>
    );
  }
  const [value, unit] = splitUnit(formatPercent(percent));
  return (
    <>
      <span aria-label={text} className={tone === "bad" ? "text-bad" : tone === "warn" ? "text-warn" : undefined}>
        <Readout size="lg" value={value} {...(unit ? { unit } : {})} className={tone ? "text-current!" : ""} />
      </span>
      <span className="flex items-center gap-2 text-xs text-fg-muted">
        {caption}
        <LevelLabel level={level} />
      </span>
    </>
  );
}

function Trend({ points, domain, label, tone = "primary" }: { points: Point[]; domain: Domain; label: string; tone?: "primary" | "accent" }) {
  const w = useMemo(() => recentWindow(points, TREND_MS), [points]);
  return <Sparkline points={w.points} xDomain={w.domain} yDomain={domain} ariaLabel={label} height={TREND_HEIGHT} tone={tone} markLast />;
}

function PercentTrend({ pick, label }: { pick: Pick; label: string }) {
  const t = useT("overview");
  const points = useSeries(pick);
  return <Trend points={points} domain={PERCENT_DOMAIN} label={t("trendAria", { label })} />;
}

export interface StatItem {
  label: string;
  metric?: Metric<number>;
  format?: (v: number) => string;
  /** A ready value instead of a metric (e.g. "Nessuna: memoria unificata"). */
  value?: ReactNode;
  level?: Level;
  /** Spans two cells (a value in words, e.g. "Nessuna: memoria unificata"). */
  wide?: boolean;
}

/**
 * Secondary readings in a grid of label-over-value cells. Metrics the platform does not
 * support (`notSupported`) leave the grid and are listed once in `unsupportedNote`.
 */
function Stats({ items }: { items: StatItem[] }) {
  const t = useT("overview");
  const shown = items.filter((i) => !(i.metric?.status === "unavailable" && i.metric.reason.kind === "notSupported"));
  const unsupported = items.filter((i) => i.metric?.status === "unavailable" && i.metric.reason.kind === "notSupported");
  return (
    <>
      {shown.length > 0 && (
        <dl className="grid grid-cols-2 gap-x-4 gap-y-3 border-t border-line pt-4 @lg:grid-cols-3">
          {shown.map((i) => (
            <div key={i.label} className={`flex min-w-0 flex-col gap-0.5 ${i.wide ? "col-span-2" : ""}`}>
              <dt className="truncate text-xs text-fg-muted">{i.label}</dt>
              <dd className="flex min-w-0 items-baseline gap-1.5 truncate text-md text-fg tabular">
                {i.value ?? (i.metric && i.format ? <MetricValue metric={i.metric} format={i.format} /> : null)}
                <LevelLabel level={i.level} />
              </dd>
            </div>
          ))}
        </dl>
      )}
      {unsupported.length > 0 && (
        <p className="text-xs text-fg-subtle">{t("notOnThisDevice", { list: unsupported.map((i) => i.label).join(", ") })}</p>
      )}
    </>
  );
}

function WidgetSkeleton() {
  return (
    <WaitingForSample>
      <span aria-hidden="true" className="flex flex-col gap-4">
        <span className="flex items-end gap-5">
          <Skeleton className="h-10 w-24" />
          <Skeleton className="h-16 flex-1" />
        </span>
        <span className="grid grid-cols-3 gap-4">
          <Skeleton className="h-9" />
          <Skeleton className="h-9" />
          <Skeleton className="h-9" />
        </span>
      </span>
    </WaitingForSample>
  );
}

// ---------------------------------------------------------------- CPU

const CpuWidget = memo(function CpuWidget() {
  const tm = useT("monitoring");
  const cpu = useLive((s) => s.latest?.cpu);
  return (
    <Widget icon="cpu" title={tm("cpuUsage")} aside={cpu && <SourceCaption metric={cpu.usagePercent} />}>
      {cpu ? <CpuBody cpu={cpu} /> : <WidgetSkeleton />}
    </Widget>
  );
});

function CpuBody({ cpu }: { cpu: CpuSample }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  return (
    <>
      <Headline
        value={<BigPercent metric={cpu.usagePercent} percent={valueOf(cpu.usagePercent)} caption={t("load")} />}
        trend={<PercentTrend pick={cpuUsage} label="CPU" />}
      />
      <Stats
        items={[
          { label: tm("temperature"), metric: cpu.temperatureC, format: formatCelsius },
          { label: t("clock"), metric: cpu.currentMhz, format: formatMhz },
          { label: tm("power"), metric: cpu.powerW, format: formatWatts },
        ]}
      />
      <FrequencyLimit cpu={cpu} />
    </>
  );
}

/** Shown only when the OS reports a limit below the nominal maximum; the cause is unknown. */
function FrequencyLimit({ cpu }: { cpu: CpuSample }) {
  const t = useT("overview");
  if (!isFrequencyLimited(cpu.limitMhz, cpu.maxMhz)) return null;
  if (cpu.limitMhz.status !== "available" || cpu.maxMhz.status !== "available") return null;
  const limit = formatMhz(cpu.limitMhz.value);
  const body = t("frequencyLimitBody", { limit, max: formatMhz(cpu.maxMhz.value) });
  return (
    <Tooltip content={body} side="top" multiline className="min-w-0">
      <p role="note" tabIndex={0} className="flex min-w-0 cursor-help items-center gap-1.5 rounded-sm text-xs text-fg-muted">
        <Icon name="alert" size={14} className="shrink-0 text-warn" />
        <span className="truncate">{t("frequencyLimited", { limit })}</span>
        <span className="sr-only">{body}</span>
      </p>
    </Tooltip>
  );
}

// ---------------------------------------------------------------- Memory

const MemoryWidget = memo(function MemoryWidget() {
  // Byte labels follow the view mode (lib/format.ts): re-render when it changes.
  useViewMode();
  const t = useT("overview");
  const tm = useT("monitoring");
  const simple = useIsSimple();
  const mac = useIsMac();
  const mem = useLive((s) => s.latest?.memory);
  if (!mem) {
    return (
      <Widget icon="memory" title={tm("memoryInUse")}>
        <WidgetSkeleton />
      </Widget>
    );
  }
  const used = valueOf(mem.usedBytes);
  const total = valueOf(mem.totalBytes);
  const percent = used !== null && total !== null && total > 0 ? (used / total) * 100 : null;
  const level = memoryLevel(valueOf(mem.availableBytes), total);
  const ofTotal = (limit: number | null) => (v: number) => (limit !== null ? t("ofTotal", { used: formatBytes(v), total: formatBytes(limit) }) : formatBytes(v));
  const commitLimit = valueOf(mem.commitLimitBytes);
  const pLevel = pressureLevel(mem.pressure.status === "available" ? mem.pressure.value : null);
  const items: StatItem[] = [
    { label: t("used"), metric: mem.usedBytes, format: ofTotal(total), level },
    { label: tm("available"), metric: mem.availableBytes, format: formatBytes },
  ];
  if (mac) {
    // The kernel's level is the "memory is short" signal on macOS: shown in both views.
    // Above normal, the status word ("Avviso" / "Critico") is the value itself.
    items.push({
      label: t("memoryPressure"),
      value: pLevel === "normal" ? <MetricValue metric={mem.pressure} format={() => t("pressureNormal")} /> : null,
      level: pLevel,
    });
  }
  if (!simple) {
    if (mac) {
      // No bar or limit: macOS grows the swap files on demand.
      items.push({ label: t("swapUsed"), metric: mem.swapUsedBytes, format: ofTotal(valueOf(mem.swapTotalBytes)) });
    } else {
      items.push({
        label: t("committedMemory"),
        metric: mem.commitUsedBytes,
        format: ofTotal(commitLimit),
        level: commitLevel(valueOf(mem.commitUsedBytes), commitLimit),
      });
    }
    items.push({ label: t("pagesFromDisk"), metric: mem.pagesPerSec, format: formatPerSec });
  }
  return (
    <Widget icon="memory" title={tm("memoryInUse")} aside={<SourceCaption metric={mem.usedBytes} />}>
      <Headline
        value={
          <BigPercent
            metric={mem.usedBytes.status === "unavailable" ? mem.usedBytes : mem.totalBytes}
            percent={percent}
            caption={t("inUse")}
            level={level}
          />
        }
        trend={<PercentTrend pick={memoryUsedPercent} label="RAM" />}
      />
      <Stats items={items} />
    </Widget>
  );
});

// ---------------------------------------------------------------- GPU

const GpuWidget = memo(function GpuWidget() {
  useViewMode();
  const t = useT("overview");
  const tm = useT("monitoring");
  const { status, data } = useInventoryData();
  const gpu = useMemo(() => (data ? primaryGpu(data.gpus) : null), [data]);
  return (
    <Widget icon="gpu" title={tm("gpuUsage")} aside={gpu && <span className="truncate">{gpu.name}</span>}>
      {status !== "ready" || !data ? (
        <InventoryState compact placeholder={<WidgetSkeleton />} />
      ) : !gpu ? (
        <StateView
          compact
          kind="empty"
          title={tm("noHardwareGpu")}
          {...(data.gpus.length > 0 ? { description: t("onlySoftwareAdapters") } : {})}
        />
      ) : (
        <GpuBody id={gpu.id} name={gpu.name} dedicatedVramBytes={dedicatedVram(gpu)} unifiedMemory={hasUnifiedMemory(gpu)} />
      )}
    </Widget>
  );
});

function GpuBody({ id, name, dedicatedVramBytes, unifiedMemory }: { id: string; name: string; dedicatedVramBytes: number | null; unifiedMemory: boolean }) {
  const tm = useT("monitoring");
  const hasSample = useLive((s) => s.latest !== null);
  const gpu = useLive((s) => s.latest?.gpus.find((g) => g.id === id));
  if (!hasSample) return <WidgetSkeleton />;
  if (!gpu) return <StateView compact kind="empty" title={tm("noSampleForAdapter")} description={name} />;
  return <GpuReadings id={id} gpu={gpu} dedicatedVramBytes={dedicatedVramBytes} unifiedMemory={unifiedMemory} />;
}

function GpuReadings({ id, gpu, dedicatedVramBytes, unifiedMemory }: { id: string; gpu: GpuSample; dedicatedVramBytes: number | null; unifiedMemory: boolean }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  const th = useT("hardware");
  const pick = useMemo(() => gpuUsage(id), [id]);
  const power = gpuPowerReading(gpu, tm);
  const vramText = (v: number) =>
    dedicatedVramBytes !== null ? t("ofTotal", { used: formatBytes(v), total: formatBytes(dedicatedVramBytes) }) : formatBytes(v);
  const items: StatItem[] = [
    { label: tm("temperature"), metric: gpu.temperatureC, format: formatCelsius },
    { label: tm("coreClock"), metric: gpu.coreClockMhz, format: formatMhz },
    { label: tm("power"), metric: power.metric, format: power.format },
    { label: tm("fan"), metric: gpu.fanRpm, format: formatRpm },
    unifiedMemory && gpu.dedicatedUsedBytes.status === "unavailable"
      ? { label: "VRAM", value: <span className="truncate text-fg-muted">{th("unifiedMemory")}</span>, wide: true }
      : {
          label: "VRAM",
          metric: gpu.dedicatedUsedBytes,
          format: vramText,
          level: vramLevel(valueOf(gpu.dedicatedUsedBytes), dedicatedVramBytes),
        },
  ];
  return (
    <>
      <Headline
        value={<BigPercent metric={gpu.usagePercent} percent={valueOf(gpu.usagePercent)} caption={t("load")} />}
        trend={<PercentTrend pick={pick} label="GPU" />}
      />
      <Stats items={items} />
    </>
  );
}

// ---------------------------------------------------------------- Disk activity

const DiskWidget = memo(function DiskWidget() {
  useViewMode();
  const t = useT("overview");
  const tm = useT("monitoring");
  const storage = useLive((s) => s.latest?.storage);
  const read = useSeries(diskRead);
  const write = useSeries(diskWrite);
  const domain = useMemo(() => throughputDomain([read, write]), [read, write]);
  if (!storage) {
    return (
      <Widget icon="storage" title={t("diskTitle")}>
        <WidgetSkeleton />
      </Widget>
    );
  }
  const r = valueOf(storage.readBytesPerSec);
  const [value, unit] = r !== null ? splitUnit(formatBytesPerSec(r)) : ["", undefined];
  return (
    <Widget icon="storage" title={t("diskTitle")} aside={<SourceCaption metric={storage.readBytesPerSec} />}>
      <Headline
        value={
          <>
            {r !== null ? (
              <Readout size="lg" value={value} {...(unit ? { unit } : {})} />
            ) : (
              <Readout size="lg" value={<MetricValue metric={storage.readBytesPerSec} format={formatBytesPerSec} />} />
            )}
            <span className="text-xs text-fg-muted">{tm("read")}</span>
          </>
        }
        trend={<Trend points={read} domain={domain} label={tm("throughputAria")} />}
      />
      <Stats
        items={[
          { label: tm("write"), metric: storage.writeBytesPerSec, format: formatBytesPerSec },
          { label: t("activeTime"), metric: storage.activeTimePercent, format: (v) => formatPercent(v) },
        ]}
      />
    </Widget>
  );
});

// ---------------------------------------------------------------- Storage (inventory, not live)

const StorageWidget = memo(function StorageWidget() {
  useViewMode();
  const t = useT("overview");
  const tm = useT("monitoring");
  const platform = usePlatform();
  const { status, data } = useInventoryData();
  return (
    <Widget
      icon="storage"
      title={t("storageTitle")}
      aside={data && <span className="tabular">{t("detectedAt", { time: formatTime(data.collectedAt) })}</span>}
    >
      {status !== "ready" || !data ? (
        <InventoryState compact placeholder={<Skeleton className="h-12" />} />
      ) : (
        <div className="grid grid-cols-1 gap-x-8 gap-y-5 @3xl:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
          {data.volumes.length === 0 ? (
            <StateView compact kind="empty" title={tm("noVolumes")} />
          ) : (
            <ul className="grid grid-cols-1 gap-4 @lg:grid-cols-2">
              {data.volumes.map((v) => (
                <VolumeItem key={v.mountPoint} volume={v} />
              ))}
            </ul>
          )}
          {showsDiskStatus(data.physicalDisks, platform) && (
            <div className="flex min-w-0 flex-col gap-2 @3xl:border-l @3xl:border-line @3xl:pl-8">
              <div className="flex items-center gap-1.5">
                <h4 className="eyebrow">{tm("windowsHealth")}</h4>
                <Tooltip content={t("healthNote")} side="top">
                  <span tabIndex={0} aria-label={t("healthNote")} className="inline-flex rounded text-fg-subtle">
                    <Icon name="info" size={12} />
                  </span>
                </Tooltip>
              </div>
              <DiskHealthList disks={data.physicalDisks} />
            </div>
          )}
        </div>
      )}
    </Widget>
  );
});

function VolumeItem({ volume: v }: { volume: VolumeInfo }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  const used = v.totalBytes - v.availableBytes;
  const percent = v.totalBytes > 0 ? (used / v.totalBytes) * 100 : null;
  const level = volumeLevel(v.availableBytes, v.totalBytes);
  const tone = toneOf(level);
  const name = volumeDisplayName(v);
  return (
    <li className="flex min-w-0 flex-col gap-1.5">
      <div className="flex min-w-0 items-baseline justify-between gap-3">
        <span className="min-w-0 truncate text-sm text-fg" title={v.label || v.mountPoint}>
          {name}
        </span>
        <span className="shrink-0 text-xs text-fg-muted tabular">
          {t("volumeFree", { free: formatStorageBytes(v.availableBytes), total: formatStorageBytes(v.totalBytes) })}
        </span>
      </div>
      <div
        role="meter"
        aria-label={tm("usedSpaceOn", { mount: v.mountPoint })}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent ?? 0}
        aria-valuetext={percent !== null ? tm("percentUsed", { percent: formatPercent(percent) }) : tm("notAvailableText")}
        className="h-1.5 overflow-hidden rounded-full bg-control-track"
      >
        <div
          className={`h-full rounded-full ${tone === "bad" ? "bg-bad" : tone === "warn" ? "bg-warn" : "bg-meter"}`}
          style={{ width: `${Math.max(percent ?? 0, 1.5)}%` }}
        />
      </div>
      {tone && <LevelLabel level={level} />}
    </li>
  );
}

function DiskHealthList({ disks }: { disks: Metric<PhysicalDiskInfo[]> }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  const tc = useT("common");
  if (disks.status === "unavailable") {
    return (
      <p className="text-sm text-fg-subtle">
        {tc("notAvailable")} — <UnavailableReason reason={disks.reason} />
      </p>
    );
  }
  if (disks.value.length === 0) return <p className="text-sm text-fg-subtle">{t("noPhysicalDisks")}</p>;
  return (
    <ul className="flex flex-col gap-2">
      {disks.value.map((d) => {
        const name = d.model ?? tm("diskFallback", { id: d.id });
        return (
          <li key={d.id} className="flex min-w-0 items-center justify-between gap-2 text-sm">
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
