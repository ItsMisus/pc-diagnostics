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
import { InventoryState, LiveBadge, LiveConnectionState } from "@/features/monitoring/StatusViews";
import { CpuSection } from "./CpuSection";
import { GpuSection } from "./GpuSection";
import { MemorySection } from "./MemorySection";
import { StorageSection } from "./StorageSection";

/**
 * Static inventory plus live values. The page subscribes only to the inventory; every
 * live block selects its own slice of the latest sample. The content is a size container:
 * the sections' column splits follow its width, not the window's.
 */
export function HardwarePage() {
  const t = useT("hardware");
  const { status, data } = useInventoryData();

  return (
    <>
      <PageHeader
        title="Hardware"
        description={t("pageDescription")}
        actions={
          <>
            <LiveBadge />
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
        <div className="@container flex flex-col gap-4">
          <div className="flex flex-col gap-1">
            <p className="text-xs text-fg-subtle tabular">{t("inventoryAt", { time: formatTime(data.collectedAt) })}</p>
            <LiveConnectionState compact />
          </div>
          <CpuSection cpu={data.cpu} />
          <GpuSection gpus={data.gpus} />
          <MemorySection memory={data.memory} />
          <StorageSection disks={data.physicalDisks} volumes={data.volumes} />
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
      <StateView
        compact
        kind={stateKindForError(error)}
        title={t("refreshFailed")}
        error={error}
        onRetry={refresh}
      />
    </div>
  );
}
