import { m } from "motion/react";
import { Fragment, useId, useRef, type ReactNode } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Disclosure } from "@/components/common/Disclosure";
import { Icon, type IconName } from "@/components/common/Icon";
import { MODAL_BODY_CLASS, MODAL_FOOTER_CLASS, Modal } from "@/components/common/Modal";
import { Spinner, StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useIsSimple } from "@/hooks/useViewMode";
import { stateKindForError } from "@/lib/errors";
import { formatBytes } from "@/lib/format";
import { duration, ease } from "@/lib/motion";
import { usePlatform } from "@/lib/platform";
import { type Translator, useT } from "@/i18n";
import type { FixPlan } from "@/types/generated/FixPlan";
import type { FixPreview } from "@/types/generated/FixPreview";
import type { FixStep } from "@/types/generated/FixStep";
import { canExecuteStep, fixFlowStore, isFlowBusy, useFixFlow, type FixPhase, type StepRun } from "./fixFlow";
import {
  actionRunsOn,
  actionStatesIrreversible,
  actionTechnicalLabel,
  describeAction,
  formatCostEstimate,
  formatTokenUsage,
  modelLabel,
  notReadyText,
  planConfidenceLabel,
  planConfidenceShort,
  stepRiskLabel,
  stepRiskTone,
} from "./model";

/**
 * The single "Risolvi" dialog (mounted once in App), on the shared `Modal` (native modal
 * `<dialog>`: the rest of the window is inert, Tab wraps inside, focus returns to the
 * trigger). Esc closes only when nothing would be lost (no request in flight, no step
 * running); the close button is disabled only while a step runs.
 */
export function FixDialog() {
  const phase = useFixFlow((s) => s.phase);
  const target = useFixFlow((s) => s.target);
  const open = phase.kind !== "closed";
  // Keep the last open phase on screen while the close animation plays.
  const lastOpen = useRef<{ phase: FixPhase; target: typeof target }>({ phase, target });
  if (open) lastOpen.current = { phase, target };
  const shown = open ? { phase, target } : lastOpen.current;
  const busy = isFlowBusy(phase);
  const stepRunning = phase.kind === "plan" && busy;
  const t = useT("assistant");

  const close = () => fixFlowStore.getState().close();

  return (
    <Modal
      open={open}
      onClose={close}
      dismissible={!busy}
      closeDisabled={stepRunning}
      closeHint={t("waitForStep")}
      title={t("dialogTitle")}
      description={shown.target?.title}
      icon="sparkle"
      size="lg"
      bare
    >
      {shown.phase.kind !== "closed" && (
        <>
          {/* Each phase renders its own scrolling body and, when it has actions, a footer. */}
          <PhaseBody phase={shown.phase} onClose={close} />
          {shown.phase.kind === "plan" && <PlanFooter plan={shown.phase.plan} />}
        </>
      )}
    </Modal>
  );
}

/**
 * Phase changes fade the new content in (opacity only, short, nothing waits for it): the
 * fragment is keyed by phase, so body and footer remount together. Updates within a phase
 * (a step starting or finishing) keep the same key and do not fade.
 */
const phaseFade = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  transition: { duration: duration.component, ease: ease.out },
} as const;

function PhaseBody({ phase, onClose }: { phase: FixPhase; onClose: () => void }) {
  const t = useT("assistant");
  return <Fragment key={phase.kind}>{renderPhase(phase, onClose, t)}</Fragment>;
}

function renderPhase(phase: FixPhase, onClose: () => void, t: Translator<"assistant">): ReactNode {
  switch (phase.kind) {
    case "closed":
      return null;
    case "checking":
      return (
        <Body>
          <StateView kind="loading" title={t("checking")} />
        </Body>
      );
    case "previewLoading":
      return (
        <Body>
          <StateView kind="loading" title={t("preparing")} description={t("nothingSentYet")} />
        </Body>
      );
    case "notReady":
      return <NotReady reason={phase.reason} onClose={onClose} />;
    case "preview":
      return <PreviewView preview={phase.preview} refreshed={phase.refreshed} onClose={onClose} />;
    case "requesting":
      return <Requesting preview={phase.preview} onClose={onClose} />;
    case "plan":
      return <PlanView plan={phase.plan} steps={phase.steps} manualDone={phase.manualDone} phase={phase} />;
    case "error":
      return <ErrorView phase={phase} onClose={onClose} />;
  }
}

/** Scrolling content area of a phase. */
function Body({ children }: { children: ReactNode }) {
  return (
    <m.div {...phaseFade} className={MODAL_BODY_CLASS}>
      {children}
    </m.div>
  );
}

/** Dialog footer: actions on the right, at most one primary, always visible below the body. */
function Actions({ children }: { children: ReactNode }) {
  return (
    <m.div
      {...phaseFade}
      className={MODAL_FOOTER_CLASS}
    >
      {children}
    </m.div>
  );
}

/** Warning-toned note. Text first; the icon only reinforces it. */
function Callout({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <div role="note" className="mt-4 flex items-start gap-2 rounded-md border border-warn/25 bg-warn-soft px-3 py-2.5 text-[13px] text-fg">
      <Icon name="alert" size={14} className="mt-0.5 shrink-0 text-warn" />
      <div className="min-w-0 flex-1">
        {title && <p className="font-medium">{title}</p>}
        {children}
      </div>
    </div>
  );
}

function NotReady({ reason, onClose }: { reason: "disabled" | "noKey"; onClose: () => void }) {
  const t = useT("assistant");
  const text = notReadyText(reason);
  const setSection = useAppStore((s) => s.setSection);
  const openSettings = () => {
    fixFlowStore.getState().requestSettingsFocus();
    onClose();
    setSection("settings");
  };
  return (
    <>
      <Body>
        <div className="flex items-center gap-2">
          <Icon name="lock" className="shrink-0 text-fg-subtle" />
          <h3 className="text-sm font-medium">{text.title}</h3>
        </div>
        <p className="mt-1.5 max-w-prose text-sm text-fg-muted">{text.explanation}</p>
        <h3 className="mt-5 text-[13px] font-semibold">{t("howToEnable")}</h3>
        <ol className="mt-2 flex flex-col gap-2">
          {text.steps.map((s, i) => (
            <li key={s} className="flex items-start gap-2.5 text-sm">
              <span
                aria-hidden="true"
                className="mt-px grid size-5 shrink-0 place-items-center rounded-sm border border-line bg-surface-2 text-2xs font-medium text-fg-muted tabular"
              >
                {i + 1}
              </span>
              <span className="min-w-0">{s}</span>
            </li>
          ))}
        </ol>
      </Body>
      <Actions>
        <Button onClick={onClose}>{t("close")}</Button>
        <Button variant="primary" onClick={openSettings} autoFocus>
          <Icon name="settings" />
          {t("openSettings")}
        </Button>
      </Actions>
    </>
  );
}

function PreviewView({ preview, refreshed, onClose }: { preview: FixPreview; refreshed: boolean; onClose: () => void }) {
  const simple = useIsSimple();
  const t = useT("assistant");
  const payloadId = useId();
  const payload = (
    <pre
      id={payloadId}
      tabIndex={0}
      aria-label={t("exactDataAria")}
      className="max-h-64 overflow-auto rounded-md border border-line bg-canvas p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all text-fg"
    >
      {preview.payload}
    </pre>
  );
  return (
    <>
      <Body>
        <h3 className="text-[13px] font-semibold">{t("whatWillBeSent")}</h3>
        <p className="mt-1 max-w-prose text-sm text-fg-muted">{t("previewIntro")}</p>
        {refreshed && <Callout>{t("previewRefreshed")}</Callout>}
        <dl className="mt-4 grid grid-cols-[max-content_minmax(0,1fr)] gap-x-6 gap-y-2 rounded-md border border-line px-3 py-2.5 text-[13px]">
          <dt className="text-fg-muted">{t("destination")}</dt>
          <dd className="break-words">{preview.destination}</dd>
          <dt className="text-fg-muted">{t("model")}</dt>
          <dd>
            {modelLabel(preview.model)}
            {!simple && <span className="ml-2 font-mono text-2xs text-fg-subtle">{preview.model}</span>}
          </dd>
          <dt className="text-fg-muted">{t("estimatedCost")}</dt>
          <dd className="tabular">
            {formatCostEstimate(preview.estimatedCostUsd)} <span className="text-fg-muted">{t("costNote")}</span>
          </dd>
          <dt className="text-fg-muted">{t("payloadSize")}</dt>
          <dd className="tabular">{formatBytes(preview.payloadBytes)}</dd>
        </dl>
        <div className="mt-4">
          {simple ? (
            <Disclosure summary={t("showExactData")}>{payload}</Disclosure>
          ) : (
            <>
              <h3 className="mb-1.5 text-[13px] font-semibold">{t("exactData")}</h3>
              {payload}
            </>
          )}
        </div>
      </Body>
      <Actions>
        <Button onClick={onClose} autoFocus>
          {t("cancel")}
        </Button>
        <Button variant="primary" onClick={() => fixFlowStore.getState().send()}>
          {t("sendAndAsk")}
        </Button>
      </Actions>
    </>
  );
}

function Requesting({ preview, onClose }: { preview: FixPreview; onClose: () => void }) {
  const t = useT("assistant");
  return (
    <>
      <Body>
        <StateView
          kind="loading"
          title={t("requesting", { model: modelLabel(preview.model) })}
          description={t("requestingDescription")}
        />
      </Body>
      <Actions>
        <Button onClick={onClose} autoFocus>
          {t("cancel")}
        </Button>
      </Actions>
    </>
  );
}

function ErrorView({ phase, onClose }: { phase: Extract<FixPhase, { kind: "error" }>; onClose: () => void }) {
  const t = useT("assistant");
  // The backend refuses with consentRequired when the assistant was switched off meanwhile.
  if (phase.error.code === "consentRequired") return <NotReady reason="disabled" onClose={onClose} />;
  const title = phase.reportChanged
    ? t("errorReportChanged")
    : phase.stage === "status"
      ? t("errorStatus")
      : phase.stage === "preview"
        ? t("errorPreview")
        : t("errorRequest");
  const description = phase.reportChanged
    ? t("errorReportChangedDescription")
    : phase.stage === "request"
      ? t("errorRequestDescription")
      : undefined;
  return (
    <>
      <Body>
        <StateView
          kind={stateKindForError(phase.error)}
          title={title}
          {...(description ? { description } : {})}
          error={phase.error}
          {...(phase.reportChanged ? {} : { onRetry: () => fixFlowStore.getState().retry() })}
        />
      </Body>
      <Actions>
        <Button onClick={onClose} autoFocus>
          {t("close")}
        </Button>
      </Actions>
    </>
  );
}

function PlanView({
  plan,
  steps,
  manualDone,
  phase,
}: {
  plan: FixPlan;
  steps: Record<string, StepRun>;
  manualDone: Record<string, boolean>;
  phase: FixPhase;
}) {
  const stepsLabelId = useId();
  const t = useT("assistant");
  return (
    <Body>
      <h3 className="text-[13px] font-semibold">{t("proposal")}</h3>
      <p className="mt-1.5 text-sm">{plan.summary}</p>
      <p className="mt-2.5 flex flex-wrap items-center gap-2 text-xs text-fg-muted">
        <StatusBadge tone="neutral" label={planConfidenceShort(plan.confidence)} />
        {planConfidenceLabel(plan.confidence)}
      </p>

      {plan.warnings.length > 0 && (
        <Callout title={t("warnings")}>
          <ul className="mt-1 flex list-disc flex-col gap-0.5 pl-4 marker:text-fg-subtle">
            {plan.warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
        </Callout>
      )}

      {plan.steps.length === 0 ? (
        <p className="mt-5 text-sm text-fg-muted">{t("noSteps")}</p>
      ) : (
        <>
          <h3 id={stepsLabelId} className="mt-5 text-[13px] font-semibold">
            {t("proposedSteps")}
          </h3>
          <ol className="mt-2 flex flex-col gap-2.5" aria-labelledby={stepsLabelId}>
            {plan.steps.map((step, i) => (
              <StepItem
                key={step.id}
                index={i + 1}
                step={step}
                run={steps[step.id] ?? { status: "idle" }}
                done={manualDone[step.id] === true}
                executable={canExecuteStep(phase, step.id)}
              />
            ))}
          </ol>
        </>
      )}
    </Body>
  );
}

/** Neutral attribute chip (same geometry as `StatusBadge`). */
function Chip({ icon, children }: { icon?: IconName; children: ReactNode }) {
  return (
    <span className="inline-flex h-[22px] shrink-0 items-center gap-1 rounded-full border border-line bg-surface-2/60 px-2 text-2xs font-medium whitespace-nowrap text-fg-muted">
      {icon && <Icon name={icon} size={12} className="-ml-0.5 text-fg-subtle" />}
      {children}
    </span>
  );
}

/** Visual state of a step, derived from the flow (manual steps: the "Fatto" checkbox). */
type StepState = "pending" | "confirming" | "running" | "done" | "failed";

function stepState(step: FixStep, run: StepRun, done: boolean): StepState {
  if (!step.action) return done ? "done" : "pending";
  switch (run.status) {
    case "idle":
      return "pending";
    case "confirming":
      return "confirming";
    case "running":
      return "running";
    case "done":
      return run.outcome.ok ? "done" : "failed";
    case "error":
      return "failed";
  }
}

const MARKER: Record<StepState, string> = {
  pending: "border-line bg-surface-2 text-fg-muted",
  confirming: "border-fg-muted bg-surface-3 text-fg",
  running: "border-line-strong bg-surface-2 text-fg-muted",
  done: "border-ok/30 bg-ok-soft text-ok",
  failed: "border-bad/30 bg-bad-soft text-bad",
};

/**
 * Step number that turns into the step's state: spinner while running, check when done,
 * cross when it failed. Decorative: the state is always also written out in the step.
 */
function StepMarker({ index, state }: { index: number; state: StepState }) {
  return (
    <span
      aria-hidden="true"
      className={`grid size-6 shrink-0 place-items-center rounded-sm border text-2xs font-semibold tabular transition-colors duration-200 ${MARKER[state]}`}
    >
      {state === "running" ? (
        <Spinner size={14} />
      ) : state === "done" ? (
        <Icon name="check" size={12} strokeWidth={2} />
      ) : state === "failed" ? (
        <Icon name="close" size={12} strokeWidth={2} />
      ) : (
        index
      )}
    </span>
  );
}

const stateFade = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  transition: { duration: duration.micro, ease: ease.out },
} as const;

function StepItem({
  index,
  step,
  run,
  done,
  executable,
}: {
  index: number;
  step: FixStep;
  run: StepRun;
  done: boolean;
  executable: boolean;
}) {
  const simple = useIsSimple();
  const t = useT("assistant");
  const checkId = useId();
  const flow = fixFlowStore.getState;
  const state = stepState(step, run, done);
  const platform = usePlatform();
  // A Windows tool on macOS: the step stays visible with its explanation, never runnable.
  const runsHere = step.action === null || actionRunsOn(step.action, platform);
  return (
    <li
      className={`flex gap-3 rounded-md border p-3.5 transition-colors duration-200 ${
        state === "confirming" ? "border-line-strong bg-surface-2/30" : "border-line"
      }`}
    >
      <StepMarker index={index} state={state} />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-start gap-x-3 gap-y-1.5">
          <h4 className="min-w-[min(100%,12rem)] flex-1 text-sm leading-6 font-medium">
            <span className="sr-only">{t("stepNumber", { n: index })} </span>
            {step.title}
          </h4>
          <div className="flex flex-wrap items-center gap-1.5 pt-px">
            <StatusBadge tone={stepRiskTone(step.risk)} label={stepRiskLabel(step.risk)} />
            {step.requiresAdmin && <Chip icon="lock">{t("requiresAdmin")}</Chip>}
            {!step.reversible && <Chip>{t("notReversible")}</Chip>}
            {!runsHere && <Chip>{t("windowsOnly")}</Chip>}
          </div>
        </div>
        <p className="mt-1 text-[13px]">{step.explanation}</p>
        {step.validationNote && (
          <p className="mt-2 flex items-start gap-1.5 text-xs text-fg-muted">
            <Icon name="info" size={14} className="mt-px shrink-0 text-fg-subtle" />
            <span>{t("appNote", { note: step.validationNote })}</span>
          </p>
        )}

        {step.action && !runsHere ? (
          <div className="mt-3">
            {!simple && <p className="mb-2 font-mono text-2xs break-all text-fg-subtle">{actionTechnicalLabel(step.action)}</p>}
            <p className="text-[13px] text-fg-muted">{describeAction(step.action)}</p>
          </div>
        ) : step.action ? (
          <div className="mt-3">
            {!simple && <p className="mb-2 font-mono text-2xs break-all text-fg-subtle">{actionTechnicalLabel(step.action)}</p>}
            <m.div key={run.status} {...stateFade}>
              {run.status === "confirming" ? (
                <div
                  role="group"
                  aria-label={t("confirmStepAria", { n: index })}
                  className="rounded-md border border-line-strong bg-surface p-3"
                >
                  <p className="text-[13px]">
                    <span className="font-medium">{t("whatWillHappen")} </span>
                    {describeAction(step.action)}
                  </p>
                  {step.requiresAdmin && step.action.kind !== "runSystemFileCheck" && (
                    <p className="mt-1 text-[13px] text-fg-muted">{t("mayAskAdmin")}</p>
                  )}
                  {!step.reversible && !actionStatesIrreversible(step.action) && (
                    <p className="mt-1 text-[13px] text-fg-muted">{t("cannotUndo")}</p>
                  )}
                  <div className="mt-3 flex flex-wrap justify-end gap-2">
                    <Button onClick={() => flow().cancelConfirm(step.id)} autoFocus>
                      {t("cancel")}
                    </Button>
                    <Button variant="primary" onClick={() => flow().execute(step.id)} disabled={!executable}>
                      {t("confirmAndRun")}
                    </Button>
                  </div>
                </div>
              ) : run.status === "running" ? (
                <p role="status" className="flex items-center gap-2 text-[13px] text-fg-muted">
                  <Spinner size={14} />
                  {t("runningStep")}
                </p>
              ) : run.status === "done" ? (
                <p role="status" className="text-[13px]">
                  <span className={`font-medium ${run.outcome.ok ? "text-ok" : "text-bad"}`}>
                    {run.outcome.ok ? t("stepDone") : t("stepFailed")}{" "}
                  </span>
                  {run.outcome.message}
                </p>
              ) : (
                <div className="flex flex-col items-start gap-2">
                  {run.status === "error" && (
                    <StateView compact kind={stateKindForError(run.error)} title={t("stepNotRun")} error={run.error} />
                  )}
                  {(run.status === "idle" || executable) && (
                    <Button
                      size="sm"
                      onClick={() => flow().askConfirm(step.id)}
                      disabled={!executable}
                      title={executable ? undefined : t("waitForStep")}
                    >
                      <Icon name="play" size={12} className="text-fg-muted" />
                      {t("runStep")}
                    </Button>
                  )}
                </div>
              )}
            </m.div>
          </div>
        ) : (
          <div className="mt-3 flex flex-col items-start gap-2.5">
            {step.manualInstructions && (
              <div className="w-full rounded-md border border-line bg-surface-2/50 p-3 text-[13px] whitespace-pre-line">
                <span className="font-medium">{t("manualStep")} </span>
                {step.manualInstructions}
              </div>
            )}
            <label htmlFor={checkId} className="flex cursor-pointer items-center gap-2 text-[13px] select-none">
              <input id={checkId} type="checkbox" checked={done} onChange={() => flow().toggleManualDone(step.id)} />
              {t("manualDone")}
            </label>
          </div>
        )}
      </div>
    </li>
  );
}

function PlanFooter({ plan }: { plan: FixPlan }) {
  const simple = useIsSimple();
  const t = useT("assistant");
  // Same rule as the header's close button: not while a step is running.
  const stepRunning = useFixFlow((s) => s.phase.kind === "plan" && isFlowBusy(s.phase));
  return (
    <footer className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-t border-line px-6 py-4">
      <div className="min-w-0 flex-1 text-xs">
        <p className="font-medium text-fg">{t("aiDisclaimer")}</p>
        <p className="mt-0.5 text-2xs text-fg-muted tabular">
          {t("answeredBy", { model: modelLabel(plan.servedBy) })}
          {!simple && <span className="font-mono"> ({plan.servedBy})</span>} · {formatTokenUsage(plan.usage)}
        </p>
      </div>
      <Button
        onClick={() => fixFlowStore.getState().close()}
        disabled={stepRunning}
        title={stepRunning ? t("waitForStep") : undefined}
      >
        {t("close")}
      </Button>
    </footer>
  );
}
