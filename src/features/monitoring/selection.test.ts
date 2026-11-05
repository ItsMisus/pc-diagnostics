import { describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import { busLabel, gpuVendorLabel, mediaLabel } from "./labels";
import {
  healthLabel,
  healthTone,
  isFrequencyLimited,
  isSystemMount,
  orderGpus,
  primaryGpu,
  systemVolume,
} from "./selection";
import { gpuInfo, missing, ok, volume } from "./testing/fixtures";

const GIB = 1024 ** 3;

describe("primaryGpu", () => {
  it("picks the first non-software adapter", () => {
    expect(primaryGpu([gpuInfo("basic", true), gpuInfo("a"), gpuInfo("b")])?.id).toBe("a");
  });

  it("returns null when only software adapters exist, or none", () => {
    expect(primaryGpu([gpuInfo("basic", true)])).toBeNull();
    expect(primaryGpu([])).toBeNull();
  });

  it("skips virtual display adapters that duplicate a real GPU", () => {
    expect(primaryGpu([gpuInfo("parsec", false, true), gpuInfo("rtx")])?.id).toBe("rtx");
    const ids = orderGpus([gpuInfo("s", true), gpuInfo("v", false, true), gpuInfo("a")]).map((g) => g.id);
    expect(ids).toEqual(["a", "v", "s"]);
  });

  it("orders hardware adapters before software ones, stable otherwise", () => {
    const ids = orderGpus([gpuInfo("s", true), gpuInfo("a"), gpuInfo("b")]).map((g) => g.id);
    expect(ids).toEqual(["a", "b", "s"]);
  });
});

describe("systemVolume", () => {
  it("prefers C:\\ in any spelling", () => {
    expect(isSystemMount("C:\\")).toBe(true);
    expect(isSystemMount("c:")).toBe(true);
    expect(isSystemMount("C:/")).toBe(true);
    expect(isSystemMount("D:\\")).toBe(false);
    expect(systemVolume([volume("D:\\", 2000 * GIB), volume("C:\\", 500 * GIB)])?.mountPoint).toBe("C:\\");
  });

  it("falls back to the largest fixed volume, then the largest overall", () => {
    expect(
      systemVolume([volume("D:\\", 100 * GIB), volume("E:\\", 900 * GIB), volume("F:\\", 4000 * GIB, true)])
        ?.mountPoint,
    ).toBe("E:\\");
    expect(systemVolume([volume("F:\\", 32 * GIB, true), volume("G:\\", 64 * GIB, true)])?.mountPoint).toBe("G:\\");
    expect(systemVolume([])).toBeNull();
  });
});

describe("disk health", () => {
  it("maps tones and always provides a text label", () => {
    expect(healthTone("healthy")).toBe("ok");
    expect(healthTone("warning")).toBe("warn");
    expect(healthTone("unhealthy")).toBe("bad");
    expect(healthTone("unknown")).toBe("neutral");
    for (const s of ["healthy", "warning", "unhealthy", "unknown"] as const) {
      expect(healthLabel(s).length).toBeGreaterThan(0);
    }
    expect(healthLabel("unhealthy")).toBe("Non integro");
  });

  it("labels health and inventory enums in English", () => {
    setLang("en");
    try {
      expect(healthLabel("healthy")).toBe("Healthy");
      expect(healthLabel("unhealthy")).toBe("Unhealthy");
      expect(busLabel("virtual")).toBe("Virtual");
      expect(mediaLabel("unknown")).toBe("Unknown");
      expect(gpuVendorLabel("other", 0x1af4)).toBe("Other (0x1AF4)");
    } finally {
      setLang("it");
    }
    expect(gpuVendorLabel("other", 0x1af4)).toBe("Altro (0x1AF4)");
  });
});

describe("isFrequencyLimited", () => {
  it("flags a limit clearly below the nominal max", () => {
    expect(isFrequencyLimited(ok(2400, "megahertz"), ok(3600, "megahertz"))).toBe(true);
  });

  it("tolerates rounding and ignores unavailable values", () => {
    expect(isFrequencyLimited(ok(3590, "megahertz"), ok(3600, "megahertz"))).toBe(false);
    expect(isFrequencyLimited(ok(3600, "megahertz"), ok(3600, "megahertz"))).toBe(false);
    expect(isFrequencyLimited(missing, ok(3600, "megahertz"))).toBe(false);
    expect(isFrequencyLimited(ok(0, "megahertz"), ok(3600, "megahertz"))).toBe(false);
  });
});
