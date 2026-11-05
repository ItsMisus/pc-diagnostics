import { Card } from "@/components/common/Card";
import { PageHeader } from "@/components/layout/PageHeader";
import { InsightsCard } from "@/features/diagnostics/InsightsCard";
import { CpuChart, GpuChart, MemoryChart, ThroughputChart } from "@/features/monitoring/LiveCharts";
import { useLive } from "@/features/monitoring/liveStore";
import { LiveBadge, LiveConnectionState } from "@/features/monitoring/StatusViews";
import { useT } from "@/i18n";
import { CpuCard, GpuCard, RamCard, SsdCard } from "./LiveCards";
import { SystemSummary } from "./SystemSummary";

/**
 * The page itself subscribes only to "has the live connection failed", so it re-renders
 * on status changes, not per sample. Each card and chart subscribes to its own slice.
 *
 * Grids respond to the content width (container queries), not the window, so collapsing
 * the sidebar or resizing never squeezes a column: cards go 2 → 4 columns from 896 px,
 * and every grid shares the same column lines (charts and the bottom row span 2 cards).
 */
export function OverviewPage() {
  const t = useT("overview");
  const failed = useLive((s) => s.status === "unsupported" || s.status === "permission" || s.status === "error");

  return (
    <>
      <PageHeader title="Overview" description={t("pageDescription")} actions={<LiveBadge />} />

      <div className="@container flex flex-col gap-6">
        {failed ? (
          <Card title={t("monitoringCard")}>
            <LiveConnectionState />
          </Card>
        ) : (
          <>
            <div className="grid grid-cols-1 gap-4 @lg:grid-cols-2 @4xl:grid-cols-4">
              <CpuCard />
              <GpuCard />
              <RamCard />
              <SsdCard />
            </div>

            <section>
              <h2 className="mb-3 text-sm leading-5 font-medium text-fg">{t("recentTrend")}</h2>
              <div className="grid grid-cols-1 gap-4 @lg:grid-cols-2">
                <CpuChart />
                <MemoryChart />
                <GpuChart />
                <ThroughputChart />
              </div>
            </section>
          </>
        )}

        <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
          <Card title={t("systemCard")}>
            <SystemSummary />
          </Card>
          <InsightsCard />
        </div>
      </div>
    </>
  );
}
