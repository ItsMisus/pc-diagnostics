import { memo, useMemo } from "react";
import { LineChart, type Series } from "@/components/charts/LineChart";
import { Card } from "@/components/common/Card";
import { Skeleton } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { useT } from "@/i18n";
import { formatBytesPerSec } from "@/lib/format";
import { useInventoryData, useSeries, useTimeDomain } from "./hooks";
import { useLive } from "./liveStore";
import { primaryGpu } from "./selection";
import { cpuUsage, diskRead, diskWrite, gpuUsage, memoryUsedPercent, PERCENT_DOMAIN, type Pick, throughputDomain } from "./series";
import { InventoryState, WaitingForSample } from "./StatusViews";

/** Keep in sync with `ChartSkeleton` (`h-30` = 120 px), so the first sample causes no shift. */
const CHART_HEIGHT = 120;
const axisPercent = (v: number) => `${v.toFixed(0)}%`;
const axisThroughput = (v: number) => formatBytesPerSec(v, 0);

/** Plot-sized placeholder for a chart whose first sample (or inventory) is pending. */
function ChartSkeleton() {
  return <Skeleton className="h-30 w-full rounded-lg" />;
}

interface PercentChartProps {
  title: string;
  /** Series label in the tooltip and the accessible name. */
  label: string;
  pick: Pick;
}

/**
 * One percent metric over time, fixed 0–100 domain. Re-renders once per accepted sample.
 * No current value in the header: it lives in the dashboard panels above (rule R1).
 */
const PercentChartCard = memo(function PercentChartCard({ title, label, pick }: PercentChartProps) {
  const t = useT("monitoring");
  const hasSamples = useLive((s) => s.samples.length > 0);
  const points = useSeries(pick);
  const xDomain = useTimeDomain();
  const series = useMemo<Series[]>(() => [{ id: "value", label, points }], [label, points]);

  return (
    <Card title={title}>
      {hasSamples ? (
        <LineChart
          series={series}
          xDomain={xDomain}
          yDomain={PERCENT_DOMAIN}
          formatY={axisPercent}
          height={CHART_HEIGHT}
          ariaLabel={t("percentChartAria", { label })}
        />
      ) : (
        <WaitingForSample>
          <ChartSkeleton />
        </WaitingForSample>
      )}
    </Card>
  );
});

export function CpuChart() {
  const t = useT("monitoring");
  return <PercentChartCard title={t("cpuUsage")} label="CPU" pick={cpuUsage} />;
}

export function MemoryChart() {
  const t = useT("monitoring");
  return <PercentChartCard title={t("memoryInUse")} label="RAM" pick={memoryUsedPercent} />;
}

export const GpuChart = memo(function GpuChart() {
  const t = useT("monitoring");
  const { status, data } = useInventoryData();
  const gpu = useMemo(() => (data ? primaryGpu(data.gpus) : null), [data]);
  const pick = useMemo(() => (gpu ? gpuUsage(gpu.id) : null), [gpu]);

  if (status !== "ready" || !data) {
    return (
      <Card title={t("gpuUsage")}>
        <InventoryState compact placeholder={<ChartSkeleton />} />
      </Card>
    );
  }
  if (!gpu || !pick) {
    return (
      <Card title={t("gpuUsage")}>
        <StateView compact kind="empty" title={t("noHardwareGpu")} />
      </Card>
    );
  }
  return <PercentChartCard title={t("gpuUsage")} label="GPU" pick={pick} />;
});

export const ThroughputChart = memo(function ThroughputChart() {
  const t = useT("monitoring");
  const hasSamples = useLive((s) => s.samples.length > 0);
  const read = useSeries(diskRead);
  const write = useSeries(diskWrite);
  const xDomain = useTimeDomain();
  const yDomain = useMemo(() => throughputDomain([read, write]), [read, write]);
  const series = useMemo<Series[]>(
    () => [
      { id: "read", label: t("read"), points: read, tone: "primary" },
      { id: "write", label: t("write"), points: write, tone: "secondary" },
    ],
    [read, write, t],
  );

  return (
    <Card title={t("diskThroughputTitle")}>
      {hasSamples ? (
        <>
          <LineChart
            series={series}
            xDomain={xDomain}
            yDomain={yDomain}
            formatY={axisThroughput}
            height={CHART_HEIGHT}
            ariaLabel={t("throughputAria")}
          />
          <ul className="mt-2 flex flex-wrap gap-x-5 gap-y-1 text-xs text-fg-muted" aria-label={t("legend")}>
            <LegendItem tone="primary" label={t("read")} />
            <LegendItem tone="secondary" label={t("write")} />
          </ul>
        </>
      ) : (
        <WaitingForSample>
          <ChartSkeleton />
          <span className="mt-2 flex h-4 items-center gap-5">
            <Skeleton className="h-2.5 w-24" />
            <Skeleton className="h-2.5 w-24" />
          </span>
        </WaitingForSample>
      )}
    </Card>
  );
});

function LegendItem({ tone, label }: { tone: "primary" | "secondary"; label: string }) {
  return (
    <li className="flex items-center gap-1.5">
      <LegendSwatch tone={tone} />
      <span>{label}</span>
    </li>
  );
}

/**
 * The series' stroke as drawn by `LineChart`: primary is a solid `fg-muted` line, secondary a
 * `meter` dashed one (`3 2`), same widths and caps, so the key reads as the chart does.
 */
function LegendSwatch({ tone }: { tone: "primary" | "secondary" }) {
  const primary = tone === "primary";
  return (
    <svg width={16} height={8} viewBox="0 0 16 8" aria-hidden="true" focusable="false" className="shrink-0">
      <line
        x1={1}
        x2={15}
        y1={4}
        y2={4}
        className={primary ? "stroke-fg-muted" : "stroke-meter"}
        strokeWidth={primary ? 1.5 : 1.25}
        strokeDasharray={primary ? undefined : "3 2"}
        strokeLinecap="round"
      />
    </svg>
  );
}
