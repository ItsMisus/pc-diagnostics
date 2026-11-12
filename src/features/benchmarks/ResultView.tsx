import { useViewMode } from "@/hooks/useViewMode";
import { memo, type ReactNode, useId, useMemo } from "react";
import { LineChart } from "@/components/charts/LineChart";
import { DefinitionList, type DefinitionItem } from "@/components/common/DefinitionList";
import { Icon } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { Readout, Stat } from "@/components/common/Readout";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useT } from "@/i18n";
import {
  formatBytes,
  formatMib,
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
import { useIsMac } from "@/lib/platform";
import {
  isDebugBuild,
  isStorageKind,
  powerSourceLabel,
  splitUnitValue,
  stateLabel,
  stateTone,
} from "./model";
import { INSET } from "./ui";

/**
 * Full view of one stored or just-finished result. Shared by Benchmarks and History. The test
 * name is the surrounding panel's (Benchmarks: the result panel; History: the detail title),
 * so it is not repeated here. `meta` (state and date) is off when the container already
 * shows them.
 *
 * Order: state, warnings, the headline value (the one hero number), the other measures, the
 * run charts, the comparison, then the conditions and parameters that give it context.
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
  const charted = result.state === "completed" ? result.series.filter((s) => s.points.length > 1) : [];
  return (
    <div className="flex flex-col gap-6">
      {(meta || result.safetyStop || result.warnings.length > 0 || isDebugBuild(result) || result.notes.length > 0) && (
        <div className="flex flex-col gap-3">
          {meta && <ResultMeta result={result} />}
          <SafetyNotes result={result} />
          <RunNotes notes={result.notes} />
        </div>
      )}

      {result.state === "completed" && <ResultMetrics metrics={result.metrics} />}
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

      {charted.length > 0 && (
        <section className="flex flex-col gap-3">
          <h3 className="text-sm font-semibold text-fg">{t("chartsTitle")}</h3>
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
            {charted.map((s) => (
              <SeriesChart key={s.id} series={s} />
            ))}
          </div>
        </section>
      )}

      {result.state === "completed" && comparison}

      <div className="grid grid-cols-1 gap-x-8 gap-y-5 border-t border-line pt-5 lg:grid-cols-2">
        <section className="min-w-0">
          <h3 className="mb-2 text-sm font-semibold text-fg">{t("contextTitle")}</h3>
          <ContextList context={result.context} />
        </section>
        <section className="min-w-0">
          <h3 className="mb-2 text-sm font-semibold text-fg">{t("paramsTitle")}</h3>
          <RunParams result={result} />
          <p className="mt-2 text-2xs text-fg-subtle">{t("comparableNote")}</p>
        </section>
      </div>
    </div>
  );
}

/** State and start time. Duration, methodology and build are in the parameters list. */
export function ResultMeta({ result }: { result: BenchmarkResult }) {
  const t = useT("benchmarks");
  return (
    <p className="flex flex-wrap items-center gap-2 text-xs text-fg-muted tabular">
      <StatusBadge tone={stateTone(result.state)} label={stateLabel(result.state, t)} glyph={result.state !== "completed"} />
      {formatDateTime(result.startedAt)}
    </p>
  );
}

/** Safety stop and warnings, prominent (amber, with a glyph). Shown for every final state. */
function SafetyNotes({ result }: { result: BenchmarkResult }) {
  const t = useT("benchmarks");
  const debug = isDebugBuild(result);
  if (!result.safetyStop && result.warnings.length === 0 && !debug) return null;
  return (
    <div role="note" className="rounded-md border border-warn/25 bg-warn-soft px-3 py-2.5 text-sm text-fg">
      {result.safetyStop && (
        <p className="flex items-start gap-2 font-medium">
          <Icon name="alert" className="mt-0.5 shrink-0 text-warn" />
          <span className="min-w-0 break-words">{t("safetyStop", { reason: result.safetyStop })}</span>
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
              <span className="min-w-0 break-words">{w}</span>
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
    <ul aria-label={t("runNotes")} className="flex flex-col gap-0.5 text-sm text-fg-muted">
      {notes.map((n) => (
        <li key={n} className="break-words">
          {n}
        </li>
      ))}
    </ul>
  );
}

/** "più alto è meglio" / "più basso è meglio": the context that makes a number readable. */
function useDirectionText() {
  const t = useT("benchmarks");
  return (m: ResultMetric) => (m.higherIsBetter ? t("higherIsBetter") : t("lowerIsBetter"));
}

/**
 * A final result: the first metric is the headline (the backend orders them), set as the one
 * hero number of the screen (`Readout xl`) with its unit; the others follow as `Stat`s.
 */
function ResultMetrics({ metrics }: { metrics: readonly ResultMetric[] }) {
  const t = useT("benchmarks");
  const direction = useDirectionText();
  const otherId = useId();
  const [head, ...rest] = metrics;
  if (!head) return <StateView compact kind="empty" title={t("noMetrics")} />;
  const headValue = splitUnitValue(formatUnitValue(head.value, head.unit));
  return (
    <div className="flex flex-col gap-6">
      <div className="flex min-w-0 flex-col gap-1">
        <span className="text-sm text-fg-muted">{head.label}</span>
        <Readout size="xl" value={headValue.value} {...(headValue.unit ? { unit: headValue.unit } : {})} className="break-words" />
        <span className="text-xs text-fg-subtle">{direction(head)}</span>
      </div>
      {rest.length > 0 && (
        <section aria-labelledby={otherId} className="flex flex-col gap-3">
          <h3 id={otherId} className="text-sm font-semibold text-fg">
            {t("otherMeasures")}
          </h3>
          <MetricList metrics={rest} />
        </section>
      )}
    </div>
  );
}

function MetricList({ metrics }: { metrics: readonly ResultMetric[] }) {
  const direction = useDirectionText();
  return (
    <ul className="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-3">
      {metrics.map((m) => {
        const v = splitUnitValue(formatUnitValue(m.value, m.unit));
        return (
          <li key={m.id} className={`${INSET} min-w-0 px-3 py-2.5`}>
            <Stat
              label={m.label}
              value={v.value}
              {...(v.unit ? { unit: v.unit } : {})}
              size="sm"
              footer={direction(m)}
            />
          </li>
        );
      })}
    </ul>
  );
}

/**
 * Metric tiles of equal weight, for the interim readings of a run in progress. Values update
 * in place, never animated.
 */
export function MetricsGrid({ metrics, interim = false }: { metrics: readonly ResultMetric[]; interim?: boolean }) {
  const t = useT("benchmarks");
  if (metrics.length === 0) {
    return interim ? (
      <p className="text-sm text-fg-subtle">{t("interimPending")}</p>
    ) : (
      <StateView compact kind="empty" title={t("noMetrics")} />
    );
  }
  return <MetricList metrics={metrics} />;
}

const SeriesChart = memo(function SeriesChart({ series }: { series: BenchmarkSeries }) {
  // Byte labels follow the view mode (lib/format.ts).
  const viewMode = useViewMode();
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
  const formatY = useMemo(() => (v: number) => formatUnitValue(v, series.unit), [series.unit, viewMode]);

  return (
    <figure className={`${INSET} min-w-0 p-3`}>
      <figcaption className="mb-2 text-xs font-medium text-fg-muted">{series.label}</figcaption>
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
  // Power plans are a Windows concept: on macOS an absent plan is no missing reading.
  const showPowerPlan = context.powerPlan !== null || !useIsMac();
  const items: DefinitionItem[] = [
    { label: t("powerSource"), value: powerSourceLabel(context.powerSource, t) },
    ...(showPowerPlan ? [{ label: t("powerPlan"), value: context.powerPlan ?? t("notReported") }] : []),
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
  return <DefinitionList items={items} divided />;
}

function RunParams({ result }: { result: BenchmarkResult }) {
  const t = useT("benchmarks");
  const r = result.request;
  const items: DefinitionItem[] = [];
  if (r.durationS !== null) items.push({ label: t("requestedDuration"), value: `${r.durationS} s` });
  if (r.threads !== null) items.push({ label: t("threads"), value: r.threads });
  if (isStorageKind(result.kind)) {
    items.push({ label: t("volume"), value: r.storageVolume ?? "—" });
    if (r.storageFileMib !== null) items.push({ label: t("tempFile"), value: formatMib(r.storageFileMib) });
  }
  items.push(
    { label: t("actualDuration"), value: formatElapsed(result.durationMs) },
    { label: t("methodology"), value: `v${result.methodologyVersion}` },
    // A debug build is flagged once, in the warnings at the top of the result.
    { label: t("build"), value: result.buildProfile },
    { label: t("appVersion"), value: result.appVersion },
  );
  return <DefinitionList items={items} divided />;
}
