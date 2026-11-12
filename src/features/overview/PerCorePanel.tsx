import { memo, useState } from "react";
import { Icon } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { UsageBar } from "@/components/common/UsageBar";
import { useT } from "@/i18n";
import { formatPercent } from "@/lib/format";
import { useLive } from "@/features/monitoring/liveStore";
import { WaitingForSample } from "@/features/monitoring/StatusViews";
import type { Metric } from "@/types/generated/Metric";

/** Per-core cells: at least 72 px wide, as many per row as fit. */
const CORE_GRID = "grid grid-cols-[repeat(auto-fill,minmax(4.5rem,1fr))] gap-x-4 gap-y-3";

/**
 * Usage per logical processor, collapsed by default. A panel that is itself a native
 * `<details>` (title in the summary); the grid is mounted only while open, so a closed panel
 * does not re-render on every sample.
 */
export function PerCorePanel() {
  const t = useT("overview");
  const [open, setOpen] = useState(false);
  return (
    <details className="group surface-secondary" onToggle={(e) => setOpen(e.currentTarget.open)}>
      <summary className="flex cursor-pointer items-center gap-2 rounded-lg px-4 py-3 select-none">
        <Icon
          name="chevronRight"
          size={14}
          strokeWidth={1.75}
          className="shrink-0 text-fg-subtle transition-transform duration-200 ease-out group-open:rotate-90"
        />
        <h3 className="font-display text-md font-semibold text-fg">{t("perCoreTitle")}</h3>
      </summary>
      <div className="px-4 pb-4">{open && <PerCore />}</div>
    </details>
  );
}

const PerCore = memo(function PerCore() {
  const perCore = useLive((s) => s.latest?.cpu.perCorePercent);
  if (!perCore) return <WaitingForSample />;
  return <PerCoreGrid metric={perCore} />;
});

function PerCoreGrid({ metric }: { metric: Metric<number[]> }) {
  const t = useT("overview");
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
