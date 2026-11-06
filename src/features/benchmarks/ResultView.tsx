import { memo, type ReactNode, useMemo } from "react";
import { LineChart } from "@/components/charts/LineChart";
import { DefinitionList, type DefinitionItem } from "@/components/common/DefinitionList";
import { Icon } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useT } from "@/i18n";
import {
  formatBytes,
  formatDateTime,
  formatElapsed,
  formatMhz,
  formatPercent,
  formatUnitValue,
  niceCeil,
} from "@/lib/format";
import type { BenchmarkContext } from "@/types/generated/BenchmarkContext";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";
import type { BenchmarkSeries } from "@/types/generated/BenchmarkSeries";
import type { ResultMetric } from "@/types/generated/ResultMetric";
import {
  isDebugBuild,
  isStorageKind,
  powerSourceLabel,
  stateLabel,
  stateTone,
} from "./model";
import { INSET } from "./ui";

/**
 * Full view of one stored or just-finished result. Shared by Benchmarks and History. The test
 * name is the surrounding card's (Benchmarks: the test panel above; History: the detail
 * title), so it is not repeated here. `meta` (state and date) is off when the container
 * already shows them.
 */
export function ResultView({
  result,
  comparison,
  meta = true,
}: {
  result: BenchmarkResult;
  comparison?: ReactNode;
  meta?: boolean;
}) {
  const t = useT("benchmarks");
  return (
    <div className="flex flex-col gap-4">
      {meta && <ResultMeta result={result} />}
      <SafetyNotes result={result} />
      <RunNotes notes={result.notes} />

      {result.state === "completed" && (
        <MetricsGrid metrics={result.metrics} />
      )}
      {result.state === "cancelled" && (
        <StateView kind="empty" title={t("cancelledTitle")} description={t("cancelledDesc")} />
      )}
      {result.state === "failed" && (
        <StateView
          kind="error"
          title={t("failedTitle")}
          description={t("failedDesc")}
          {...(result.error ? { error: result.error } : {})}
        />
      )}

      {result.state === "completed" && result.series.some((s) => s.points.length > 1) && (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          {result.series
            .filter((s) => s.points.length > 1)
            .map((s) => (
              <SeriesChart key={s.id} series={s} />
            ))}
        </div>
      )}

      {result.state === "completed" && comparison}

      <div className="grid grid-cols-1 gap-x-8 gap-y-5 border-t border-line pt-4 lg:grid-cols-2">
        <section>
          <h3 className="mb-2 text-sm font-semibold text-fg">{t("contextTitle")}</h3>
          <ContextList context={result.context} />
        </section>
        <section>
          <h3 className="mb-2 text-sm font-semibold text-fg">{t("paramsTitle")}</h3>
          <RunParams result={result} />
        </section>
      </div>
    </div>
  );
}

/** State and start time. Duration, methodology and build are in the parameters list. */
function ResultMeta({ result }: { result: BenchmarkResult }) {
  const t = useT("benchmarks");
  return (
    <p className="flex flex-wrap items-center gap-2 text-xs text-fg-muted tabular">
      <StatusBadge tone={stateTone(result.state)} label={stateLabel(result.state, t)} />
      {formatDateTime(result.startedAt)}
    </p>
  );
}

/** Safety stop and warnings, prominent (amber). Shown for every final state. */
function SafetyNotes({ result }: { result: BenchmarkResult }) {
  const t = useT("benchmarks");
  const debug = isDebugBuild(result);
  if (!result.safetyStop && result.warnings.length === 0 && !debug) return null;
  return (
    <div role="note" className="rounded-md border border-warn/25 bg-warn-soft px-3 py-2.5 text-[13px] text-fg">
      {result.safetyStop && (
        <p className="flex items-start gap-2 font-medium">
          <Icon name="alert" className="mt-0.5 shrink-0 text-warn" />
          <span>{t("safetyStop", { reason: result.safetyStop })}</span>
        </p>
      )}
      {(result.warnings.length > 0 || debug) && (
        <ul className={`flex flex-col gap-1 ${result.safetyStop ? "mt-2" : ""}`}>
          {debug && (
            <li className="flex items-start gap-2">
              <Icon name="alert" className="mt-0.5 shrink-0 text-warn" />
              <span>{t("debugWarning")}</span>
            </li>
          )}
          {result.warnings.map((w) => (
            <li key={w} className="flex items-start gap-2">
              <Icon name="alert" className="mt-0.5 shrink-0 text-warn" />
              <span>{w}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Neutral facts about how the run was performed (e.g. the adapter used): no warning style. */
function RunNotes({ notes }: { notes: readonly string[] }) {
  const t = useT("benchmarks");
  if (notes.length === 0) return null;
  return (
    <ul aria-label={t("runNotes")} className="flex flex-col gap-0.5 text-[13px] text-fg-muted">
      {notes.map((n) => (
        <li key={n}>{n}</li>
      ))}
    </ul>
  );
}

/**
 * Metric tiles. In a final result the first metric is the headline (the backend orders
 * them) and is set larger; interim values are all equal weight. Values update in place.
 */
export function MetricsGrid({ metrics, interim = false }: { metrics: readonly ResultMetric[]; interim?: boolean }) {
  const t = useT("benchmarks");
  if (metrics.length === 0) {
    return interim ? (
      <p className="text-[13px] text-fg-subtle">{t("interimPending")}</p>
    ) : (
      <StateView compact kind="empty" title={t("noMetrics")} />
    );
  }
  return (
    <ul className="grid grid-cols-[repeat(auto-fit,minmax(11rem,1fr))] gap-3">
      {metrics.map((m, i) => {
        const headline = i === 0 && !interim;
        return (
          <li key={m.id} className={`${INSET} flex flex-col px-3 py-2.5`}>
            <span className="text-xs text-fg-muted">{m.label}</span>
            <span
              className={`mt-1 font-display tabular text-fg ${
                headline ? "text-[28px] leading-9 font-light tracking-[-0.02em]" : "text-lg leading-7 font-light"
              }`}
            >
              {formatUnitValue(m.value, m.unit)}
            </span>
            <span className="mt-auto pt-1 text-2xs text-fg-subtle">
              {m.higherIsBetter ? t("higherIsBetter") : t("lowerIsBetter")}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

const SeriesChart = memo(function SeriesChart({ series }: { series: BenchmarkSeries }) {
  const t = useT("benchmarks");
  const { points, xDomain, yDomain } = useMemo(() => {
    const pts = series.points.map((p) => ({ t: p.elapsedMs, v: Number.isFinite(p.value) ? p.value : null }));
    const maxT = Math.max(1, ...series.points.map((p) => p.elapsedMs));
    const maxV = Math.max(0, ...series.points.map((p) => (Number.isFinite(p.value) ? p.value : 0)));
    // Zero baseline: never zoom into small variations.
    return { points: pts, xDomain: [0, maxT] as const, yDomain: [0, niceCeil(maxV * 1.05) || 1] as const };
  }, [series]);
  const chartSeries = useMemo(
    () => [{ id: series.id, label: series.label, points, tone: "primary" as const }],
    [series.id, series.label, points],
  );
  const formatY = useMemo(() => (v: number) => formatUnitValue(v, series.unit), [series.unit]);

  return (
    <figure className={`${INSET} min-w-0 p-3`}>
      <figcaption className="mb-2 text-xs font-semibold text-fg-muted">{series.label}</figcaption>
      <LineChart
        series={chartSeries}
        xDomain={xDomain}
        yDomain={yDomain}
        formatY={formatY}
        formatX={formatElapsed}
        height={140}
        ariaLabel={t("seriesAria", { label: series.label })}
      />
    </figure>
  );
});

function ContextList({ context }: { context: BenchmarkContext }) {
  const t = useT("benchmarks");
  const items: DefinitionItem[] = [
    { label: t("powerSource"), value: powerSourceLabel(context.powerSource, t) },
    { label: t("powerPlan"), value: context.powerPlan ?? t("notReported") },
    {
      label: t("backgroundCpu"),
      value: <MetricValue metric={context.backgroundCpuPercent} format={(v) => formatPercent(v, 1)} />,
    },
    {
      label: t("frequencyLimit"),
      value: t.rich("limitOfMax", {
        limit: <MetricValue metric={context.cpuLimitMhz} format={formatMhz} />,
        max: <MetricValue metric={context.cpuMaxMhz} format={formatMhz} />,
      }),
    },
    { label: t("availableMemory"), value: <MetricValue metric={context.availableMemoryBytes} format={formatBytes} /> },
    { label: "CPU", value: context.cpuBrand ?? t("notReportedFem") },
    { label: t("logicalProcessors"), value: context.logicalProcessors },
    { label: t("totalMemory"), value: formatBytes(context.totalMemoryBytes) },
    { label: t("system"), value: context.osVersion ?? t("notReported") },
  ];
  if (context.storageModel) items.push({ label: t("disk"), value: context.storageModel });
  if (context.gpuName) items.push({ label: "GPU", value: context.gpuName });
  if (context.gpuDriverVersion) items.push({ label: t("gpuDriver"), value: context.gpuDriverVersion });
  return <DefinitionList items={items} />;
}

function RunParams({ result }: { result: BenchmarkResult }) {
  const t = useT("benchmarks");
  const r = result.request;
  const items: DefinitionItem[] = [];
  if (r.durationS !== null) items.push({ label: t("requestedDuration"), value: `${r.durationS} s` });
  if (r.threads !== null) items.push({ label: t("threads"), value: r.threads });
  if (isStorageKind(result.kind)) {
    items.push({ label: t("volume"), value: r.storageVolume ?? "—" });
    if (r.storageFileMib !== null) items.push({ label: t("tempFile"), value: `${r.storageFileMib} MiB` });
  }
  items.push(
    { label: t("actualDuration"), value: formatElapsed(result.durationMs) },
    {
      label: t("methodology"),
      value: <span title={t("comparableNote")}>v{result.methodologyVersion}</span>,
    },
    {
      label: t("build"),
      // A debug build is flagged once, in the warnings at the top of the result.
      value: result.buildProfile,
    },
    { label: t("appVersion"), value: result.appVersion },
  );
  return <DefinitionList items={items} />;
}
