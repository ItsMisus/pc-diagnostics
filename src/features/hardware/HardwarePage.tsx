import { useId, type ReactNode } from "react";
import { Button } from "@/components/common/Button";
import { Surface } from "@/components/common/Panel";
import { Skeleton, SkeletonLines } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { PageHeader } from "@/components/layout/PageHeader";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatTime } from "@/lib/format";
import { useInventoryData } from "@/features/monitoring/hooks";
import { useInventory } from "@/features/monitoring/inventoryStore";
import { DefinitionSkeleton, InventoryState } from "@/features/monitoring/StatusViews";
import type { HardwareInventory } from "@/types/generated/HardwareInventory";
import { CpuSection } from "./CpuSection";
import { GpuSection } from "./GpuSection";
import { arrangePanels } from "./layout";
import { MemorySection, hasModulesTable } from "./MemorySection";
import { StorageSection } from "./StorageSection";
import { SystemSection } from "./SystemSection";

/**
 * What the PC is: inventory and specs as a spec sheet, no live values (those live on
 * Overview, docs/ui-dashboard-brief.md rule R1). The system identity is the focal (primary)
 * panel; the components follow as secondary panels with divided spec rows. The grids are
 * size containers: they follow their own width, not the window's.
 */
export function HardwarePage() {
  const t = useT("hardware");
  const { status, data } = useInventoryData();
  const error = useInventory((s) => s.error);
  const firstLoad = status !== "ready" && (status === "idle" || status === "loading" || !error);

  return (
    <>
      <PageHeader
        title="Hardware"
        actions={
          <>
            {data && (
              <span className="text-xs text-fg-subtle tabular">{t("inventoryAt", { time: formatTime(data.collectedAt) })}</span>
            )}
            <RefreshButton disabled={status !== "ready"} />
          </>
        }
      />
      <RefreshError />

      {status === "ready" && data ? (
        <Inventory data={data} />
      ) : firstLoad ? (
        <InventoryState placeholder={<PageSkeleton />} />
      ) : (
        <Surface variant="secondary">
          <InventoryState />
        </Surface>
      )}
    </>
  );
}

function Inventory({ data }: { data: HardwareInventory }) {
  const t = useT("hardware");
  const headingId = useId();
  // Short panels share a row; GPUs (more than one adapter) and Memory (modules table, six
  // columns) take the full width when they need it. Storage always does (volumes table).
  const panels: Record<"cpu" | "gpu" | "memory", ReactNode> = {
    cpu: <CpuSection cpu={data.cpu} />,
    gpu: <GpuSection gpus={data.gpus} />,
    memory: <MemorySection memory={data.memory} />,
  };
  const arranged = arrangePanels([
    { key: "cpu", half: true },
    { key: "gpu", half: data.gpus.length <= 1 },
    { key: "memory", half: !hasModulesTable(data.memory) },
  ]);

  return (
    <div className="flex flex-col gap-8">
      <SystemSection formFactor={data.formFactor} />
      <section aria-labelledby={headingId} className="flex flex-col gap-3">
        <SectionHeading id={headingId}>{t("componentsTitle")}</SectionHeading>
        {/* The container wraps the grid: an element cannot query its own size. */}
        <div className="@container">
          <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
            {arranged.map(({ key, span }) => (
              <div key={key} className={`min-w-0 ${span === "full" ? "@3xl:col-span-2" : ""}`}>
                {panels[key]}
              </div>
            ))}
            <div className="min-w-0 @3xl:col-span-2">
              <StorageSection disks={data.physicalDisks} volumes={data.volumes} />
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

/** First load: the page's own shape (identity panel, then two component panels). */
function PageSkeleton() {
  return (
    <span aria-hidden="true" className="flex flex-col gap-8">
      <span className="surface-primary block p-5">
        <span className="flex items-center gap-4">
          <Skeleton round className="size-12 shrink-0" />
          <span className="flex w-full flex-col gap-2">
            <Skeleton className="h-5 w-56 max-w-full" />
            <Skeleton className="h-3 w-40 max-w-full" />
          </span>
        </span>
        <span className="mt-6 block">
          <DefinitionSkeleton rows={4} />
        </span>
      </span>
      {/* Same container rule as the real grid (its width, not the window's). */}
      <span className="@container block">
        <span className="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
          {[0, 1].map((i) => (
            <span key={i} className="surface-secondary block p-4">
              <Skeleton className="mb-4 h-4 w-24" />
              <SkeletonLines lines={4} />
            </span>
          ))}
        </span>
      </span>
    </span>
  );
}

function RefreshButton({ disabled }: { disabled: boolean }) {
  const t = useT("hardware");
  const refreshing = useInventory((s) => s.refreshing);
  const refresh = useInventory((s) => s.refresh);
  return (
    <Button icon="refresh" loading={refreshing} onClick={refresh} disabled={disabled}>
      {refreshing ? t("refreshing") : t("refresh")}
    </Button>
  );
}

function RefreshError() {
  const t = useT("hardware");
  const error = useInventory((s) => s.refreshError);
  const refresh = useInventory((s) => s.refresh);
  if (!error) return null;
  return (
    <div className="mb-4">
      <StateView compact kind={stateKindForError(error)} title={t("refreshFailed")} error={error} onRetry={refresh} />
    </div>
  );
}
