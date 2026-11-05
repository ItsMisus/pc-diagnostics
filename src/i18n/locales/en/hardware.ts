import type it from "../it/hardware";
import type { Translation } from "../types";

/** English texts for the "hardware" namespace. */
export default {
  pageDescription: "Detected components and live metrics",
  inventoryAt: "Inventory collected at {time}",
  refreshing: "Refreshing…",
  refresh: "Refresh inventory",
  refreshFailed: "Inventory refresh failed: showing the previous data",
  liveUnavailable: "Live values unavailable.",

  // Shared section headings and labels.
  specs: "Specifications",
  live: "Live",
  model: "Model",
  manufacturer: "Manufacturer",
  usage: "Usage",
  totalUsage: "Total usage",
  inUse: "In use",
  ofTotal: "{used} of {total}",
  capacity: "Capacity",

  // CPU.
  architecture: "Architecture",
  physicalCores: "Physical cores",
  logicalThreads: "Logical threads",
  maxNominalFrequency: "Rated maximum frequency",
  perCoreUsage: "Usage per logical processor",
  currentFrequency: "Current frequency",
  frequencyLimit: "Frequency limit",
  frequencyLimitTitle: "Frequency limit reported by the operating system",
  frequencyLimitBody:
    "Windows reports a limit of {limit} against a rated maximum of {max}. The system does not say why (for example power plan, temperature or power supply), so this app cannot tell you the cause.",
  logicalProcessor: "Logical processor {n}",

  // GPU.
  noGpuAdapters: "No graphics adapters detected",
  softwareAdapter: "Software adapter",
  virtualDisplay: "Virtual display (no telemetry)",
  dedicatedVram: "Dedicated VRAM",
  sharedMemory: "Shared memory",
  driverVersion: "Driver version",
  telemetry: "Telemetry",
  noProvider: "No provider",
  noSample: "No sample for this adapter.",
  vramInUse: "VRAM in use",

  // Memory.
  totalInstalled: "Total installed",
  modules: "Modules",
  committedMemory: "Committed memory",
  pagesFromDisk: "Pages read from disk",
  noModules: "No modules reported by the system",
  modulesCaption: "Installed memory modules",
  slot: "Slot",
  ratedSpeed: "Rated speed",
  configuredSpeed: "Configured speed",
  partNumber: "Part number",

  // Storage.
  storageLive: "Live activity (all physical disks)",
  physicalDisks: "Physical disks",
  volumes: "Volumes",
  activeTime: "Active time",
  noDisks: "No physical disks reported by the system",
  bus: "Bus",
  media: "Media",
  firmware: "Firmware",
  healthNote: "“Windows status” is the Windows Storage summary, not a full evaluation of SMART data.",
  drive: "Drive",
  label: "Label",
  fileSystem: "File system",
  occupancy: "Usage",
  removable: "Removable",
  freeOfTotal: "{free} free of {total}",
  usedWithSummary: "{percent} used, {summary}",
  yes: "Yes",
  no: "No",
} satisfies Translation<typeof it>;
