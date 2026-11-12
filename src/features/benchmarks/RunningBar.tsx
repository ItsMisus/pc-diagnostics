import { AnimatePresence, m } from "motion/react";
import { useEffect, useRef } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { ProgressBar } from "@/components/common/ProgressBar";
import { StateView } from "@/components/common/StateView";
import { toast } from "@/components/common/Toast";
import { Tooltip } from "@/components/common/Tooltip";
import { translatorFor, useT } from "@/i18n";
import { formatElapsed, formatPercent, formatUnitValue } from "@/lib/format";
import { duration, ease } from "@/lib/motion";
import type { BenchmarkProgress } from "@/types/generated/BenchmarkProgress";
import { useBench } from "./benchStore";
import { componentOfKind, headlineMetric, isRunOnScreen, kindLabel, stateLabel } from "./model";
import { COMPONENT_ICON } from "./ui";
import { useBenchView } from "./viewStore";

/**
 * Enters when a run starts and leaves when it ends. The values are constant, so the many
 * progress re-renders in between never restart the animation. It floats over the content,
 * so nothing below moves when it appears.
 */
const stripMotion = {
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0, transition: { duration: duration.component, ease: ease.out } },
  exit: { opacity: 0, y: 4, transition: { duration: duration.instant, ease: ease.in } },
} as const;

/**
 * Global "benchmark running" capsule, glass floating at the bottom of the main area on every
 * page (AppShell positions it): the run's phase, progress, elapsed time and Stop, wherever
 * the user is. On the running component's own screen it steps aside, because the test panel
 * shows the same progress and Stop (one place at a time). It is the only shell component
 * subscribed to the progress slice, so progress events re-render it (and the run block) but
 * not the rest of the app.
 */
export function RunningBar() {
  const t = useT("benchmarks");
  const run = useBench((s) => s.activeRun);
  const section = useAppStore((s) => s.section);
  const openComponent = useBenchView((s) => s.component);
  const visible = run !== null && !isRunOnScreen(run, section, openComponent);
  useCompletionToast();

  return (
    <AnimatePresence initial={false}>
      {visible && (
        <m.div
          key="running-bar"
          role="region"
          aria-label={t("runningRegion")}
          {...stripMotion}
          className="pointer-events-none absolute inset-x-0 bottom-4 z-20 flex justify-center px-4 sm:px-6"
        >
          {/* While exiting, AnimatePresence keeps rendering the last children it was given. */}
          <Strip run={run} />
        </m.div>
      )}
    </AnimatePresence>
  );
}

function Strip({ run }: { run: BenchmarkProgress }) {
  const t = useT("benchmarks");
  const cancelling = useBench((s) => s.cancelling);
  const cancelError = useBench((s) => s.cancelError);
  const cancel = useBench((s) => s.cancelRun);
  const setSection = useAppStore((s) => s.setSection);
  const openComponent = useBenchView((s) => s.open);

  const isCancelling = cancelling || run.state === "cancelling";
  const label = kindLabel(run.kind, t);
  const fraction = Math.min(1, Math.max(0, Number.isFinite(run.fraction) ? run.fraction : 0));
  const component = componentOfKind(run.kind);

  return (
    <div className={`glass pointer-events-auto w-[min(44rem,100%)] ${cancelError ? "rounded-2xl" : "rounded-full"}`}>
      <div className="flex h-13 items-center gap-3 pr-2 pl-2">
        <Tooltip content={t("openScreen")} side="top" className="min-w-0 shrink">
          <button
            type="button"
            onClick={() => {
              openComponent(component);
              setSection("benchmarks");
            }}
            className="flex min-w-0 items-center gap-2.5 rounded-full py-1 pr-3 pl-1 text-left text-sm transition-colors duration-150 hover:bg-control"
          >
            <span aria-hidden="true" className="satin grid size-8 shrink-0 place-items-center rounded-full text-accent-text">
              <Icon name={COMPONENT_ICON[component]} size={16} />
            </span>
            <span className="min-w-0 truncate">
              <span className="font-semibold text-fg">{label}</span>
              <span className="text-fg-muted">
                {" "}
                · {isCancelling ? stateLabel("cancelling", t) : run.phase || stateLabel(run.state, t)}
              </span>
              <span className="sr-only">. {t("openScreen")}</span>
            </span>
          </button>
        </Tooltip>
        <ProgressBar fraction={fraction} tone="accent" label={t("progressAria", { kind: label })} className="min-w-16 flex-1" />
        <span className="shrink-0 text-right text-xs whitespace-nowrap text-fg-muted tabular" aria-live="off">
          <span className="font-medium text-fg">{formatPercent(fraction * 100)}</span> · {formatElapsed(run.elapsedMs)}
        </span>
        <Button
          variant="danger"
          size="sm"
          icon="stop"
          onClick={() => void cancel()}
          disabled={isCancelling}
          loading={isCancelling}
          className="shrink-0"
        >
          {isCancelling ? t("stopping") : t("stop")}
        </Button>
      </div>
      {cancelError && (
        <div className="px-4 pb-3">
          <StateView
            compact
            kind="error"
            title={t("stopFailed")}
            error={{ ...cancelError, retriable: true }}
            onRetry={() => void cancel()}
          />
        </div>
      )}
    </div>
  );
}

/**
 * Confirms a completed run when the strip goes away and the result is not already on screen
 * (i.e. the user is elsewhere). Cancelled and failed runs are reported inline only.
 */
function useCompletionToast() {
  const finished = useBench((s) => s.lastFinished);
  const seen = useRef(finished?.runId ?? null);
  useEffect(() => {
    if (!finished || finished.runId === seen.current) return;
    seen.current = finished.runId;
    if (finished.state !== "completed") return;
    const onResultScreen =
      useAppStore.getState().section === "benchmarks" &&
      useBenchView.getState().component === componentOfKind(finished.kind);
    if (onResultScreen) return;
    const head = headlineMetric(finished);
    // A toast is a one-off message: it is written once, in the language current at that moment.
    const t = translatorFor("benchmarks");
    const kind = kindLabel(finished.kind, t);
    toast.success(t("completedToast"), head ? `${kind} · ${head.label} ${formatUnitValue(head.value, head.unit)}` : kind);
  }, [finished]);
}
