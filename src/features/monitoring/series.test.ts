import { describe, expect, it } from "vitest";
import {
  cpuUsage,
  diskRead,
  gpuUsage,
  memoryUsedPercent,
  seriesFrom,
  throughputDomain,
  timeDomain,
} from "./series";
import { gpuSample, missing, ok, sample } from "./testing/fixtures";

const MIB = 1024 * 1024;

describe("seriesFrom", () => {
  it("uses sample time and value, with null for unavailable metrics", () => {
    const samples = [
      sample(1, { cpu: { ...sample(1).cpu, usagePercent: ok(10) } }),
      sample(2, { cpu: { ...sample(2).cpu, usagePercent: missing } }),
      sample(3, { cpu: { ...sample(3).cpu, usagePercent: ok(30) } }),
    ];
    expect(seriesFrom(samples, cpuUsage)).toEqual([
      { t: samples[0]?.at, v: 10 },
      { t: samples[1]?.at, v: null },
      { t: samples[2]?.at, v: 30 },
    ]);
  });

  it("treats non-finite values as gaps", () => {
    const s = sample(1, { cpu: { ...sample(1).cpu, usagePercent: ok(Number.NaN) } });
    expect(seriesFrom([s], cpuUsage)[0]?.v).toBeNull();
  });

  it("derives memory percent only when used and total are available", () => {
    const full = sample(1);
    const noTotal = sample(2, { memory: { ...sample(2).memory, totalBytes: missing } });
    expect(seriesFrom([full, noTotal], memoryUsedPercent).map((p) => p.v)).toEqual([25, null]);
  });

  it("joins GPUs by id; a missing adapter is a gap, not zero", () => {
    const a = sample(1, { gpus: [gpuSample("luid:a", ok(40)), gpuSample("luid:b", ok(5))] });
    const b = sample(2, { gpus: [gpuSample("luid:b", ok(6))] });
    const c = sample(3, { gpus: [gpuSample("luid:a", missing)] });
    expect(seriesFrom([a, b, c], gpuUsage("luid:a")).map((p) => p.v)).toEqual([40, null, null]);
  });

  it("reads disk throughput", () => {
    const s = sample(1, { storage: { ...sample(1).storage, readBytesPerSec: ok(5 * MIB, "bytesPerSecond") } });
    expect(seriesFrom([s], diskRead)[0]?.v).toBe(5 * MIB);
  });
});

describe("throughputDomain", () => {
  it("starts at 0 with a floor of 1 MiB/s", () => {
    expect(throughputDomain([])).toEqual([0, MIB]);
    expect(throughputDomain([[{ t: 0, v: 1000 }, { t: 1, v: null }]])).toEqual([0, MIB]);
  });

  it("rounds the max observed up to a nice MiB value", () => {
    const d = throughputDomain([
      [{ t: 0, v: 120 * MIB }],
      [{ t: 0, v: 340 * MIB }],
    ]);
    expect(d).toEqual([0, 500 * MIB]);
  });

  it("never moves the baseline away from zero", () => {
    const d = throughputDomain([[{ t: 0, v: 900 * MIB }, { t: 1, v: 950 * MIB }]]);
    expect(d[0]).toBe(0);
    expect(d[1]).toBeGreaterThanOrEqual(950 * MIB);
  });
});

describe("timeDomain", () => {
  it("spans at least the minimum window ending at the last sample", () => {
    const samples = [sample(1), sample(2)];
    const last = samples[1]?.at ?? 0;
    expect(timeDomain(samples, 60_000)).toEqual([last - 60_000, last]);
  });

  it("covers the whole buffer when it is longer than the window", () => {
    const samples = Array.from({ length: 120 }, (_, i) => sample(i + 1));
    const d = timeDomain(samples, 60_000);
    expect(d).toEqual([samples[0]?.at, samples[119]?.at]);
  });

  it("has a fallback for an empty buffer", () => {
    expect(timeDomain([], 1000)).toEqual([0, 1000]);
  });
});
