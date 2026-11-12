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
  memoryPressure: "Memory pressure",
  pressureNormal: "Normal",
  swapUsed: "Swap used",
  diskTitle: "Disk",
  activeTime: "Active time",
  storageTitle: "Storage",
  detectedAt: "detected at {time}",
  healthNote: "Windows Storage summary, not a full evaluation of SMART data.",
  "healthNote@macos": "From the SMART log of NVMe disks (critical warnings, spare, wear), not a full evaluation.",
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

  // Hero: device identity.
  heroLabel: "Your computer",
  thisComputer: "This computer",
  identityCpu: "Processor",
  identityGpu: "Graphics",
  identityMemory: "Memory",
  identitySystem: "System",
  cores: "{count} cores",
  coresThreads: "{physical} cores, {logical} threads",

  // Hero: overall state, from the latest diagnosis (no score).
  stateEyebrow: "Status",
  stateClear: "No problems found",
  stateFrom: "From the latest diagnosis, {date}",
  stateNotAScore: "This is the result of the latest diagnosis, not a health score.",
  stateNeverRunHint: "Run it to find out whether anything needs fixing.",
  stateLoading: "Reading the status…",
  stateUnavailable: "Status unavailable",
  runBenchmark: "Run a benchmark",
  benchmarkRunning: "Benchmark running",
  quickActions: "Quick actions",

  // Hero: device model shown by the 3D stage.
  shapeTitle: "Device model",
  shape_tower: "Desktop",
  shape_laptop: "Laptop",
  shape_mini: "Mini PC",
  shapeAuto: "Automatic",
  shapeDetected_productName: "Detected from the product name",
  shapeDetected_modelIdentifier: "Detected from the model identifier",
  shapeDetected_chassisType: "Detected from the chassis type",
  shapeDetected_unknownBasis: "Detected automatically",
  shapeOverride: "Chosen by you",
  shapePending: "Detecting the model…",
  shapeUnknown: "Model not detected",
  shapeChange: "Change",
  shapeChangeAria: "Change the device model",
  shapeReset: "Use detection",
  shapeChooseTitle: "What kind of computer is this?",
  shapeChooseHint: "It could not be detected reliably. Choose the model to show: you can change it at any time.",
  shapeNoModel_allInOne: "This is an all-in-one computer, which has no dedicated model: choose the closest one.",
  shapeNoModel_other: "There is no dedicated model for this kind of computer: choose the closest one.",
  shapeChooseOption: "Show as {shape}",
  shapeSaveFailed: "Choice not saved",
  shapeEvidenceProduct: "Product: {value}",
  shapeEvidenceModel: "Identifier: {value}",
  shapeEvidenceChassis: "SMBIOS chassis type: {value}",

  // Chips around the device (live values).
  chipCpu: "CPU",
  chipRam: "RAM",
  chipGpu: "GPU",
  chipTemp: "CPU temp.",
  chipAria: "{label}: {value}",

  // Essential metrics.
  essentialsTitle: "Resources",
  sparkAria: "{label}, last minute",
  cpuLoad: "CPU load",
  gpuLoad: "GPU load",
  memoryUsed: "Memory in use",
  storageFree: "Free space",
  freeOf: "free of {total}",
  temperatureValue: "Temperature {value}",
  noGpu: "No GPU",

  // Groups.
  findingsTitle: "Findings",
  detailsTitle: "Details",
  findingsNeverRun: "Findings appear after the first diagnosis.",
} satisfies Translation<typeof it>;
