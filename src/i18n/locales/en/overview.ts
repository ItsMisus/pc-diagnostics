import type it from "../it/overview";
import type { Translation } from "../types";

/** English texts for the "overview" namespace (the dashboard). */
export default {
  monitoringCard: "Monitoring",
  recentTrend: "Recent activity",

  // Panels.
  load: "Load",
  inUse: "In use",
  used: "Used",
  meterAria: "{component} {label}",
  ofTotal: "{used} of {total}",
  clock: "Clock",
  frequencyLimited: "Limited to {limit}",
  frequencyLimitBody:
    "Windows reports a limit of {limit} against a rated maximum of {max}. The system does not state the cause (for example power plan, temperature or power supply).",
  "frequencyLimitBody@macos":
    "macOS reports a limit of {limit} against a rated maximum of {max}. The system does not state the cause (for example temperature, power supply or Low Power Mode).",
  onlySoftwareAdapters: "Only software adapters are present.",
  committedMemory: "Committed memory",
  pagesFromDisk: "Pages read from disk",
  diskTitle: "Disk",
  activeTime: "Active time",
  storageTitle: "Storage",
  detectedAt: "detected at {time}",
  healthNote: "Windows Storage summary, not a full evaluation of SMART data.",
  "healthNote@macos": "Status reported by macOS, not a full evaluation of SMART data.",
  noPhysicalDisks: "No physical disks reported",

  // Status of a meter above its threshold (features/overview/levels.ts).
  levelWarn: "Warning",
  levelBad: "Critical",
  levelAria: "{value}, {level}",

  // Top processes.
  processesTitle: "Processes",
  openProcesses: "Open Processes",
  topProcessesCaption: "Processes with the highest impact",
  colName: "Name",
  ioNote: "Logical I/O (files, network, devices), not physical disk bytes",
  "ioNote@macos": "Disk I/O of the process (macOS does not count network and device I/O per process)",
  processesWaiting: "Waiting for process data…",
  processesUnavailable: "Process data unavailable",
  noProcesses: "No processes in the ranking",

  // Per logical processor.
  perCoreTitle: "Logical processors",
  perCoreUsage: "Usage per logical processor",
  logicalProcessor: "Logical processor {n}",
} satisfies Translation<typeof it>;
