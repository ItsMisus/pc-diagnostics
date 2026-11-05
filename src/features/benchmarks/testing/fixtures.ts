import type { BenchmarkCapability } from "@/types/generated/BenchmarkCapability";
import type { BenchmarkGpuTarget } from "@/types/generated/BenchmarkGpuTarget";
import type { BenchmarkKind } from "@/types/generated/BenchmarkKind";
import type { BenchmarkProgress } from "@/types/generated/BenchmarkProgress";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";
import type { MetricDelta } from "@/types/generated/MetricDelta";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";

export function capability(kind: BenchmarkKind, patch: Partial<BenchmarkCapability> = {}): BenchmarkCapability {
  const storage = kind === "storageSequential" || kind === "storageRandom";
  return {
    kind,
    availability: { status: "available", value: true, unit: "none", source: "benchmark", at: 0 },
    description: "test",
    measures: ["throughput"],
    methodologyVersion: 1,
    defaultDurationS: storage ? null : 10,
    maxDurationS: storage ? null : 60,
    supportsThreads: kind === "cpuMultiCore" || kind === "cpuSustained",
    maxThreads: null,
    defaultFileMib: storage ? 256 : null,
    maxFileMib: storage ? 1024 : null,
    maxBytesWritten: storage ? 1024 * 1024 * 1024 : null,
    gpuTargets: [],
    ...patch,
  };
}

export function gpuTarget(id: string, patch: Partial<BenchmarkGpuTarget> = {}): BenchmarkGpuTarget {
  return {
    id,
    name: `GPU ${id}`,
    isDefault: false,
    availability: { status: "available", value: true, unit: "none", source: "benchmark", at: 0 },
    stopTemperatureC: 87,
    maxDurationS: 60,
    ...patch,
  };
}

export function progress(runId: string, patch: Partial<BenchmarkProgress> = {}): BenchmarkProgress {
  return {
    runId,
    kind: "cpuSingleCore",
    state: "running",
    fraction: 0.5,
    elapsedMs: 5000,
    phase: "Calcolo",
    interim: [],
    ...patch,
  };
}

export function result(runId: string, patch: Partial<BenchmarkResult> = {}): BenchmarkResult {
  return {
    runId,
    kind: "cpuSingleCore",
    state: "completed",
    methodologyVersion: 1,
    appVersion: "0.1.0",
    buildProfile: "release",
    startedAt: 1_000,
    finishedAt: 11_000,
    durationMs: 10_000,
    request: {
      kind: "cpuSingleCore",
      durationS: 10,
      threads: null,
      storageVolume: null,
      storageFileMib: null,
      storageWriteConsent: false,
      gpuId: null,
    },
    context: {
      osVersion: "Windows 11",
      cpuBrand: "CPU",
      logicalProcessors: 8,
      totalMemoryBytes: 16 * 1024 ** 3,
      powerSource: "ac",
      powerPlan: "Balanced",
      backgroundCpuPercent: { status: "available", value: 3, unit: "percent", source: "pdh", at: 0 },
      availableMemoryBytes: { status: "unavailable", reason: { kind: "notImplemented" } },
      cpuLimitMhz: { status: "unavailable", reason: { kind: "notImplemented" } },
      cpuMaxMhz: { status: "unavailable", reason: { kind: "notImplemented" } },
      storageModel: null,
      gpuName: null,
      gpuDriverVersion: null,
    },
    metrics: [{ id: "score", label: "Punteggio", value: 1000, unit: "none", higherIsBetter: true }],
    series: [],
    warnings: [],
    notes: [],
    safetyStop: null,
    error: null,
    ...patch,
  };
}

export function volume(mountPoint: string, patch: Partial<VolumeInfo> = {}): VolumeInfo {
  return {
    mountPoint,
    label: null,
    fileSystem: "NTFS",
    totalBytes: 500 * 1024 ** 3,
    availableBytes: 100 * 1024 ** 3,
    removable: false,
    media: "ssd",
    ...patch,
  };
}

export function delta(patch: Partial<MetricDelta> = {}): MetricDelta {
  return {
    id: "score",
    label: "Punteggio",
    unit: "none",
    current: 110,
    previous: 100,
    changePercent: 10,
    higherIsBetter: true,
    ...patch,
  };
}
