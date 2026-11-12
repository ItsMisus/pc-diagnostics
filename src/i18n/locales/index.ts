import itCommon from "./it/common";
import itShell from "./it/shell";
import itOverview from "./it/overview";
import itHardware from "./it/hardware";
import itMonitoring from "./it/monitoring";
import itBenchmarks from "./it/benchmarks";
import itHistory from "./it/history";
import itProcesses from "./it/processes";
import itDrivers from "./it/drivers";
import itDiagnostics from "./it/diagnostics";
import itAssistant from "./it/assistant";
import itSettings from "./it/settings";
import itErrors from "./it/errors";
import itDevice3d from "./it/device3d";
import enCommon from "./en/common";
import enShell from "./en/shell";
import enOverview from "./en/overview";
import enHardware from "./en/hardware";
import enMonitoring from "./en/monitoring";
import enBenchmarks from "./en/benchmarks";
import enHistory from "./en/history";
import enProcesses from "./en/processes";
import enDrivers from "./en/drivers";
import enDiagnostics from "./en/diagnostics";
import enAssistant from "./en/assistant";
import enSettings from "./en/settings";
import enErrors from "./en/errors";
import enDevice3d from "./en/device3d";

/**
 * Every namespace of every language. One namespace per feature, so texts can be edited in
 * parallel without touching a shared file.
 */
export const dictionaries = {
  it: { common: itCommon, shell: itShell, overview: itOverview, hardware: itHardware, monitoring: itMonitoring, benchmarks: itBenchmarks, history: itHistory, processes: itProcesses, drivers: itDrivers, diagnostics: itDiagnostics, assistant: itAssistant, settings: itSettings, errors: itErrors, device3d: itDevice3d },
  en: { common: enCommon, shell: enShell, overview: enOverview, hardware: enHardware, monitoring: enMonitoring, benchmarks: enBenchmarks, history: enHistory, processes: enProcesses, drivers: enDrivers, diagnostics: enDiagnostics, assistant: enAssistant, settings: enSettings, errors: enErrors, device3d: enDevice3d },
} as const;

export type Namespace = keyof (typeof dictionaries)["it"];
