import { useEffect } from "react";
import { DefinitionList, type DefinitionItem } from "@/components/common/DefinitionList";
import { Skeleton } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { type Translator, translatorFor, useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatBytes, formatMts } from "@/lib/format";
import { useInventory } from "@/features/monitoring/inventoryStore";
import { mediaLabel } from "@/features/monitoring/labels";
import type { HardwareInventory } from "@/types/generated/HardwareInventory";
import type { BenchComponent } from "./model";

type T = Translator<"benchmarks">;

/** One-line name of the component, for the cards. */
export function componentSummary(
  inv: HardwareInventory,
  component: BenchComponent,
  t: T = translatorFor("benchmarks"),
): string {
  switch (component) {
    case "cpu":
      return inv.cpu.brand ?? t("cpuUnidentified");
    case "gpu": {
      const real = inv.gpus.filter((g) => !g.isSoftware && !g.isVirtual);
      return real.length > 0 ? real.map((g) => g.name).join(", ") : t("noHardwareGpu");
    }
    case "memory":
      return t("installed", { size: formatBytes(inv.memory.totalBytes) });
    case "storage":
      return t.plural("volumes", inv.volumes.length);
  }
}

export function useEnsureInventory() {
  const ensure = useInventory((s) => s.ensureLoaded);
  useEffect(() => ensure(), [ensure]);
}

/** Detected hardware for the open screen, from the shared (cached) inventory. */
export function DetectedComponent({ component }: { component: BenchComponent }) {
  useEnsureInventory();
  const t = useT("benchmarks");
  const tm = useT("monitoring");
  const data = useInventory((s) => s.data);
  const status = useInventory((s) => s.status);
  const error = useInventory((s) => s.error);
  const reload = useInventory((s) => s.reload);

  if (!data) {
    if (status === "error" && error) {
      return (
        <StateView
          compact
          kind={stateKindForError(error)}
          title={t("componentNotDetected")}
          error={error}
          onRetry={reload}
        />
      );
    }
    // First load: a placeholder shaped like the list it precedes.
    return (
      <div role="status" aria-busy="true" className="flex flex-col gap-2.5 py-0.5">
        <span className="sr-only">{t("detectingComponent")}</span>
        {["w-48", "w-36", "w-40"].map((w) => (
          <span key={w} className="flex items-center gap-6">
            <Skeleton className="h-3 w-24" />
            <Skeleton className={`h-3 ${w}`} />
          </span>
        ))}
      </div>
    );
  }
  return <DefinitionList items={items(data, component, t, tm)} />;
}

function items(
  inv: HardwareInventory,
  component: BenchComponent,
  t: T,
  tm: Translator<"monitoring">,
): DefinitionItem[] {
  switch (component) {
    case "cpu":
      return [
        { label: t("processor"), value: inv.cpu.brand ?? t("notIdentified") },
        { label: t("physicalCores"), value: inv.cpu.physicalCores ?? t("notReportedPlural") },
        { label: t("logicalProcessors"), value: inv.cpu.logicalCores },
      ];
    case "gpu": {
      const real = inv.gpus.filter((g) => !g.isSoftware && !g.isVirtual);
      if (real.length === 0) return [{ label: "GPU", value: t("noHardwareGpu") }];
      return real.map((g, i) => ({
        label: real.length > 1 ? t("gpuN", { n: i + 1 }) : "GPU",
        value: `${g.name}${g.dedicatedVramBytes > 0 ? ` · ${t("vramDedicated", { size: formatBytes(g.dedicatedVramBytes) })}` : ""}`,
      }));
    }
    case "memory": {
      const list: DefinitionItem[] = [{ label: t("total"), value: formatBytes(inv.memory.totalBytes) }];
      if (inv.memory.modules.status === "available") {
        const modules = inv.memory.modules.value;
        // An empty list (e.g. memory built into the chip package) is no count of modules.
        list.push({ label: t("modules"), value: modules.length > 0 ? modules.length : t("noModulesReported") });
        modules.forEach((m, i) => {
          const speed = m.configuredSpeedMts ?? m.speedMts;
          list.push({
            label: t("moduleN", { n: i + 1 }),
            value: [formatBytes(m.capacityBytes, 0), speed ? formatMts(speed) : null, m.manufacturer, m.slot]
              .filter(Boolean)
              .join(" · "),
          });
        });
      } else {
        list.push({ label: t("modules"), value: t("detailUnavailable") });
      }
      return list;
    }
    case "storage":
      if (inv.volumes.length === 0) return [{ label: t("volumesLabel"), value: t("noVolumes") }];
      return inv.volumes.map((v) => ({
        label: v.mountPoint,
        value: [
          v.label || null,
          mediaLabel(v.media, tm),
          t("freeOfTotal", { free: formatBytes(v.availableBytes), total: formatBytes(v.totalBytes) }),
          v.removable ? t("removable") : null,
        ]
          .filter(Boolean)
          .join(" · "),
      }));
  }
}
