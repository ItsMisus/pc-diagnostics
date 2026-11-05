// Test-only builders. Not imported by application code.
import type { DeviceRecord } from "@/types/generated/DeviceRecord";
import type { DriverInventory } from "@/types/generated/DriverInventory";
import type { DriverSummary } from "@/types/generated/DriverSummary";

export function device(name: string, overrides: Partial<DeviceRecord> = {}): DeviceRecord {
  return {
    instanceId: `PCI\\${name}`,
    name,
    manufacturer: "Contoso",
    className: "System",
    classGuid: null,
    category: "other",
    hardwareIds: [],
    driver: null,
    health: "working",
    problemCode: null,
    problemDescription: null,
    update: { kind: "notChecked" },
    ...overrides,
  };
}

export const EMPTY_SUMMARY: DriverSummary = {
  total: 0,
  working: 0,
  problems: 0,
  driverMissing: 0,
  disabled: 0,
  updatesAvailable: 0,
  notVerifiable: 0,
  notChecked: 0,
};

export function inventory(devices: DeviceRecord[] = [], collectedAt = 1): DriverInventory {
  return { collectedAt, devices, summary: { ...EMPTY_SUMMARY, total: devices.length }, updateCheck: null };
}
