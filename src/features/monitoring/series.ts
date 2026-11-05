import type { Domain, Point } from "@/components/charts/scale";
import { niceCeil } from "@/lib/format";
import type { LiveSample } from "@/types/generated/LiveSample";
import type { Metric } from "@/types/generated/Metric";

const MIB = 1024 * 1024;

/** Extracts a chartable value from a sample; `null` marks a gap. */
export type Pick = (sample: LiveSample) => number | null;

export function valueOf(metric: Metric<number> | undefined): number | null {
  return metric?.status === "available" && Number.isFinite(metric.value) ? metric.value : null;
}

/** One point per sample. Unavailable values become `v: null` so the line breaks. */
export function seriesFrom(samples: readonly LiveSample[], pick: Pick): Point[] {
  return samples.map((s) => ({ t: s.at, v: pick(s) }));
}

export const cpuUsage: Pick = (s) => valueOf(s.cpu.usagePercent);

export const memoryUsedPercent: Pick = (s) => {
  const used = valueOf(s.memory.usedBytes);
  const total = valueOf(s.memory.totalBytes);
  return used !== null && total !== null && total > 0 ? (used / total) * 100 : null;
};

/** A GPU missing from a sample is a gap, not zero. */
export function gpuUsage(id: string): Pick {
  return (s) => valueOf(s.gpus.find((g) => g.id === id)?.usagePercent);
}

export const diskRead: Pick = (s) => valueOf(s.storage.readBytesPerSec);
export const diskWrite: Pick = (s) => valueOf(s.storage.writeBytesPerSec);

export const PERCENT_DOMAIN: Domain = [0, 100];

/** X domain spanning the buffer, never narrower than `minSpanMs` (no stretched lines). */
export function timeDomain(samples: readonly LiveSample[], minSpanMs = 60_000): Domain {
  const last = samples[samples.length - 1];
  const first = samples[0];
  if (!last || !first) return [0, minSpanMs];
  return [Math.min(first.at, last.at - minSpanMs), last.at];
}

/**
 * Throughput y domain: baseline always 0, ceiling a nice number of MiB/s, at least 1 MiB/s.
 * The ceiling follows the maximum observed in the buffer, so it never zooms into a
 * non-zero baseline.
 */
export function throughputDomain(series: readonly (readonly Point[])[]): Domain {
  let max = 0;
  for (const points of series) {
    for (const p of points) if (p.v !== null && p.v > max) max = p.v;
  }
  return [0, niceCeil(Math.max(max, MIB) / MIB) * MIB];
}
