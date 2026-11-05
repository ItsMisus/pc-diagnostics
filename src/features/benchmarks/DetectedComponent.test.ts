import { describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import { gpuInfo, volume } from "@/features/monitoring/testing/fixtures";
import type { HardwareInventory } from "@/types/generated/HardwareInventory";
import { componentSummary } from "./DetectedComponent";

const inventory = (patch: Partial<HardwareInventory>): HardwareInventory =>
  ({
    cpu: { brand: null },
    gpus: [],
    memory: { totalBytes: 16 * 1024 ** 3 },
    volumes: [],
    ...patch,
  }) as unknown as HardwareInventory;

describe("componentSummary", () => {
  it("summarises each component in Italian", () => {
    const inv = inventory({ volumes: [volume("C:\\", 1e12)], gpus: [gpuInfo("soft", true)] });
    expect(componentSummary(inv, "cpu")).toBe("CPU non identificata");
    expect(componentSummary(inv, "gpu")).toBe("Nessuna GPU hardware rilevata");
    expect(componentSummary(inv, "storage")).toBe("1 volume");
  });

  it("summarises each component in English", () => {
    const inv = inventory({ volumes: [volume("C:\\", 1e12), volume("D:\\", 1e12)] });
    setLang("en");
    try {
      expect(componentSummary(inv, "cpu")).toBe("Unidentified CPU");
      expect(componentSummary(inv, "gpu")).toBe("No hardware GPU detected");
      expect(componentSummary(inv, "memory")).toMatch(/ installed$/);
      expect(componentSummary(inv, "storage")).toBe("2 volumes");
    } finally {
      setLang("it");
    }
  });
});
