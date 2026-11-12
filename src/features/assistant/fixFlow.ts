import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";
import { tr } from "@/i18n";
import { getPlatform } from "@/lib/platform";
import { commands } from "@/services/commands";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { AssistantStatus } from "@/types/generated/AssistantStatus";
import type { FixPlan } from "@/types/generated/FixPlan";
import type { FixPreview } from "@/types/generated/FixPreview";
import type { StepOutcome } from "@/types/generated/StepOutcome";
import type { Platform } from "@/types/generated/Platform";
import { actionRunsOn, assistantReadiness, isPreviewExpired, type AssistantReadiness } from "./model";

/**
 * State machine of the "Risolvi" dialog. Nothing leaves the machine before `send()`;
 * every executable step needs `askConfirm` then `execute`; only one step runs at a time.
 * The API key never passes through here.
 */

export type StepRun =
  | { status: "idle" }
  | { status: "confirming" }
  | { status: "running" }
  | { status: "done"; outcome: StepOutcome }
  | { status: "error"; error: AppError };

export type ErrorStage = "status" | "preview" | "request";

export type FixPhase =
  | { kind: "closed" }
  | { kind: "checking" }
  | { kind: "notReady"; reason: Exclude<AssistantReadiness, "ready"> }
  | { kind: "previewLoading" }
  /** `refreshed`: the previous preview had expired and was rebuilt; the user must look again. */
  | { kind: "preview"; preview: FixPreview; refreshed: boolean }
  | { kind: "requesting"; preview: FixPreview }
  | { kind: "plan"; plan: FixPlan; steps: Record<string, StepRun>; manualDone: Record<string, boolean> }
  /** `reportChanged`: the latest report no longer has this finding at that index; retrying cannot help. */
  | { kind: "error"; stage: ErrorStage; error: AppError; reportChanged: boolean };

export interface FixTarget {
  /** Index into the latest `DiagnosticReport.findings` (backend order, not display order). */
  findingIndex: number;
  ruleId: string;
  title: string;
}

export interface FixFlowState {
  phase: FixPhase;
  target: FixTarget | null;
  /** Set when the user follows "Apri Settings": the assistant card scrolls into view once. */
  settingsFocusRequested: boolean;
  start: (target: FixTarget) => void;
  close: () => void;
  send: () => void;
  retry: () => void;
  askConfirm: (stepId: string) => void;
  cancelConfirm: (stepId: string) => void;
  execute: (stepId: string) => void;
  toggleManualDone: (stepId: string) => void;
  requestSettingsFocus: () => void;
  consumeSettingsFocus: () => boolean;
}

export interface FixFlowDeps {
  getStatus: () => Promise<AssistantStatus>;
  previewFix: (findingIndex: number) => Promise<FixPreview>;
  requestFix: (token: string) => Promise<FixPlan>;
  executeStep: (sessionId: string, stepId: string) => Promise<StepOutcome>;
  now: () => number;
}

/** True while closing would lose something: a paid request in flight or a step running. Esc is ignored then. */
export function isFlowBusy(phase: FixPhase): boolean {
  if (phase.kind === "requesting") return true;
  return phase.kind === "plan" && Object.values(phase.steps).some((s) => s.status === "running");
}

/**
 * A step can still be executed: its action exists on this platform (`actionRunsOn`), it is not
 * done, not running, and no other step is running.
 */
export function canExecuteStep(phase: FixPhase, stepId: string, platform: Platform = getPlatform()): boolean {
  if (phase.kind !== "plan") return false;
  const step = phase.plan.steps.find((s) => s.id === stepId);
  if (!step?.action) return false;
  if (!actionRunsOn(step.action, platform)) return false;
  if (Object.values(phase.steps).some((s) => s.status === "running")) return false;
  const run = phase.steps[stepId] ?? { status: "idle" };
  return run.status === "idle" || run.status === "confirming" || (run.status === "error" && run.error.retriable);
}

function mismatchError(expected: string, got: string): AppError {
  return {
    code: "notFound",
    // Shown under "technical details"; the dialog has its own title and description.
    message: tr("assistant", "errorReportChangedDetail", { expected, got }),
    detail: null,
    retriable: false,
  };
}

export function createFixFlowStore(deps: FixFlowDeps): StoreApi<FixFlowState> {
  return createStore<FixFlowState>()((set, get) => {
    // Bumped on start/close: results of an abandoned request are ignored.
    let gen = 0;

    const loadPreview = async (my: number, refreshed: boolean) => {
      const target = get().target;
      if (!target) return;
      set({ phase: { kind: "previewLoading" } });
      try {
        const preview = await deps.previewFix(target.findingIndex);
        if (my !== gen) return;
        if (preview.ruleId !== target.ruleId) {
          set({
            phase: { kind: "error", stage: "preview", error: mismatchError(target.ruleId, preview.ruleId), reportChanged: true },
          });
          return;
        }
        set({ phase: { kind: "preview", preview, refreshed } });
      } catch (e) {
        if (my === gen) set({ phase: { kind: "error", stage: "preview", error: toAppError(e), reportChanged: false } });
      }
    };

    const begin = async (my: number) => {
      set({ phase: { kind: "checking" } });
      let status: AssistantStatus;
      try {
        status = await deps.getStatus();
      } catch (e) {
        if (my === gen) set({ phase: { kind: "error", stage: "status", error: toAppError(e), reportChanged: false } });
        return;
      }
      if (my !== gen) return;
      const readiness = assistantReadiness(status);
      if (readiness !== "ready") {
        set({ phase: { kind: "notReady", reason: readiness } });
        return;
      }
      await loadPreview(my, false);
    };

    const patchStep = (stepId: string, run: StepRun) => {
      const phase = get().phase;
      if (phase.kind !== "plan") return;
      set({ phase: { ...phase, steps: { ...phase.steps, [stepId]: run } } });
    };

    return {
      phase: { kind: "closed" },
      target: null,
      settingsFocusRequested: false,

      start: (target) => {
        if (isFlowBusy(get().phase)) return;
        gen += 1;
        set({ target });
        void begin(gen);
      },

      close: () => {
        gen += 1;
        set({ phase: { kind: "closed" }, target: null });
      },

      send: () => {
        const phase = get().phase;
        if (phase.kind !== "preview") return;
        const my = gen;
        if (isPreviewExpired(phase.preview.expiresAt, deps.now())) {
          void loadPreview(my, true);
          return;
        }
        set({ phase: { kind: "requesting", preview: phase.preview } });
        deps.requestFix(phase.preview.token).then(
          (plan) => {
            if (my !== gen) return;
            const steps: Record<string, StepRun> = {};
            for (const s of plan.steps) steps[s.id] = { status: "idle" };
            set({ phase: { kind: "plan", plan, steps, manualDone: {} } });
          },
          (e: unknown) => {
            if (my === gen) set({ phase: { kind: "error", stage: "request", error: toAppError(e), reportChanged: false } });
          },
        );
      },

      retry: () => {
        const phase = get().phase;
        if (phase.kind !== "error" || phase.reportChanged) return;
        // The preview token is one-shot: after a failed request a new preview (and a new
        // explicit "Invia") is needed.
        if (phase.stage === "status") void begin(gen);
        else void loadPreview(gen, false);
      },

      askConfirm: (stepId) => {
        if (!canExecuteStep(get().phase, stepId)) return;
        patchStep(stepId, { status: "confirming" });
      },

      cancelConfirm: (stepId) => {
        const phase = get().phase;
        if (phase.kind === "plan" && phase.steps[stepId]?.status === "confirming") patchStep(stepId, { status: "idle" });
      },

      execute: (stepId) => {
        const phase = get().phase;
        if (phase.kind !== "plan" || phase.steps[stepId]?.status !== "confirming") return;
        if (!canExecuteStep(phase, stepId)) return;
        const my = gen;
        const sessionId = phase.plan.sessionId;
        patchStep(stepId, { status: "running" });
        deps.executeStep(sessionId, stepId).then(
          (outcome) => {
            if (my === gen) patchStep(stepId, { status: "done", outcome });
          },
          (e: unknown) => {
            if (my === gen) patchStep(stepId, { status: "error", error: toAppError(e) });
          },
        );
      },

      toggleManualDone: (stepId) => {
        const phase = get().phase;
        if (phase.kind !== "plan") return;
        set({ phase: { ...phase, manualDone: { ...phase.manualDone, [stepId]: !phase.manualDone[stepId] } } });
      },

      requestSettingsFocus: () => set({ settingsFocusRequested: true }),
      consumeSettingsFocus: () => {
        const requested = get().settingsFocusRequested;
        if (requested) set({ settingsFocusRequested: false });
        return requested;
      },
    };
  });
}

export const fixFlowStore = createFixFlowStore({
  getStatus: commands.getAssistantStatus,
  previewFix: commands.previewFix,
  requestFix: commands.requestFix,
  executeStep: commands.executeFixStep,
  now: () => Date.now(),
});

export function useFixFlow<T>(selector: (state: FixFlowState) => T): T {
  return useStore(fixFlowStore, selector);
}
