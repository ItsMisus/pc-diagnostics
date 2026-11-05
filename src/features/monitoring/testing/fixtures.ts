// Test-only builders. Not imported by application code.
import type { GpuInfo } from "@/types/generated/GpuInfo";
import type { GpuSample } from "@/types/generated/GpuSample";
import type { LiveSample } from "@/types/generated/LiveSample";
import type { Metric } from "@/types/generated/Metric";
import type { Unit } from "@/types/generated/Unit";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";

export const ok = (value: number, unit: Unit = "percent", at = 0): Metric<number> => ({
  status: "available",
  value,
  unit,
  source: "pdh",
  at,
});

export const missing: Metric<never> = { status: "unavailable", reason: { kind: "providerMissing", detail: "x" } };

export function gpuSample(id: string, usage: Metric<number>): GpuSample {
  return {
    id,
    usagePercent: usage,
    dedicatedUsedBytes: missing,
    temperatureC: { status: "unavailable", reason: { kind: "notImplemented" } },
    coreClockMhz: missing,
    powerW: missing,
    powerPercent: missing,
    fanRpm: missing,
  };
}

export function sample(seq: number, overrides: Partial<LiveSample> = {}): LiveSample {
  const at = 1_000_000 + seq * 1000;
  return {
    seq,
    at,
    cpu: {
      usagePercent: ok(seq % 100),
      perCorePercent: { status: "available", value: [1, 2], unit: "percent", source: "pdh", at },
      currentMhz: ok(3000, "megahertz"),
      maxMhz: ok(3600, "megahertz"),
      limitMhz: ok(3600, "megahertz"),
      temperatureC: missing,
      powerW: missing,
    },
    memory: {
      totalBytes: ok(16, "bytes"),
      usedBytes: ok(4, "bytes"),
      availableBytes: ok(12, "bytes"),
      commitUsedBytes: ok(5, "bytes"),
      commitLimitBytes: ok(20, "bytes"),
      pagesPerSec: ok(0, "count"),
    },
    gpus: [],
    storage: {
      readBytesPerSec: ok(0, "bytesPerSecond"),
      writeBytesPerSec: ok(0, "bytesPerSecond"),
      activeTimePercent: ok(0),
    },
    ...overrides,
  };
}

export function gpuInfo(id: string, isSoftware = false, isVirtual = false): GpuInfo {
  return {
    id,
    name: id,
    vendor: isSoftware ? "microsoft" : "nvidia",
    vendorId: isSoftware ? 0x1414 : 0x10de,
    deviceId: 1,
    dedicatedVramBytes: 0,
    sharedMemoryBytes: 0,
    isSoftware,
    isVirtual,
    driverVersion: missing,
    telemetrySources: [],
  };
}

export function volume(mountPoint: string, totalBytes: number, removable = false): VolumeInfo {
  return {
    mountPoint,
    label: null,
    fileSystem: "NTFS",
    totalBytes,
    availableBytes: totalBytes / 2,
    removable,
    media: "ssd",
  };
}
