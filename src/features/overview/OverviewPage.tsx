import { useEffect, useId } from "react";
import { Panel } from "@/components/common/Panel";
import { PageHeader } from "@/components/layout/PageHeader";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { preloadDeviceStage } from "@/features/device3d";
import { CpuChart, GpuChart, MemoryChart, ThroughputChart } from "@/features/monitoring/LiveCharts";
import { useLive } from "@/features/monitoring/liveStore";
import { LiveConnectionState } from "@/features/monitoring/StatusViews";
import { useT } from "@/i18n";
import { EssentialMetrics } from "./EssentialMetrics";
import { Hero } from "./Hero";
import { CpuPanel, DiskPanel, GpuPanel, RamPanel, StoragePanel } from "./Panels";
import { PerCorePanel } from "./PerCorePanel";
import { TopFindings } from "./TopFindings";
import { TopProcesses } from "./TopProcesses";

/** Fetches the 3D bundle once the app is idle, so the stage appears without waiting. */
function preloadStageWhenIdle(): () => void {
  if (typeof requestIdleCallback === "function") {
    const id = requestIdleCallback(preloadDeviceStage, { timeout: 2000 });
    return () => cancelIdleCallback(id);
  }
  const id = setTimeout(preloadDeviceStage, 300);
  return () => clearTimeout(id);
}

/**
 * The start page (docs/design-system.md, "Screens > Overview"):
 * 1. the hero, the one focal element: the 3D device, its identity and the overall state
 *    from the latest diagnosis, with the quick actions;
 * 2. the essential readings with a one-minute sparkline each;
 * 3. the top findings next to the top processes;
 * 4. the details of every component (Simple/Advanced rows as before) and the per-core grid;
 * 5. the recent trend charts.
 *
 * Live is the normal state, so nothing says so; a failed connection replaces the live
 * blocks with its reason. The page subscribes only to "has the live connection failed";
 * each block selects its own slice. Grids follow the content width (container queries).
 */
export function OverviewPage() {
  const t = useT("overview");
  const failed = useLive((s) => s.status === "unsupported" || s.status === "permission" || s.status === "error");
  const essentialsId = useId();
  const detailsId = useId();
  const trendId = useId();
  useEffect(preloadStageWhenIdle, []);

  return (
    <>
      <PageHeader title="Overview" />

      <div className="@container flex flex-col gap-8">
        <Hero />

        {failed ? (
          <Panel title={t("monitoringCard")}>
            <LiveConnectionState />
          </Panel>
        ) : (
          <EssentialMetrics headingId={essentialsId} />
        )}

        <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-5">
          <TopFindings className="min-w-0 @3xl:col-span-2" />
          <TopProcesses className="min-w-0 @3xl:col-span-3" />
        </div>

        <section aria-labelledby={detailsId} className="flex flex-col gap-4">
          <SectionHeading id={detailsId}>{t("detailsTitle")}</SectionHeading>
          {failed ? (
            <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-3">
              <StoragePanel />
            </div>
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
              <PerCorePanel />
            </>
          )}
        </section>

        {!failed && (
          <section aria-labelledby={trendId} className="flex flex-col gap-4">
            <SectionHeading id={trendId}>{t("recentTrend")}</SectionHeading>
            <div className="grid grid-cols-1 gap-4 @lg:grid-cols-2 @4xl:grid-cols-4">
              <CpuChart />
              <MemoryChart />
              <GpuChart />
              <ThroughputChart />
            </div>
          </section>
        )}
      </div>
    </>
  );
}
