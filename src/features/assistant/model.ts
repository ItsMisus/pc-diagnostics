import type { Tone } from "@/components/common/StatusBadge";
import { localeTag, translatorFor, type Translator } from "@/i18n";
import type { AssistantStatus } from "@/types/generated/AssistantStatus";
import type { Confidence } from "@/types/generated/Confidence";
import type { Finding } from "@/types/generated/Finding";
import type { RemediationAction } from "@/types/generated/RemediationAction";
import type { SettingsPage } from "@/types/generated/SettingsPage";
import type { StepRisk } from "@/types/generated/StepRisk";
import type { TokenUsage } from "@/types/generated/TokenUsage";

/**
 * Pure helpers for the "Risolvi" assistant. Every text the user reads before consenting
 * to an action comes from here, so it is unit-tested. Texts come from the "assistant"
 * namespace, looked up at call time in the current language; components that render them
 * also call `useT("assistant")`, so they re-render on a language change.
 */

const at = () => translatorFor("assistant");
type AssistantKey = Parameters<Translator<"assistant">>[0];

// ---------------------------------------------------------------------------
// When the button is shown

/** "Risolvi" is offered only for findings that ask for attention (warning or critical). */
export function canShowRisolvi(finding: Pick<Finding, "severity">): boolean {
  return finding.severity === "warning" || finding.severity === "critical";
}

/**
 * What happens when "Risolvi" is clicked. `ready` = the flow can start (preview first);
 * otherwise the dialog only explains how to enable it and makes no request.
 */
export type AssistantReadiness = "ready" | "disabled" | "noKey";

export function assistantReadiness(status: Pick<AssistantStatus, "enabled" | "keyConfigured">): AssistantReadiness {
  if (!status.enabled) return "disabled";
  if (!status.keyConfigured) return "noKey";
  return "ready";
}

export interface NotReadyText {
  title: string;
  explanation: string;
  steps: string[];
}

export function notReadyText(reason: Exclude<AssistantReadiness, "ready">): NotReadyText {
  const t = at();
  const steps = [t("setupStepOpenSettings"), t("setupStepEnable"), t("setupStepKey")];
  switch (reason) {
    case "disabled":
      return { title: t("disabledTitle"), explanation: t("disabledExplanation"), steps };
    case "noKey":
      return { title: t("noKeyTitle"), explanation: t("noKeyExplanation"), steps: steps.filter((_, i) => i !== 1) };
  }
}

// ---------------------------------------------------------------------------
// Models

interface ModelInfo {
  /** Product name: never translated. */
  label: string;
  /** Key of the one-line hint (translated at call time). */
  hint: AssistantKey;
  recommended: boolean;
}

const KNOWN_MODELS: Readonly<Record<string, ModelInfo>> = {
  "claude-sonnet-5-5": { label: "Claude Sonnet 5.5", hint: "modelHintSonnet55", recommended: true },
  "claude-opus-5-5": { label: "Claude Opus 5.5", hint: "modelHintOpus55", recommended: false },
  "claude-opus-4-5": { label: "Claude Opus 4.5", hint: "modelHintOpus45", recommended: false },
  "claude-haiku-5-5": { label: "Claude Haiku 5.5", hint: "modelHintHaiku55", recommended: false },
};

const MODEL_ID = /^claude-([a-z]+)-(\d+)(?:-(\d{1,2}))?(?:-\d{8})?$/;

/** Display name of a model id. Unknown `claude-<family>-<major>[-<minor>]` ids are prettified; anything else is shown as is. */
export function modelLabel(id: string): string {
  const known = KNOWN_MODELS[id];
  if (known) return known.label;
  const m = MODEL_ID.exec(id);
  if (!m || !m[1] || !m[2]) return id;
  const family = m[1].charAt(0).toUpperCase() + m[1].slice(1);
  return `Claude ${family} ${m[2]}${m[3] ? `.${m[3]}` : ""}`;
}

/** Label in the Settings selector: the recommended model is marked. */
export function modelOptionLabel(id: string): string {
  return KNOWN_MODELS[id]?.recommended ? at()("modelRecommended", { model: modelLabel(id) }) : modelLabel(id);
}

/** One-line hint under the selector, empty for unknown models. */
export function modelHint(id: string): string {
  const known = KNOWN_MODELS[id];
  return known ? at()(known.hint) : "";
}

// ---------------------------------------------------------------------------
// Plan

export function stepRiskLabel(r: StepRisk): string {
  return at()(`risk_${r}`);
}

export function stepRiskTone(r: StepRisk): Tone {
  switch (r) {
    case "low":
      return "neutral";
    case "medium":
      return "warn";
    case "high":
      return "bad";
  }
}

/** The model's own confidence in the plan, in plain words (never stronger than what it said). */
export function planConfidenceLabel(c: Confidence): string {
  return at()(`planConfidence_${c}`);
}

export function planConfidenceShort(c: Confidence): string {
  return at()(`planConfidenceShort_${c}`);
}

// ---------------------------------------------------------------------------
// Actions

export function settingsPageLabel(p: SettingsPage): string {
  return at()(`settingsPage_${p}`);
}

/** Exactly what the app will do for an action, shown in the confirmation before running it. */
export function describeAction(action: RemediationAction): string {
  const t = at();
  switch (action.kind) {
    case "openSettingsPage":
      return t("action_openSettingsPage", { page: settingsPageLabel(action.page) });
    case "openDeviceManager":
      return t("action_openDeviceManager");
    case "openDiskCleanup":
      return t("action_openDiskCleanup", { drive: action.drive });
    case "emptyRecycleBin":
      return t("action_emptyRecycleBin");
    case "cleanTempFiles":
      return t("action_cleanTempFiles", { days: t.plural("days", action.olderThanDays) });
    case "closeProgram":
      return t("action_closeProgram", { name: action.name });
    case "runSystemFileCheck":
      return t("action_runSystemFileCheck");
  }
}

/**
 * True when `describeAction` already says that the action cannot be undone, so the
 * confirmation does not repeat it. Decided by kind, not by matching the text, so it holds
 * in every language.
 */
export function actionStatesIrreversible(action: RemediationAction): boolean {
  switch (action.kind) {
    case "emptyRecycleBin":
    case "cleanTempFiles":
      return true;
    case "openSettingsPage":
    case "openDeviceManager":
    case "openDiskCleanup":
    case "closeProgram":
    case "runSystemFileCheck":
      return false;
  }
}

/** Short technical identification of an action for the "Avanzata" view. */
export function actionTechnicalLabel(action: RemediationAction): string {
  switch (action.kind) {
    case "openSettingsPage":
      return `openSettingsPage · ${action.page}`;
    case "openDiskCleanup":
      return `openDiskCleanup · ${action.drive}`;
    case "cleanTempFiles":
      return `cleanTempFiles · olderThanDays=${action.olderThanDays}`;
    case "closeProgram":
      return `closeProgram · ${action.name} (PID ${action.pid})`;
    case "openDeviceManager":
    case "emptyRecycleBin":
    case "runSystemFileCheck":
      return action.kind;
  }
}

// ---------------------------------------------------------------------------
// Numbers

/**
 * Rough cost estimate in USD, in the current language's style ("≈ 0,03 $" / "≈ $0.03").
 * Always presented as a "stima".
 */
export function formatCostEstimate(usd: number): string {
  if (!Number.isFinite(usd) || usd < 0) return "—";
  const t = at();
  if (usd === 0) return t("costZero");
  if (usd < 0.01) return t("costBelowCent");
  const amount = new Intl.NumberFormat(localeTag(), {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
    useGrouping: false,
  }).format(usd);
  return t("costApprox", { amount });
}

export function formatTokenUsage(u: TokenUsage): string {
  const int = new Intl.NumberFormat(localeTag(), { maximumFractionDigits: 0 });
  return at()("tokenUsage", { input: int.format(u.inputTokens), output: int.format(u.outputTokens) });
}

/** True when the one-shot preview token can no longer be used. */
export function isPreviewExpired(expiresAt: number, now: number): boolean {
  return Number.isFinite(expiresAt) && now >= expiresAt;
}
