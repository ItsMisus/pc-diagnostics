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
} satisfies Translation<typeof it>;
