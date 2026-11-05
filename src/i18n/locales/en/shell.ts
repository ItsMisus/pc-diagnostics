import type it from "../it/shell";
import type { Translation } from "../types";

/** English texts for the "shell" namespace: navigation and app frame. */
export default {
  navLabel: "Main navigation",
  expandSidebar: "Expand sidebar",
  collapseSidebar: "Collapse sidebar",
  expand: "Expand",
  collapse: "Collapse",
  section_overview: "Overview",
  section_hardware: "Hardware",
  section_benchmarks: "Benchmarks",
  section_processes: "Processes",
  section_drivers: "Drivers",
  section_diagnostics: "Diagnostics",
  section_history: "History",
  section_settings: "Settings",
  sectionDesc_overview: "System at a glance",
  sectionDesc_hardware: "Detected components and live metrics",
  sectionDesc_benchmarks: "Selective CPU, GPU, RAM and SSD tests",
  sectionDesc_processes: "Processes putting pressure on resources",
  sectionDesc_drivers: "Device and driver status (read-only)",
  sectionDesc_diagnostics: "Diagnostics based on verifiable rules",
  sectionDesc_history: "Benchmark history and comparison",
  sectionDesc_settings: "Application preferences",
} satisfies Translation<typeof it>;
