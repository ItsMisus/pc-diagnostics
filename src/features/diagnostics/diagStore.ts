import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";
import { commands } from "@/services/commands";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";
import type { ExportedReport } from "@/types/generated/ExportedReport";
import type { ReportFormat } from "@/types/generated/ReportFormat";

export type ExportState =
  | { status: "idle" }
  | { status: "running"; format: ReportFormat }
  | { status: "done"; format: ReportFormat; result: ExportedReport }
  | { status: "error"; format: ReportFormat; error: AppError };

export interface DiagState {
  /** Loading of the latest stored report. `ready` with `report: null` = never ran. */
  status: "idle" | "loading" | "ready" | "error";
  report: DiagnosticReport | null;
  error: AppError | null;
  running: boolean;
  runError: AppError | null;
  export: ExportState;
  ensureLoaded: () => void;
  reload: () => void;
  run: () => Promise<void>;
  exportReport: (format: ReportFormat, redact: boolean) => Promise<void>;
}

export interface DiagDeps {
  getLatest: () => Promise<DiagnosticReport | null>;
  run: () => Promise<DiagnosticReport>;
  exportReport: (format: ReportFormat, redact: boolean) => Promise<ExportedReport>;
}

export function createDiagStore(deps: DiagDeps): StoreApi<DiagState> {
  return createStore<DiagState>()((set, get) => {
    const load = async () => {
      set({ status: "loading", error: null });
      try {
        const report = await deps.getLatest();
        // A run that finished meanwhile may be newer than the stored report: keep the newest.
        const current = get().report;
        const newest = current && (!report || current.generatedAt >= report.generatedAt) ? current : report;
        set({ status: "ready", report: newest, error: null });
      } catch (e) {
        set({ status: "error", error: toAppError(e) });
      }
    };

    return {
      status: "idle",
      report: null,
      error: null,
      running: false,
      runError: null,
      export: { status: "idle" },
      ensureLoaded: () => {
        if (get().status === "idle") void load();
      },
      reload: () => {
        if (get().status !== "loading") void load();
      },
      run: async () => {
        if (get().running) return;
        set({ running: true, runError: null });
        try {
          const report = await deps.run();
          // A new report invalidates the previous export result.
          set({ running: false, status: "ready", report, error: null, export: { status: "idle" } });
        } catch (e) {
          set({ running: false, runError: toAppError(e) });
        }
      },
      exportReport: async (format, redact) => {
        if (get().export.status === "running") return;
        set({ export: { status: "running", format } });
        try {
          const result = await deps.exportReport(format, redact);
          set({ export: { status: "done", format, result } });
        } catch (e) {
          set({ export: { status: "error", format, error: toAppError(e) } });
        }
      },
    };
  });
}

export const diagStore = createDiagStore({
  getLatest: commands.getLatestDiagnostics,
  run: commands.runDiagnostics,
  exportReport: commands.exportReport,
});

export function useDiag<T>(selector: (state: DiagState) => T): T {
  return useStore(diagStore, selector);
}
