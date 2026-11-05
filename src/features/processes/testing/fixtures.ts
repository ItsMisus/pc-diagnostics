// Test-only builders. Not imported by application code.
import type { Metric } from "@/types/generated/Metric";
import type { ProcessRow } from "@/types/generated/ProcessRow";
import type { ProcessSnapshot } from "@/types/generated/ProcessSnapshot";
import type { Unit } from "@/types/generated/Unit";

export const ok = (value: number, unit: Unit = "percent"): Metric<number> => ({
  status: "available",
  value,
  unit,
  source: "pdh",
  at: 0,
});

export const denied: Metric<number> = { status: "unavailable", reason: { kind: "permissionDenied" } };

export function row(name: string, overrides: Partial<ProcessRow> & { impact?: number } = {}): ProcessRow {
  const { impact = 10, ...rest } = overrides;
  return {
    key: `${name}:1`,
    pid: 1,
    name,
    category: "application",
    accessible: true,
    isSelf: false,
    exePath: null,
    cpuPercent: ok(1),
    workingSetBytes: ok(1024, "bytes"),
    privateBytes: ok(1024, "bytes"),
    ioReadBytesPerSec: ok(0, "bytesPerSecond"),
    ioWriteBytesPerSec: ok(0, "bytesPerSecond"),
    pressure: { cpu: 0, ram: 0, io: 0, impact },
    impactPeak: impact,
    ...rest,
  };
}

export function snapshot(at: number, rows: ProcessRow[] = []): ProcessSnapshot {
  return {
    at,
    intervalMs: 2000,
    windowS: 60,
    methodologyVersion: 1,
    weights: { cpu: 0.45, ram: 0.25, io: 0.3 },
    context: {
      logicalProcessors: 8,
      totalMemoryBytes: 16 * 1024 ** 3,
      availableMemoryBytes: 8 * 1024 ** 3,
      ioReferenceBytesPerSec: 100 * 1024 ** 2,
    },
    totalProcesses: rows.length,
    inaccessibleProcesses: rows.filter((r) => !r.accessible).length,
    rankingInstant: rows,
    rankingWindow: rows,
  };
}
