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
  subtitle_overview: "Your computer at a glance: the device, resources in use and findings.",
  subtitle_hardware: "The computer's components, as the system reports them.",
  subtitle_benchmarks: "Measure the performance of the processor, graphics, memory and disk.",
  subtitle_processes: "The programs running and how much they weigh on the system.",
  subtitle_drivers: "Device drivers, with the ones that have problems first.",
  subtitle_diagnostics: "Looks for problems on the computer and explains what to do.",
  subtitle_history: "The results of the benchmarks run so far.",
  subtitle_settings: "Appearance, device, monitoring and privacy of the app.",
} satisfies Translation<typeof it>;
