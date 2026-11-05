import type it from "../it/overview";
import type { Translation } from "../types";

/** English texts for the "overview" namespace. */
export default {
  pageDescription: "System at a glance",
  monitoringCard: "Monitoring",
  recentTrend: "Recent activity",
  systemCard: "System",

  // Live cards.
  cpuCaption: "Total usage",
  gpuCaption: "Usage (busiest engine)",
  onlySoftwareAdapters: "Only software adapters are present.",
  ramCaption: "in use",
  ramCaptionOf: "in use of {total}",
  detectedAt: "detected at {time}",
  ssdCaption: "free on {mount} of {total}",
  noPhysicalDisks: "No physical disks reported",

  // System summary.
  notReported: "Not reported by the system",
  os: "Operating system",
  version: "Version",
  kernel: "Kernel",
  hostname: "Computer name",
  cpuArch: "CPU architecture",
  appBuild: "App build",
  uptime: "Uptime",
} satisfies Translation<typeof it>;
