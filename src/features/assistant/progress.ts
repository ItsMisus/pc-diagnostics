import type { FixPlan } from "@/types/generated/FixPlan";
import type { Platform } from "@/types/generated/Platform";
import type { FixPhase, StepRun } from "./fixFlow";
import { actionRunsOn } from "./model";

/**
 * Progress of the "Risolvi" dialog, derived from the flow's phase. Pure, unit-tested.
 * Four stages, always in this order: check the setup, review what would be sent, wait for
 * the proposal, carry out the steps.
 */
export const FLOW_STAGES = ["check", "preview", "request", "steps"] as const;
export type FlowStage = (typeof FLOW_STAGES)[number];

/**
 * - `done`: passed. `current`: in progress or waiting for the user. `blocked`: the setup is
 *   missing (assistant off or no key); the user must act in Settings. `failed`: the stage's
 *   request failed. `upcoming`: not reached yet.
 */
export type StageState = "done" | "current" | "blocked" | "failed" | "upcoming";

const ERROR_STAGE: Record<Extract<FixPhase, { kind: "error" }>["stage"], number> = { status: 0, preview: 1, request: 2 };

/** Index of the active stage and its state, `null` while the dialog is closed. */
export function activeStage(phase: FixPhase): { index: number; state: Exclude<StageState, "done" | "upcoming"> } | null {
  switch (phase.kind) {
    case "closed":
      return null;
    case "checking":
      return { index: 0, state: "current" };
    case "notReady":
      return { index: 0, state: "blocked" };
    case "previewLoading":
    case "preview":
      return { index: 1, state: "current" };
    case "requesting":
      return { index: 2, state: "current" };
    case "plan":
      return { index: 3, state: "current" };
    case "error":
      // The dialog shows the setup explanation for a refused consent: same as `notReady`.
      if (phase.error.code === "consentRequired") return { index: 0, state: "blocked" };
      return { index: ERROR_STAGE[phase.stage], state: "failed" };
  }
}

/** The state of each of the four stages (empty while closed). */
export function stageStates(phase: FixPhase): StageState[] {
  const active = activeStage(phase);
  if (!active) return [];
  return FLOW_STAGES.map((_, i) => (i < active.index ? "done" : i === active.index ? active.state : "upcoming"));
}

export interface PlanProgress {
  /** Steps the user can complete here: executable on this platform, or manual. */
  total: number;
  done: number;
  failed: number;
}

/**
 * How far the user got through the proposed steps. Steps whose tool exists only on another
 * platform cannot be carried out here and are not counted. A manual step counts as done
 * once ticked "Fatto"; an executed step once it ran and reported success.
 */
export function planProgress(
  plan: Pick<FixPlan, "steps">,
  steps: Record<string, StepRun>,
  manualDone: Record<string, boolean>,
  platform: Platform,
): PlanProgress {
  let total = 0;
  let done = 0;
  let failed = 0;
  for (const step of plan.steps) {
    if (step.action && !actionRunsOn(step.action, platform)) continue;
    total += 1;
    if (!step.action) {
      if (manualDone[step.id] === true) done += 1;
      continue;
    }
    const run = steps[step.id];
    if (run?.status === "done") {
      if (run.outcome.ok) done += 1;
      else failed += 1;
    } else if (run?.status === "error") failed += 1;
  }
  return { total, done, failed };
}
