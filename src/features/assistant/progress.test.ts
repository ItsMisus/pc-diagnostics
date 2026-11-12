import { describe, expect, it } from "vitest";
import type { AppError } from "@/types/generated/AppError";
import type { FixPlan } from "@/types/generated/FixPlan";
import type { FixPreview } from "@/types/generated/FixPreview";
import type { FixStep } from "@/types/generated/FixStep";
import type { FixPhase } from "./fixFlow";
import { activeStage, FLOW_STAGES, planProgress, stageStates } from "./progress";

const PREVIEW: FixPreview = {
  token: "tok",
  ruleId: "r",
  findingTitle: "t",
  model: "claude-sonnet-5-5",
  payload: "{}",
  payloadBytes: 2,
  destination: "Anthropic",
  estimatedCostUsd: 0.03,
  expiresAt: 10_000,
};

const ERR: AppError = { code: "network", message: "m", detail: null, retriable: true };

const step = (id: string, action: FixStep["action"]): FixStep => ({
  id,
  title: id,
  explanation: "",
  risk: "low",
  requiresAdmin: false,
  reversible: true,
  action,
  manualInstructions: action ? null : "fai",
  validationNote: null,
});

const PLAN: Pick<FixPlan, "steps"> = {
  steps: [step("bin", { kind: "emptyRecycleBin" }), step("dm", { kind: "openDeviceManager" }), step("m", null)],
};

describe("dialog stages", () => {
  it("has four stages in order", () => {
    expect(FLOW_STAGES).toEqual(["check", "preview", "request", "steps"]);
  });

  it("maps each phase to its stage", () => {
    const cases: [FixPhase, number, string][] = [
      [{ kind: "checking" }, 0, "current"],
      [{ kind: "notReady", reason: "noKey" }, 0, "blocked"],
      [{ kind: "previewLoading" }, 1, "current"],
      [{ kind: "preview", preview: PREVIEW, refreshed: false }, 1, "current"],
      [{ kind: "requesting", preview: PREVIEW }, 2, "current"],
      [{ kind: "plan", plan: { ...PLAN } as FixPlan, steps: {}, manualDone: {} }, 3, "current"],
      [{ kind: "error", stage: "status", error: ERR, reportChanged: false }, 0, "failed"],
      [{ kind: "error", stage: "preview", error: ERR, reportChanged: true }, 1, "failed"],
      [{ kind: "error", stage: "request", error: ERR, reportChanged: false }, 2, "failed"],
    ];
    for (const [phase, index, state] of cases) expect(activeStage(phase)).toEqual({ index, state });
    expect(activeStage({ kind: "closed" })).toBeNull();
  });

  it("treats a refused consent as the setup stage, like the dialog does", () => {
    const phase: FixPhase = { kind: "error", stage: "request", error: { ...ERR, code: "consentRequired" }, reportChanged: false };
    expect(activeStage(phase)).toEqual({ index: 0, state: "blocked" });
  });

  it("marks earlier stages done and later ones upcoming", () => {
    expect(stageStates({ kind: "requesting", preview: PREVIEW })).toEqual(["done", "done", "current", "upcoming"]);
    expect(stageStates({ kind: "error", stage: "preview", error: ERR, reportChanged: false })).toEqual([
      "done",
      "failed",
      "upcoming",
      "upcoming",
    ]);
    expect(stageStates({ kind: "closed" })).toEqual([]);
  });
});

describe("plan progress", () => {
  it("counts only the steps that can be carried out on this platform", () => {
    expect(planProgress(PLAN, {}, {}, "macos")).toEqual({ total: 2, done: 0, failed: 0 });
    expect(planProgress(PLAN, {}, {}, "windows")).toEqual({ total: 3, done: 0, failed: 0 });
  });

  it("counts successful runs and ticked manual steps as done, failures apart", () => {
    const ok = { sessionId: "s", stepId: "bin", ok: true, message: "", at: 0 };
    expect(planProgress(PLAN, { bin: { status: "done", outcome: ok } }, { m: true }, "macos")).toEqual({
      total: 2,
      done: 2,
      failed: 0,
    });
    expect(planProgress(PLAN, { bin: { status: "done", outcome: { ...ok, ok: false } } }, {}, "macos")).toEqual({
      total: 2,
      done: 0,
      failed: 1,
    });
    expect(planProgress(PLAN, { bin: { status: "error", error: ERR }, dm: { status: "running" } }, { m: false }, "windows")).toEqual({
      total: 3,
      done: 0,
      failed: 1,
    });
  });
});
