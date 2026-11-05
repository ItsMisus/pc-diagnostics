import { type Translator, translatorFor } from "@/i18n";
import { formatPercent, formatWatts } from "@/lib/format";
import type { GpuSample } from "@/types/generated/GpuSample";
import type { Metric } from "@/types/generated/Metric";

export interface GpuPowerReading {
  metric: Metric<number>;
  format: (value: number) => string;
}

/**
 * Board power to show for a GPU: watts when a provider measures them (NVML), otherwise
 * the driver's share of the adapter maximum (WDDM), which is never converted to watts.
 * When both are missing, the more specific reason wins over "not implemented".
 */
export function gpuPowerReading(
  sample: Pick<GpuSample, "powerW" | "powerPercent">,
  t: Translator<"monitoring"> = translatorFor("monitoring"),
): GpuPowerReading {
  if (sample.powerW.status === "available") return { metric: sample.powerW, format: formatWatts };
  if (sample.powerPercent.status === "available") {
    return { metric: sample.powerPercent, format: (v) => t("percentOfMax", { value: formatPercent(v) }) };
  }
  const metric =
    sample.powerW.reason.kind === "notImplemented" && sample.powerPercent.reason.kind !== "notImplemented"
      ? sample.powerPercent
      : sample.powerW;
  return { metric, format: formatWatts };
}
