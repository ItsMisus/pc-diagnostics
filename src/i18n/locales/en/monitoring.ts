import type it from "../it/monitoring";
import type { Translation } from "../types";

/** English texts for the "monitoring" namespace: live data, inventory, shared metric labels. */
export default {
  // Shared metric labels (Overview, Hardware).
  temperature: "Temperature",
  coreClock: "Core clock",
  power: "Power",
  fan: "Fan",
  available: "Available",
  read: "Read",
  write: "Write",
  cpuUsage: "CPU usage",
  gpuUsage: "GPU usage",
  memoryInUse: "Memory in use",
  notAvailableText: "not available",
  usedSpaceOn: "Used space on {mount}",
  percentUsed: "{percent} used",
  windowsHealth: "Windows status",
  diskFallback: "Disk {id}",
  noVolumes: "No volumes detected",
  noHardwareGpu: "No hardware GPU detected",
  noSampleForAdapter: "No sample for this adapter",

  // GPU power.
  percentOfMax: "{value} of max",

  // Labels for inventory enums.
  vendorOther: "Other (0x{id})",
  busVirtual: "Virtual",
  busOther: "Other",
  mediaUnknown: "Unknown",
  health_healthy: "Healthy",
  health_warning: "Warning",
  health_unhealthy: "Unhealthy",
  health_unknown: "Unknown",

  // Live charts.
  percentChartAria: "{label} over time, fixed 0–100% scale",
  diskThroughputTitle: "Disk: read and write",
  throughputAria: "Disk read and write speed over time, zero-based scale",
  legend: "Legend",

  // Live connection and inventory states.
  liveUnsupportedDesc: "Live monitoring requires the desktop app. No demo data is shown.",
  liveConnecting: "Connecting to monitoring…",
  liveUnsupported: "Monitoring not supported",
  livePermission: "Monitoring not allowed",
  liveUnavailable: "Monitoring unavailable",
  waitingFirstSample: "Waiting for the first sample…",
  badgeLive: "Live",
  badgeConnecting: "Connecting…",
  badgeStopped: "Monitoring stopped",
  detectingHardware: "Detecting hardware…",
  inventoryUnsupported: "Inventory not supported",
  inventoryUnsupportedDesc: "The hardware inventory requires the desktop app. No demo data is shown.",
} satisfies Translation<typeof it>;
