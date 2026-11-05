import type { AppInfo } from "@/types/generated/AppInfo";
import type { AppOverhead } from "@/types/generated/AppOverhead";
import type { AppSettings } from "@/types/generated/AppSettings";
import type { Language } from "@/types/generated/Language";
import type { AssistantStatus } from "@/types/generated/AssistantStatus";
import type { BenchmarkCapability } from "@/types/generated/BenchmarkCapability";
import type { BenchmarkComparison } from "@/types/generated/BenchmarkComparison";
import type { BenchmarkKind } from "@/types/generated/BenchmarkKind";
import type { BenchmarkProgress } from "@/types/generated/BenchmarkProgress";
import type { BenchmarkRequest } from "@/types/generated/BenchmarkRequest";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";
import type { ExportedReport } from "@/types/generated/ExportedReport";
import type { FixPlan } from "@/types/generated/FixPlan";
import type { FixPreview } from "@/types/generated/FixPreview";
import type { StepOutcome } from "@/types/generated/StepOutcome";
import type { ReportFormat } from "@/types/generated/ReportFormat";
import type { DriverInventory } from "@/types/generated/DriverInventory";
import type { HardwareInventory } from "@/types/generated/HardwareInventory";
import type { LiveSample } from "@/types/generated/LiveSample";
import type { MonitorStatus } from "@/types/generated/MonitorStatus";
import type { ProcessHistory } from "@/types/generated/ProcessHistory";
import type { ProcessSnapshot } from "@/types/generated/ProcessSnapshot";
import type { SystemInfo } from "@/types/generated/SystemInfo";
import { call } from "./ipc";

export const commands = {
  getAppInfo: () => call<AppInfo>("get_app_info"),
  getSystemInfo: () => call<SystemInfo>("get_system_info"),
  getSettings: () => call<AppSettings>("get_settings"),
  updateSettings: (settings: AppSettings) => call<AppSettings>("update_settings", { settings }),
  /** Effective language (preference with "system" resolved against the Windows display language). */
  getLanguage: () => call<Language>("get_language"),
  /** Cached by the backend; `refresh: true` forces a new (slow, WMI-backed) collection. */
  getHardwareInventory: (refresh = false) => call<HardwareInventory>("get_hardware_inventory", { refresh }),
  /** Ring-buffer contents, oldest first. Used to seed charts when a page mounts. */
  getLiveHistory: () => call<LiveSample[]>("get_live_history"),
  getMonitorStatus: () => call<MonitorStatus>("get_monitor_status"),
  getAppOverhead: () => call<AppOverhead>("get_app_overhead"),
  /** Latest ranking; null-free: rejects with notFound until the first snapshot exists. */
  getProcessSnapshot: () => call<ProcessSnapshot>("get_process_snapshot"),
  /** Recent history of one process instance (`ProcessRow.key`). Rejects with notFound if unknown. */
  getProcessHistory: (key: string) => call<ProcessHistory>("get_process_history", { key }),
  /** Cached device/driver inventory; `refresh: true` re-enumerates devices (no network). */
  getDriverInventory: (refresh = false) => call<DriverInventory>("get_driver_inventory", { refresh }),
  /**
   * Searches Windows Update for driver updates (network, can take a minute or more).
   * Rejects with consentRequired when online checks are disabled, busy if one is running.
   */
  checkDriverUpdates: () => call<DriverInventory>("check_driver_updates"),
  getBenchmarkCapabilities: () => call<BenchmarkCapability[]>("get_benchmark_capabilities"),
  /** Starts one benchmark; resolves with its run id. Rejects `busy` if one is active, `invalidInput` on bad params. */
  startBenchmark: (request: BenchmarkRequest) => call<string>("start_benchmark", { request }),
  /** Cooperative cancel; the final result still arrives via the finished event (state `cancelled`). */
  cancelBenchmark: (runId: string) => call<void>("cancel_benchmark", { runId }),
  /** Progress of the active run, or null when idle. */
  getBenchmarkStatus: () => call<BenchmarkProgress | null>("get_benchmark_status"),
  /** Stored results, newest first; `kind` null = all kinds. */
  getBenchmarkHistory: (kind: BenchmarkKind | null = null, limit = 50) =>
    call<BenchmarkResult[]>("get_benchmark_history", { kind, limit }),
  /** Compares two stored runs; `previousRunId` null = latest earlier comparable run. Rejects notFound. */
  compareBenchmarks: (currentRunId: string, previousRunId: string | null = null) =>
    call<BenchmarkComparison>("compare_benchmarks", { currentRunId, previousRunId }),
  /** Runs the deterministic rule engine over current data; stored as the latest report. */
  runDiagnostics: () => call<DiagnosticReport>("run_diagnostics"),
  /** Latest stored report, or null if diagnostics never ran. */
  getLatestDiagnostics: () => call<DiagnosticReport | null>("get_latest_diagnostics"),
  /** Writes the latest report into Documents/PC Diagnostics. `redact` removes personal data. */
  exportReport: (format: ReportFormat, redact: boolean) => call<ExportedReport>("export_report", { format, redact }),

  // "Risolvi" assistant. The API key is write-only: it goes to the backend (Windows
  // Credential Manager) and is never returned, stored or logged by the frontend.
  getAssistantStatus: () => call<AssistantStatus>("get_assistant_status"),
  setAssistantApiKey: (key: string) => call<AssistantStatus>("set_assistant_api_key", { key }),
  clearAssistantApiKey: () => call<AssistantStatus>("clear_assistant_api_key"),
  /** Builds the redacted payload for one finding (index into the latest report's `findings`). Sends nothing. */
  previewFix: (findingIndex: number) => call<FixPreview>("preview_fix", { findingIndex }),
  /** Sends exactly the previewed payload (one-shot token) to the model; can take 30–60 s. */
  requestFix: (token: string) => call<FixPlan>("request_fix", { token }),
  /** Runs one validated, whitelisted step of a plan. */
  executeFixStep: (sessionId: string, stepId: string) => call<StepOutcome>("execute_fix_step", { sessionId, stepId }),
};
