import { MetricValue } from "@/components/common/MetricValue";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useT } from "@/i18n";
import type { DiskHealthStatus } from "@/types/generated/DiskHealthStatus";
import type { Metric } from "@/types/generated/Metric";
import { healthLabel, healthTone } from "./selection";

/**
 * The OS's disk health summary (Windows Storage on Windows, the status macOS reports on a Mac;
 * not a SMART evaluation). Text always states the status.
 */
export function DiskHealth({ health }: { health: Metric<DiskHealthStatus> }) {
  const t = useT("monitoring");
  if (health.status === "unavailable") {
    return <MetricValue metric={health} format={(s: DiskHealthStatus) => healthLabel(s, t)} className="text-xs" />;
  }
  return <StatusBadge tone={healthTone(health.value)} label={healthLabel(health.value, t)} />;
}
