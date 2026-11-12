import type it from "../it/hardware";
import type { Translation } from "../types";

/** English texts for the "hardware" namespace (inventory and specs, no live values). */
export default {
  inventoryAt: "Inventory collected at {time}",
  refreshing: "Refreshing…",
  refresh: "Refresh inventory",
  refreshFailed: "Inventory refresh failed: showing the previous data",

  // Shared labels.
  model: "Model",
  manufacturer: "Manufacturer",
  capacity: "Capacity",

  // Page groups.
  componentsTitle: "Components",

  // Device identity (form factor and its evidence).
  deviceGroup: "Device",
  softwareGroup: "Software",
  formFactor_desktop: "Desktop PC",
  formFactor_laptop: "Laptop",
  formFactor_mini: "Mini PC",
  formFactor_allInOne: "All-in-one",
  formFactor_other: "Other device type",
  basis_productName: "Detected from the product name",
  basis_modelIdentifier: "Detected from the model identifier",
  basis_chassisType: "Detected from the chassis type",
  deviceUnknown: "Device type not determined",
  modelIdentifier: "Model identifier",
  chassisTypes: "Chassis type (SMBIOS)",
  internalBattery: "Internal battery",

  // System.
  systemTitle: "System",
  notReported: "Not reported by the system",
  os: "Operating system",
  version: "Version",
  kernel: "Kernel",
  hostname: "Computer name",
  appBuild: "App build",
  uptime: "Uptime",

  // CPU.
  architecture: "Architecture",
  physicalCores: "Physical cores",
  logicalThreads: "Logical threads",
  maxNominalFrequency: "Rated maximum frequency",

  // GPU.
  noGpuAdapters: "No graphics adapters detected",
  softwareAdapter: "Software adapter",
  virtualDisplay: "Virtual display (no telemetry)",
  dedicatedVram: "Dedicated VRAM",
  sharedMemory: "Shared memory",
  unifiedMemory: "None: unified memory",
  driverVersion: "Driver version",
  telemetry: "Telemetry",
  noProvider: "No provider",

  // Memory.
  memoryTitle: "Memory",
  totalInstalled: "Total installed",
  modules: "Modules",
  noModules: "No modules reported by the system",
  modulesCaption: "Installed memory modules",
  slot: "Slot",
  ratedSpeed: "Rated speed",
  configuredSpeed: "Configured speed",
  moduleDetails: "Module details",
  partNumber: "Part number",

  // Storage.
  storageTitle: "Storage",
  physicalDisks: "Physical disks",
  volumes: "Volumes",
  noDisks: "No physical disks reported by the system",
  bus: "Bus",
  media: "Media",
  firmware: "Firmware",
  drive: "Drive",
  "drive@macos": "Mount point",
  label: "Label",
  fileSystem: "File system",
  removable: "Removable",
  yes: "Yes",
  no: "No",
} satisfies Translation<typeof it>;
