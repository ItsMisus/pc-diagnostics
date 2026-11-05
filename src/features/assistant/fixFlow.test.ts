import { describe, expect, it, vi } from "vitest";
import { setLang } from "@/i18n";
import type { AssistantStatus } from "@/types/generated/AssistantStatus";
import type { FixPlan } from "@/types/generated/FixPlan";
import type { FixPreview } from "@/types/generated/FixPreview";
import type { StepOutcome } from "@/types/generated/StepOutcome";
import { canExecuteStep, createFixFlowStore, isFlowBusy, type FixFlowDeps } from "./fixFlow";

const flush = () => new Promise((r) => setTimeout(r, 0));

const READY: AssistantStatus = { enabled: true, keyConfigured: true, model: "claude-sonnet-5-5", availableModels: [] };

const PREVIEW: FixPreview = {
  token: "tok",
  ruleId: "memory.pressure",
  findingTitle: "Memoria",
  model: "claude-sonnet-5-5",
  payload: "{}",
  payloadBytes: 2,
  destination: "Anthropic",
  estimatedCostUsd: 0.03,
  expiresAt: 10_000,
};

const PLAN: FixPlan = {
  sessionId: "s1",
  ruleId: "memory.pressure",
  model: "claude-sonnet-5-5",
  servedBy: "claude-sonnet-5-5",
  summary: "x",
  confidence: "medium",
  warnings: [],
  usage: { inputTokens: 1, outputTokens: 1 },
  createdAt: 0,
  steps: [
    { id: "a", title: "A", explanation: "", risk: "low", requiresAdmin: false, reversible: true, action: { kind: "emptyRecycleBin" }, manualInstructions: null, validationNote: null },
    { id: "b", title: "B", explanation: "", risk: "low", requiresAdmin: false, reversible: true, action: { kind: "openDeviceManager" }, manualInstructions: null, validationNote: null },
    { id: "m", title: "M", explanation: "", risk: "low", requiresAdmin: false, reversible: true, action: null, manualInstructions: "fai", validationNote: null },
  ],
};

const TARGET = { findingIndex: 2, ruleId: "memory.pressure", title: "Memoria" };

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

function setup(patch: Partial<FixFlowDeps> = {}) {
  const deps: FixFlowDeps = {
    getStatus: vi.fn(() => Promise.resolve(READY)),
    previewFix: vi.fn(() => Promise.resolve(PREVIEW)),
    requestFix: vi.fn(() => Promise.resolve(PLAN)),
    executeStep: vi.fn((sessionId: string, stepId: string): Promise<StepOutcome> =>
      Promise.resolve({ sessionId, stepId, ok: true, message: "ok", at: 0 }),
    ),
    now: () => 0,
    ...patch,
  };
  return { deps, store: createFixFlowStore(deps) };
}

describe("fix flow", () => {
  it("makes no preview request when the assistant is disabled or has no key", async () => {
    for (const status of [
      { ...READY, enabled: false },
      { ...READY, keyConfigured: false },
    ]) {
      const { deps, store } = setup({ getStatus: () => Promise.resolve(status) });
      store.getState().start(TARGET);
      await flush();
      expect(store.getState().phase.kind).toBe("notReady");
      expect(deps.previewFix).not.toHaveBeenCalled();
      expect(deps.requestFix).not.toHaveBeenCalled();
    }
  });

  it("shows the preview for the backend index and sends nothing until send()", async () => {
    const { deps, store } = setup();
    store.getState().start(TARGET);
    await flush();
    expect(deps.previewFix).toHaveBeenCalledWith(2);
    expect(store.getState().phase.kind).toBe("preview");
    expect(deps.requestFix).not.toHaveBeenCalled();
    store.getState().send();
    await flush();
    expect(deps.requestFix).toHaveBeenCalledWith("tok");
    expect(store.getState().phase.kind).toBe("plan");
  });

  it("rebuilds an expired preview instead of sending it", async () => {
    const { deps, store } = setup({ now: () => 20_000 });
    store.getState().start(TARGET);
    await flush();
    store.getState().send();
    await flush();
    expect(deps.requestFix).not.toHaveBeenCalled();
    const phase = store.getState().phase;
    expect(phase.kind === "preview" && phase.refreshed).toBe(true);
  });

  it("refuses a preview for a different finding (report changed) without retry", async () => {
    const { deps, store } = setup({ previewFix: () => Promise.resolve({ ...PREVIEW, ruleId: "other" }) });
    store.getState().start(TARGET);
    await flush();
    const phase = store.getState().phase;
    expect(phase.kind === "error" && phase.reportChanged).toBe(true);
    expect(phase.kind === "error" && phase.error.message).toBe(
      "Il report più recente è cambiato: attesa la regola memory.pressure, l'anteprima riguarda other",
    );
    store.getState().retry();
    expect(store.getState().phase.kind).toBe("error");
    expect(deps.requestFix).not.toHaveBeenCalled();
  });

  it("ignores the plan when the dialog was cancelled while waiting", async () => {
    const d = deferred<FixPlan>();
    const { store } = setup({ requestFix: () => d.promise });
    store.getState().start(TARGET);
    await flush();
    store.getState().send();
    expect(isFlowBusy(store.getState().phase)).toBe(true);
    store.getState().close();
    d.resolve(PLAN);
    await flush();
    expect(store.getState().phase.kind).toBe("closed");
  });

  it("needs a confirmation, runs one step at a time and never re-runs a finished step", async () => {
    const d = deferred<StepOutcome>();
    const executeStep = vi.fn(() => d.promise);
    const { store } = setup({ executeStep });
    store.getState().start(TARGET);
    await flush();
    store.getState().send();
    await flush();

    store.getState().execute("a"); // not confirmed yet
    expect(executeStep).not.toHaveBeenCalled();

    store.getState().askConfirm("a");
    store.getState().execute("a");
    let phase = store.getState().phase;
    expect(isFlowBusy(phase)).toBe(true);
    expect(canExecuteStep(phase, "b")).toBe(false);
    store.getState().askConfirm("b");
    phase = store.getState().phase;
    expect(phase.kind === "plan" && phase.steps.b?.status).toBe("idle");

    d.resolve({ sessionId: "s1", stepId: "a", ok: false, message: "Non riuscito", at: 1 });
    await flush();
    phase = store.getState().phase;
    expect(phase.kind === "plan" && phase.steps.a?.status).toBe("done");
    expect(canExecuteStep(phase, "a")).toBe(false);
    expect(canExecuteStep(phase, "b")).toBe(true);
    expect(canExecuteStep(phase, "m")).toBe(false); // manual step
    expect(executeStep).toHaveBeenCalledTimes(1);
    expect(executeStep).toHaveBeenCalledWith("s1", "a");
  });

  it("tracks manual steps locally", async () => {
    const { store } = setup();
    store.getState().start(TARGET);
    await flush();
    store.getState().send();
    await flush();
    store.getState().toggleManualDone("m");
    const phase = store.getState().phase;
    expect(phase.kind === "plan" && phase.manualDone.m).toBe(true);
  });

  it("asks for a fresh preview after a failed request", async () => {
    const { deps, store } = setup({
      requestFix: () => Promise.reject({ code: "network", message: "offline", detail: null, retriable: true }),
    });
    store.getState().start(TARGET);
    await flush();
    store.getState().send();
    await flush();
    const phase = store.getState().phase;
    expect(phase.kind === "error" && phase.stage).toBe("request");
    expect(phase.kind === "error" && phase.error.code).toBe("network");
    store.getState().retry();
    await flush();
    expect(store.getState().phase.kind).toBe("preview");
    expect(deps.previewFix).toHaveBeenCalledTimes(2);
  });
});

describe("fix flow in English", () => {
  it("explains a changed report in English", async () => {
    setLang("en");
    try {
      const { store } = setup({ previewFix: () => Promise.resolve({ ...PREVIEW, ruleId: "other" }) });
      store.getState().start(TARGET);
      await flush();
      const phase = store.getState().phase;
      expect(phase.kind === "error" && phase.error.message).toBe(
        "The latest report changed: expected rule memory.pressure, the preview is for other",
      );
    } finally {
      setLang("it");
    }
  });
});
