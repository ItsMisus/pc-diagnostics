import type it from "../it/history";
import type { Translation } from "../types";

/** English texts for the "history" namespace. Test kinds and states come from "benchmarks". */
export default {
  refresh: "Refresh",
  refreshFailed: "Refresh failed: showing the previous data",
  unavailable: "History not available",
  unsupportedDesc: "History requires the desktop app. No demo data is shown.",
  runs: "Runs",
  testType: "Test type",
  filterAll: "All",
  noRunsOfKind: "No runs of this type",
  limitNote: "Showing the latest {count} runs.",
  loading: "Reading history…",
  emptyTitle: "No saved benchmarks",
  emptyDesc: "Results appear here after each run.",
  goToBenchmarks: "Go to Benchmarks",
  tableCaption: "Saved runs",
  colDate: "Date",
  colTest: "Test",
  colState: "Status",
  colHeadline: "Main result",
  colDuration: "Duration",
  openDetail: ", open details",
  noResult: "no result",
  compareWith: "Compare with",
  compareAuto: "Previous comparable run (automatic)",
} satisfies Translation<typeof it>;
