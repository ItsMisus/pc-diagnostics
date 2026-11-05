import { memo } from "react";
import { Card } from "@/components/common/Card";
import { DefinitionList } from "@/components/common/DefinitionList";
import { Icon } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { Skeleton } from "@/components/common/Skeleton";
import { UsageBar } from "@/components/common/UsageBar";
import { NotReported } from "@/components/common/Table";
import { useT } from "@/i18n";
import { formatMhz, formatPercent, formatCelsius, formatWatts } from "@/lib/format";
import { useLive } from "@/features/monitoring/liveStore";
import { isFrequencyLimited } from "@/features/monitoring/selection";
import { DefinitionSkeleton } from "@/features/monitoring/StatusViews";
import type { CpuInfo } from "@/types/generated/CpuInfo";
import type { CpuSample } from "@/types/generated/CpuSample";
import type { Metric } from "@/types/generated/Metric";
import { LiveSlot, SubHeading } from "./LiveSlot";
import { metricItem } from "./metricItem";
import { SPLIT } from "./layout";

const pct = (v: number) => formatPercent(v);

/** Per-core cells: at least 72 px wide, as many per row as fit. */
const CORE_GRID = "grid grid-cols-[repeat(auto-fill,minmax(4.5rem,1fr))] gap-x-4 gap-y-3";

export function CpuSection({ cpu }: { cpu: CpuInfo }) {
  const t = useT("hardware");
  return (
    <Card title="CPU">
      <div className={SPLIT}>
        <div className="min-w-0">
          <SubHeading>{t("specs")}</SubHeading>
          <DefinitionList
            items={[
              { label: t("model"), value: cpu.brand ?? <NotReported /> },
              { label: t("manufacturer"), value: cpu.vendor ?? <NotReported /> },
              { label: t("architecture"), value: cpu.arch },
              { label: t("physicalCores"), value: cpu.physicalCores ?? <NotReported /> },
              { label: t("logicalThreads"), value: cpu.logicalCores },
              metricItem(t("maxNominalFrequency"), cpu.maxMhz, formatMhz),
            ]}
          />
        </div>
        <div className="min-w-0">
          <SubHeading>{t("live")}</SubHeading>
          <LiveSlot placeholder={<DefinitionSkeleton rows={5} />}>
            <CpuLive />
          </LiveSlot>
        </div>
      </div>
      <div className="mt-6">
        <SubHeading>{t("perCoreUsage")}</SubHeading>
        <LiveSlot placeholder={<PerCoreSkeleton count={cpu.logicalCores} />}>
          <PerCore />
        </LiveSlot>
      </div>
    </Card>
  );
}

const CpuLive = memo(function CpuLive() {
  const t = useT("hardware");
  const tm = useT("monitoring");
  const cpu = useLive((s) => s.latest?.cpu);
  if (!cpu) return null;
  return (
    <>
      <DefinitionList
        items={[
          metricItem(t("totalUsage"), cpu.usagePercent, pct),
          metricItem(t("currentFrequency"), cpu.currentMhz, formatMhz),
          metricItem(t("frequencyLimit"), cpu.limitMhz, formatMhz),
          metricItem(tm("temperature"), cpu.temperatureC, formatCelsius),
          metricItem(tm("power"), cpu.powerW, formatWatts),
        ]}
      />
      <FrequencyLimitNote cpu={cpu} />
    </>
  );
});

function FrequencyLimitNote({ cpu }: { cpu: CpuSample }) {
  const t = useT("hardware");
  if (!isFrequencyLimited(cpu.limitMhz, cpu.maxMhz)) return null;
  return (
    <div role="note" className="mt-4 flex gap-2.5 rounded-lg border border-warn/25 bg-warn-soft px-3 py-2.5">
      <Icon name="alert" className="mt-0.5 shrink-0 text-warn" />
      <div className="min-w-0">
        <p className="text-[13px] leading-5 font-medium text-fg">{t("frequencyLimitTitle")}</p>
        <p className="mt-0.5 text-xs leading-relaxed text-fg-muted">
          {t.rich("frequencyLimitBody", {
            limit: <MetricValue metric={cpu.limitMhz} format={formatMhz} />,
            max: <MetricValue metric={cpu.maxMhz} format={formatMhz} />,
          })}
        </p>
      </div>
    </div>
  );
}

const PerCore = memo(function PerCore() {
  const perCore = useLive((s) => s.latest?.cpu.perCorePercent);
  if (!perCore) return null;
  return <PerCoreGrid metric={perCore} />;
});

function PerCoreGrid({ metric }: { metric: Metric<number[]> }) {
  const t = useT("hardware");
  if (metric.status === "unavailable") {
    return <MetricValue metric={metric} format={() => ""} className="text-sm" />;
  }
  return (
    <ul className={CORE_GRID} aria-label={t("perCoreUsage")}>
      {metric.value.map((v, i) => (
        <li key={i} className="min-w-0">
          <div className="mb-1 flex h-4 items-baseline justify-between gap-1">
            <span className="text-2xs text-fg-subtle tabular">#{i}</span>
            <span className="text-xs text-fg tabular">{formatPercent(v)}</span>
          </div>
          <UsageBar label={t("logicalProcessor", { n: i })} percent={v} valueText={formatPercent(v)} />
        </li>
      ))}
    </ul>
  );
}

/** One placeholder cell per logical processor reported by the inventory (same grid). */
function PerCoreSkeleton({ count }: { count: number }) {
  return (
    <span aria-hidden="true" className={CORE_GRID}>
      {Array.from({ length: count }, (_, i) => (
        <span key={i} className="block">
          <span className="mb-1 flex h-4 items-center justify-between">
            <Skeleton className="h-2 w-5" />
            <Skeleton className="h-2.5 w-7" />
          </span>
          <Skeleton className="h-1 w-full rounded-full" />
        </span>
      ))}
    </span>
  );
}
