import { memo, useMemo } from "react";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { MetricValue, UnavailableReason, useUnavailabilityText } from "@/components/common/MetricValue";
import { Readout } from "@/components/common/Readout";
import { RingGauge } from "@/components/common/RingGauge";
import { Skeleton } from "@/components/common/Skeleton";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { Tooltip } from "@/components/common/Tooltip";
import { useIsSimple, useViewMode } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { formatBytes, formatBytesPerSec, formatCelsius, formatMhz, formatPercent, formatPerSec, formatRpm, formatTime, formatWatts } from "@/lib/format";
import { DiskHealth } from "@/features/monitoring/DiskHealth";
import { gpuPowerReading } from "@/features/monitoring/gpuPower";
import { useInventoryData } from "@/features/monitoring/hooks";
import { useLive } from "@/features/monitoring/liveStore";
import { isFrequencyLimited, primaryGpu } from "@/features/monitoring/selection";
import { valueOf } from "@/features/monitoring/series";
import { InventoryState, WaitingForSample } from "@/features/monitoring/StatusViews";
import type { CpuSample } from "@/types/generated/CpuSample";
import type { GpuSample } from "@/types/generated/GpuSample";
import type { Metric } from "@/types/generated/Metric";
import type { PhysicalDiskInfo } from "@/types/generated/PhysicalDiskInfo";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";
import { commitLevel, type Level, memoryLevel, volumeLevel, vramLevel } from "./levels";
import { type Bar, BarRow, GaugeLayout, GaugeSkeleton, metricText, splitUnit, toneOf, useLevelText } from "./parts";

/** Panels are flex columns so a trailing line can sit at the bottom when a row stretches them. */
const PANEL = "flex flex-col";

/**
 * Display scale of the temperature bars, 0–100 °C (the throttling point of most CPUs and GPUs
 * is close to 100 °C). The value itself is always written out next to the bar.
 */
const TEMP_SCALE_C = 100;

const pct = (v: number) => formatPercent(v);
const bytes = (v: number) => formatBytes(v);
const rate = (v: number) => formatBytesPerSec(v);

function ratio(value: number | null, total: number | null): number | null {
  return value !== null && total !== null && total > 0 ? (value / total) * 100 : null;
}

/** Headline gauge for a 0–100 metric: "n/d" with the reason (tooltip and spoken) when unavailable. */
function PercentGauge({
  metric,
  percent,
  label,
  caption,
  level,
  size = 140,
}: {
  /** Drives availability and the reason. */
  metric: Metric<number>;
  /** The fill, when it is derived (e.g. used / total). Defaults to the metric's value. */
  percent?: number | null;
  label: string;
  caption: string;
  /** Status of the value (`levels.ts`): colours the arc. State it as text nearby too. */
  level?: Level | undefined;
  size?: 140 | 112;
}) {
  const tc = useT("common");
  const reasonText = useUnavailabilityText();
  const p = percent !== undefined ? percent : valueOf(metric);
  const valueText = useLevelText(p === null ? "" : formatPercent(p), level);
  if (metric.status === "unavailable" || p === null) {
    const reason = metric.status === "unavailable" ? reasonText(metric.reason) : undefined;
    return (
      <Tooltip content={reason} side="bottom" multiline enabled={reason !== undefined}>
        <span tabIndex={reason !== undefined ? 0 : undefined} className="inline-flex shrink-0 rounded-full">
          <RingGauge
            percent={null}
            value={tc("notAvailable")}
            caption={caption}
            label={label}
            valueText={reason ? `${tc("notAvailable")} — ${reason}` : tc("notAvailable")}
            size={size}
          />
        </span>
      </Tooltip>
    );
  }
  return (
    <RingGauge
      percent={p}
      value={Math.round(p).toString()}
      unit="%"
      caption={caption}
      label={label}
      valueText={valueText}
      tone={toneOf(level)}
      size={size}
    />
  );
}

/** Temperature bar on the fixed display scale. */
function useTemperatureBar(metric: Metric<number>, component: string): Bar {
  const t = useT("overview");
  const tm = useT("monitoring");
  const v = valueOf(metric);
  return {
    percent: v === null ? null : (v / TEMP_SCALE_C) * 100,
    label: t("meterAria", { label: tm("temperature"), component }),
    valueText: metricText(metric, formatCelsius, tm),
  };
}

// ---------------------------------------------------------------- CPU

export const CpuPanel = memo(function CpuPanel() {
  const cpu = useLive((s) => s.latest?.cpu);
  return (
    <Card title="CPU" className={PANEL} aside={cpu && <SourceCaption metric={cpu.usagePercent} />}>
      {cpu ? (
        <CpuBody cpu={cpu} />
      ) : (
        <WaitingForSample>
          <GaugeSkeleton rows={3} />
        </WaitingForSample>
      )}
    </Card>
  );
});

function CpuBody({ cpu }: { cpu: CpuSample }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  const temp = useTemperatureBar(cpu.temperatureC, "CPU");
  const max = valueOf(cpu.maxMhz);
  const clockBar: Bar | undefined =
    max !== null
      ? {
          percent: ratio(valueOf(cpu.currentMhz), max),
          label: t("meterAria", { label: t("clock"), component: "CPU" }),
          valueText: metricText(cpu.currentMhz, formatMhz, tm),
        }
      : undefined;
  return (
    <GaugeLayout gauge={<PercentGauge metric={cpu.usagePercent} label={tm("cpuUsage")} caption={t("load")} />}>
      <BarRow label={tm("temperature")} bar={temp} value={<MetricValue metric={cpu.temperatureC} format={formatCelsius} />} />
      <BarRow label={t("clock")} bar={clockBar} value={<MetricValue metric={cpu.currentMhz} format={formatMhz} />} />
      <BarRow label={tm("power")} value={<MetricValue metric={cpu.powerW} format={formatWatts} />} />
      <FrequencyLimit cpu={cpu} />
    </GaugeLayout>
  );
}

/** Shown only when Windows reports a limit below the nominal maximum; the cause is unknown. */
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

// ---------------------------------------------------------------- GPU

export const GpuPanel = memo(function GpuPanel() {
  // Byte labels follow the view mode (lib/format.ts): re-render when it changes.
  useViewMode();
  const t = useT("overview");
  const tm = useT("monitoring");
  const { status, data } = useInventoryData();
  const gpu = useMemo(() => (data ? primaryGpu(data.gpus) : null), [data]);
  return (
    <Card title="GPU" className={PANEL}>
      {status !== "ready" || !data ? (
        <InventoryState compact placeholder={<GaugeSkeleton rows={5} />} />
      ) : !gpu ? (
        <StateView
          compact
          kind="empty"
          title={tm("noHardwareGpu")}
          {...(data.gpus.length > 0 ? { description: t("onlySoftwareAdapters") } : {})}
        />
      ) : (
        <GpuBody id={gpu.id} name={gpu.name} dedicatedVramBytes={gpu.dedicatedVramBytes} />
      )}
    </Card>
  );
});

function GpuBody({ id, name, dedicatedVramBytes }: { id: string; name: string; dedicatedVramBytes: number }) {
  const tm = useT("monitoring");
  const hasSample = useLive((s) => s.latest !== null);
  const gpu = useLive((s) => s.latest?.gpus.find((g) => g.id === id));
  if (!hasSample) {
    return (
      <WaitingForSample>
        <GaugeSkeleton rows={5} />
      </WaitingForSample>
    );
  }
  if (!gpu) return <StateView compact kind="empty" title={tm("noSampleForAdapter")} description={name} />;
  return (
    <>
      <GpuReadings gpu={gpu} dedicatedVramBytes={dedicatedVramBytes} />
      {/* Which adapter (DXGI primary); pinned to the bottom edge when the row stretches the panel. */}
      <p className="mt-auto truncate pt-3 text-xs text-fg-subtle" title={name}>
        {name}
      </p>
    </>
  );
}

function GpuReadings({ gpu, dedicatedVramBytes }: { gpu: GpuSample; dedicatedVramBytes: number }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  const temp = useTemperatureBar(gpu.temperatureC, "GPU");
  const power = gpuPowerReading(gpu, tm);
  const powerBar: Bar | undefined =
    power.metric === gpu.powerPercent && gpu.powerPercent.status === "available"
      ? {
          percent: valueOf(gpu.powerPercent),
          label: t("meterAria", { label: tm("power"), component: "GPU" }),
          valueText: metricText(power.metric, power.format, tm),
        }
      : undefined;
  const vramText = (v: number) =>
    dedicatedVramBytes > 0 ? t("ofTotal", { used: formatBytes(v), total: formatBytes(dedicatedVramBytes) }) : formatBytes(v);
  const vramBar: Bar | undefined =
    dedicatedVramBytes > 0
      ? {
          percent: ratio(valueOf(gpu.dedicatedUsedBytes), dedicatedVramBytes),
          label: t("meterAria", { label: "VRAM", component: "GPU" }),
          valueText: metricText(gpu.dedicatedUsedBytes, vramText, tm),
          level: vramLevel(valueOf(gpu.dedicatedUsedBytes), dedicatedVramBytes),
        }
      : undefined;

  return (
    <GaugeLayout gauge={<PercentGauge metric={gpu.usagePercent} label={tm("gpuUsage")} caption={t("load")} />}>
      <BarRow label={tm("temperature")} bar={temp} value={<MetricValue metric={gpu.temperatureC} format={formatCelsius} />} />
      <BarRow label={tm("coreClock")} value={<MetricValue metric={gpu.coreClockMhz} format={formatMhz} />} />
      <BarRow label={tm("power")} bar={powerBar} value={<MetricValue metric={power.metric} format={power.format} />} />
      <BarRow label={tm("fan")} value={<MetricValue metric={gpu.fanRpm} format={formatRpm} />} />
      <BarRow label="VRAM" bar={vramBar} value={<MetricValue metric={gpu.dedicatedUsedBytes} format={vramText} />} />
    </GaugeLayout>
  );
}

// ---------------------------------------------------------------- RAM

export const RamPanel = memo(function RamPanel() {
  // Byte labels follow the view mode (lib/format.ts): re-render when it changes.
  useViewMode();
  const t = useT("overview");
  const tm = useT("monitoring");
  const simple = useIsSimple();
  const mem = useLive((s) => s.latest?.memory);
  if (!mem) {
    return (
      <Card title="RAM" className={PANEL}>
        <WaitingForSample>
          <GaugeSkeleton rows={2} size={112} />
        </WaitingForSample>
      </Card>
    );
  }
  const total = valueOf(mem.totalBytes);
  const usedText = (v: number) => (total !== null ? t("ofTotal", { used: formatBytes(v), total: formatBytes(total) }) : formatBytes(v));
  const commitLimit = valueOf(mem.commitLimitBytes);
  const memLevel = memoryLevel(valueOf(mem.availableBytes), total);
  const commitText = (v: number) =>
    commitLimit !== null ? t("ofTotal", { used: formatBytes(v), total: formatBytes(commitLimit) }) : formatBytes(v);
  return (
    <Card title="RAM" className={PANEL} aside={<SourceCaption metric={mem.usedBytes} />}>
      <GaugeLayout
        gauge={
          <PercentGauge
            metric={mem.usedBytes}
            percent={ratio(valueOf(mem.usedBytes), total)}
            label={tm("memoryInUse")}
            caption={t("inUse")}
            level={memLevel}
            size={112}
          />
        }
      >
        <BarRow label={t("used")} level={memLevel} value={<MetricValue metric={mem.usedBytes} format={usedText} />} />
        <BarRow label={tm("available")} value={<MetricValue metric={mem.availableBytes} format={bytes} />} />
        {!simple && (
          <>
            <BarRow
              label={t("committedMemory")}
              bar={{
                percent: ratio(valueOf(mem.commitUsedBytes), commitLimit),
                label: t("committedMemory"),
                valueText: metricText(mem.commitUsedBytes, commitText, tm),
                level: commitLevel(valueOf(mem.commitUsedBytes), commitLimit),
              }}
              value={<MetricValue metric={mem.commitUsedBytes} format={commitText} />}
            />
            <BarRow label={t("pagesFromDisk")} value={<MetricValue metric={mem.pagesPerSec} format={formatPerSec} />} />
          </>
        )}
      </GaugeLayout>
    </Card>
  );
});

// ---------------------------------------------------------------- Disk activity

export const DiskPanel = memo(function DiskPanel() {
  // Byte labels follow the view mode (lib/format.ts): re-render when it changes.
  useViewMode();
  const t = useT("overview");
  const tm = useT("monitoring");
  const storage = useLive((s) => s.latest?.storage);
  return (
    <Card title={t("diskTitle")} className={PANEL} aside={storage && <SourceCaption metric={storage.readBytesPerSec} />}>
      {!storage ? (
        <WaitingForSample>
          <span aria-hidden="true" className="grid grid-cols-2 gap-4">
            <Skeleton className="h-14" />
            <Skeleton className="h-14" />
          </span>
        </WaitingForSample>
      ) : (
        <>
          <div className="grid grid-cols-2 gap-4">
            <RateReadout label={tm("read")} metric={storage.readBytesPerSec} />
            <RateReadout label={tm("write")} metric={storage.writeBytesPerSec} />
          </div>
          <div className="mt-4">
            <BarRow
              label={t("activeTime")}
              bar={{
                percent: valueOf(storage.activeTimePercent),
                label: t("meterAria", { label: t("activeTime"), component: t("diskTitle") }),
                valueText: metricText(storage.activeTimePercent, pct, tm),
              }}
              value={<MetricValue metric={storage.activeTimePercent} format={pct} />}
            />
          </div>
        </>
      )}
    </Card>
  );
});

function RateReadout({ label, metric }: { label: string; metric: Metric<number> }) {
  const v = valueOf(metric);
  const [value, unit] = v !== null ? splitUnit(rate(v)) : ["", undefined];
  return (
    <div className="min-w-0">
      <p className="text-xs text-fg-muted">{label}</p>
      {v !== null ? (
        <Readout size="md" value={value} {...(unit ? { unit } : {})} />
      ) : (
        <Readout size="md" value={<MetricValue metric={metric} format={rate} />} />
      )}
    </div>
  );
}

// ---------------------------------------------------------------- Storage (inventory)

export const StoragePanel = memo(function StoragePanel() {
  // Byte labels follow the view mode (lib/format.ts): re-render when it changes.
  useViewMode();
  const t = useT("overview");
  const tm = useT("monitoring");
  const { status, data } = useInventoryData();
  if (status !== "ready" || !data) {
    return (
      <Card title={t("storageTitle")} className={PANEL}>
        <InventoryState compact placeholder={<GaugeSkeleton rows={2} size={112} />} />
      </Card>
    );
  }
  return (
    <Card
      title={t("storageTitle")}
      className={PANEL}
      aside={<span className="text-2xs text-fg-subtle tabular">{t("detectedAt", { time: formatTime(data.collectedAt) })}</span>}
    >
      {data.volumes.length === 0 ? (
        <StateView compact kind="empty" title={tm("noVolumes")} />
      ) : (
        <div className="flex flex-col gap-3">
          {data.volumes.map((v) => (
            <VolumeRow key={v.mountPoint} volume={v} />
          ))}
        </div>
      )}
      <div className="mt-4 mb-2 flex items-center gap-1.5">
        <h3 className="eyebrow">{tm("windowsHealth")}</h3>
        <Tooltip content={t("healthNote")} side="top">
          <span tabIndex={0} aria-label={t("healthNote")} className="inline-flex rounded text-fg-subtle">
            <Icon name="info" size={12} />
          </span>
        </Tooltip>
      </div>
      <DiskHealthList disks={data.physicalDisks} />
    </Card>
  );
});

function VolumeRow({ volume: v }: { volume: VolumeInfo }) {
  const t = useT("overview");
  const tm = useT("monitoring");
  const used = v.totalBytes - v.availableBytes;
  const percent = ratio(used, v.totalBytes);
  return (
    <BarRow
      label={v.label ? `${v.mountPoint} ${v.label}` : v.mountPoint}
      title={v.label || undefined}
      bar={{
        percent,
        label: tm("usedSpaceOn", { mount: v.mountPoint }),
        valueText: percent !== null ? tm("percentUsed", { percent: formatPercent(percent) }) : tm("notAvailableText"),
        level: volumeLevel(v.availableBytes, v.totalBytes),
      }}
      value={t("ofTotal", { used: formatBytes(used), total: formatBytes(v.totalBytes) })}
    />
  );
}

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
