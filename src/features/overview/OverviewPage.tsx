import { Card } from "@/components/common/Card";
import { PageHeader } from "@/components/layout/PageHeader";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { InsightsCard } from "@/features/diagnostics/InsightsCard";
import { CpuChart, GpuChart, MemoryChart, ThroughputChart } from "@/features/monitoring/LiveCharts";
import { useLive } from "@/features/monitoring/liveStore";
import { LiveConnectionState } from "@/features/monitoring/StatusViews";
import { useT } from "@/i18n";
import { CpuPanel, DiskPanel, GpuPanel, RamPanel, StoragePanel } from "./Panels";
import { PerCorePanel } from "./PerCorePanel";
import { TopProcesses } from "./TopProcesses";

/**
 * The dashboard: every live value of the PC on one screen (docs/ui-dashboard-brief.md).
 * Row 1 CPU and GPU, row 2 RAM, disk activity and storage, then top processes and insights,
 * the per-core grid (collapsed) and the recent trend. Live is the normal state, so nothing
 * says so; a failed connection replaces the live panels with its reason.
 *
 * The page subscribes only to "has the live connection failed"; each panel selects its own
 * slice. Grids follow the content width (container queries), not the window.
 */
export function OverviewPage() {
  const t = useT("overview");
  const failed = useLive((s) => s.status === "unsupported" || s.status === "permission" || s.status === "error");

  return (
    <>
      <PageHeader title="Overview" />

      <div className="@container flex flex-col gap-4">
        {failed ? (
          <>
            <Card title={t("monitoringCard")}>
              <LiveConnectionState />
            </Card>
            <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-3">
              <StoragePanel />
            </div>
          </>
        ) : (
          <>
            <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
              <CpuPanel />
              <GpuPanel />
            </div>
            <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-3">
              <RamPanel />
              <DiskPanel />
              <StoragePanel />
            </div>
          </>
        )}

        <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-3">
          <TopProcesses className="min-w-0 @3xl:col-span-2" />
          <InsightsCard className="min-w-0" />
        </div>

        {!failed && (
          <>
            <PerCorePanel />
            <section>
              <SectionHeading className="mb-3">{t("recentTrend")}</SectionHeading>
              <div className="grid grid-cols-1 gap-4 @lg:grid-cols-2 @4xl:grid-cols-4">
                <CpuChart />
                <MemoryChart />
                <GpuChart />
                <ThroughputChart />
              </div>
            </section>
          </>
        )}
      </div>
    </>
  );
}
