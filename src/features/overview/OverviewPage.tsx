import { useEffect, useId } from "react";
import { Panel } from "@/components/common/Panel";
import { PageHeader } from "@/components/layout/PageHeader";
import { preloadDeviceStage } from "@/features/device3d";
import { useLive } from "@/features/monitoring/liveStore";
import { LiveConnectionState } from "@/features/monitoring/StatusViews";
import { useT } from "@/i18n";
import { ComponentWidgets } from "./ComponentWidgets";
import { Hero } from "./Hero";
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
 * 2. the components as widgets: value, two-minute trend and secondary readings
 *    (Simple/Advanced rows as before), then storage;
 * 3. the top findings next to the top processes;
 * 4. the per-core grid.
 *
 * Live is the normal state, so nothing says so; a failed connection replaces the live
 * blocks with its reason. The page subscribes only to "has the live connection failed";
 * each block selects its own slice. Grids follow the content width (container queries).
 */
export function OverviewPage() {
  const t = useT("overview");
  const failed = useLive((s) => s.status === "unsupported" || s.status === "permission" || s.status === "error");
  const componentsId = useId();
  useEffect(preloadStageWhenIdle, []);

  return (
    <>
      <PageHeader title="Overview" />

      <div className="@container flex flex-col gap-10">
        <Hero />

        <ComponentWidgets
          headingId={componentsId}
          liveFailed={
            failed ? (
              <Panel title={t("monitoringCard")}>
                <LiveConnectionState />
              </Panel>
            ) : null
          }
        />
        {!failed && <LiveConnectionState compact />}

        <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-5">
          <TopFindings className="min-w-0 @3xl:col-span-2" />
          <TopProcesses className="min-w-0 @3xl:col-span-3" />
        </div>

        {!failed && <PerCorePanel />}
      </div>
    </>
  );
}
