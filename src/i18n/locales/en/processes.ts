import type it from "../it/processes";
import type { Translation } from "../types";

/** English texts for the "processes" namespace. */
export default {
  // ProcessesPage
  pageDescription: "Processes putting pressure on CPU, memory and I/O (read-only)",
  connectingTitle: "Connecting to process analysis…",
  waitingTitle: "Waiting for the first measurement…",
  waitingDescription:
    "The first ranking is available a few seconds after startup: a process's CPU usage is calculated by comparing two consecutive samples.",
  unsupportedTitle: "Process analysis not supported",
  unsupportedDescription: "Process analysis requires the desktop app. No demo data is shown.",
  notAllowedTitle: "Process analysis not allowed",
  unavailableTitle: "Process analysis unavailable",
  badgeLive: "Live",
  badgeConnecting: "Connecting…",
  badgeWaiting: "Waiting for the first sample",
  badgeUnsupported: "Not supported",
  badgeNotAllowed: "Not allowed",
  badgeError: "Analysis stopped",
  modeInstant: "Instant",
  modeWindow: "Interval (last {seconds} s)",
  rankingLabel: "Ranking",
  searchLabel: "Search processes by name",
  searchPlaceholder: "Search by name…",
  summary:
    "{total} active processes · {inaccessible} protected (path not readable; metrics still real) · showing the top {shown} by Impact Score (number configurable in {settings}) · updated at {time}",

  // ProcessTable
  protectedNote:
    "Protected process: the executable path can't be read without elevated privileges. The metrics come from the kernel snapshot and are real.",
  ioNote: "Logical I/O (files, network, devices): it does not match the physical bytes written to disk",
  colName: "Process name",
  selfTag: "this app",
  emptyRanking: "No processes ranked in this sample.",
  noMatch: "No process matches “{query}”.",
  tableCaption: "Processes sorted by {column}",
  rowShowDetails: "{name}, PID {pid}: show details",
  rowHideDetails: "{name}, PID {pid}: hide details",
  protectedShort: "protected",
  impactOf: "Impact Score of {name}",
  outOf100: "{value} out of 100",
  peak: "peak {value}",

  // rows.ts: process categories
  category_system: "System",
  category_service: "Service",
  category_application: "Application",
  category_background: "Background",
  category_unknown: "Unclassified",

  // ProcessDetail
  detailTitle: "Details",
  detailEmpty: "Select a process in the table to see its recent trend.",
  protectedDetail: "Protected process: executable path not readable without elevated privileges (real metrics from the kernel)",
  historyLoading: "Loading history…",
  endedTitle: "Process ended or no longer monitored",
  endedDescription: "A process's history is kept only for a short time after it closes.",
  memorySection: "Memory",
  privateMemory: "Private memory",
  exePathSection: "Executable path",
  localInfoAria: "Local information",
  exePathUnavailable: "Not available for this process",
  localInfoNote: "Local information, shown only here: it is not included in shareable reports.",
  pressureSection: "Resource pressure (0–100)",
  pressureAria: "Resource pressure",
  pressureOf: "{label} pressure",
  staleNote: "The process is no longer ranked: showing the last known values.",
  historyRunning: "Running",
  historyEnded: "Ended",
  noSamples: "No samples recorded for this process yet.",
  cpuChartTitle: "CPU (% of the whole system)",
  cpuChartAria: "Process CPU over time, fixed 0–100% scale",
  ramChartTitle: "RAM (working set)",
  ramChartAria: "Process memory over time, scale from zero",
  ioChartTitle: "Logical I/O: write (solid line) and read",
  ioChartAria: "Process logical I/O over time, scale from zero",
  seriesWrite: "Write",
  seriesRead: "Read",

  // ScoreExplanation
  scoreDisclaimer: "The score estimates pressure on resources; it does not prove that a process is slowing down the computer.",
  scoreTitle: "How the Impact Score is calculated",
  scoreIntro: "Each resource is normalized to a 0–100 scale and the score is the weighted average:",
  scoreCpu: "share of the whole system's capacity ({processors} logical processors); 100 means all processors are busy.",
  scoreRam: "process memory relative to total physical memory ({total}; available now {available}).",
  scoreIo:
    "logical I/O (files, network, devices) relative to a reference of {reference}, which corresponds to 100. It does not match the physical bytes written to disk.",
  scoreModes:
    "“Instant” uses the latest sample (every {interval} s). “Interval” uses the average of the last {window} s; the peak is the highest value seen in the same interval.",
  scoreMethodology:
    "Methodology v{version}: scores are comparable only between measurements with the same version. This page is read-only: it does not end, suspend or change any process.",
} satisfies Translation<typeof it>;
