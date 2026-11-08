import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { Spinner, StateView } from "@/components/common/StateView";
import { PageHeader } from "@/components/layout/PageHeader";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatTime } from "@/lib/format";
import { useInventoryData } from "@/features/monitoring/hooks";
import { useInventory } from "@/features/monitoring/inventoryStore";
import { InventoryState } from "@/features/monitoring/StatusViews";
import { CpuSection } from "./CpuSection";
import { GpuSection } from "./GpuSection";
import { MemorySection } from "./MemorySection";
import { StorageSection } from "./StorageSection";
import { SystemSection } from "./SystemSection";

/**
 * What the PC is: inventory and specs, one panel per component, no live values (those live
 * on Overview, docs/ui-dashboard-brief.md rule R1). The content is a size container: the
 * grid follows its width, not the window's.
 */
export function HardwarePage() {
  const t = useT("hardware");
  const { status, data } = useInventoryData();
  // GPU and Memory share a row when both are short: one adapter, and no modules table (it
  // has six columns and needs the full width). Otherwise each takes the full width.
  const pairGpuMemory =
    data !== null &&
    data !== undefined &&
    data.gpus.length <= 1 &&
    (data.memory.modules.status === "unavailable" || data.memory.modules.value.length === 0);

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

      {status !== "ready" || !data ? (
        <Card>
          <InventoryState />
        </Card>
      ) : (
        // The container wraps the grid: an element cannot query its own size, so with both on
        // one element the two-column rule never applied and the spanning panels made an
        // implicit, content-sized second column (unequal widths).
        <div className="@container">
          <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
            <SystemSection />
            <CpuSection cpu={data.cpu} />
            <div className={`min-w-0 ${pairGpuMemory ? "" : "@3xl:col-span-2"}`}>
              <GpuSection gpus={data.gpus} />
            </div>
            <div className={`min-w-0 ${pairGpuMemory ? "" : "@3xl:col-span-2"}`}>
              <MemorySection memory={data.memory} />
            </div>
            <div className="min-w-0 @3xl:col-span-2">
              <StorageSection disks={data.physicalDisks} volumes={data.volumes} />
            </div>
          </div>
        </div>
      )}
    </>
  );
}

function RefreshButton({ disabled }: { disabled: boolean }) {
  const t = useT("hardware");
  const refreshing = useInventory((s) => s.refreshing);
  const refresh = useInventory((s) => s.refresh);
  return (
    <Button onClick={refresh} disabled={disabled || refreshing} aria-busy={refreshing}>
      {refreshing ? <Spinner /> : <Icon name="refresh" />}
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
