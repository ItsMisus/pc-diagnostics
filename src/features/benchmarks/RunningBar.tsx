import { AnimatePresence, m } from "motion/react";
import { useEffect, useRef } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { StateView } from "@/components/common/StateView";
import { toast } from "@/components/common/Toast";
import { Tooltip } from "@/components/common/Tooltip";
import { translatorFor, useT } from "@/i18n";
import { formatElapsed, formatUnitValue } from "@/lib/format";
import { duration, ease } from "@/lib/motion";
import type { BenchmarkProgress } from "@/types/generated/BenchmarkProgress";
import { useBench } from "./benchStore";
import { componentOfKind, headlineMetric, kindLabel, stateLabel } from "./model";
import { RunProgress } from "./ui";
import { useBenchView } from "./viewStore";

/**
 * Enters when a run starts and leaves when it ends. The values are constant, so the many
 * progress re-renders in between never restart the animation. Fixed height (`h-10`): the
 * strip takes its place in one step, no height animation.
 */
const stripMotion = {
  initial: { opacity: 0, y: -4 },
  animate: { opacity: 1, y: 0, transition: { duration: duration.component, ease: ease.out } },
  exit: { opacity: 0, transition: { duration: 0.1, ease: ease.in } },
} as const;

/**
 * Global "benchmark running" strip at the top of the main area, on every page: the one place
 * for a run's progress and its Stop action (the benchmark screen shows only the interim
 * readings). Monochrome; the only control is Stop. It is the only shell component subscribed
 * to the progress slice, so progress events re-render it (and the interim readings) but not
 * the rest of the app.
 */
export function RunningBar() {
  const t = useT("benchmarks");
  const run = useBench((s) => s.activeRun);
  useCompletionToast();

  return (
    <AnimatePresence initial={false}>
      {run && (
        <m.div
          key="running-bar"
          role="region"
          aria-label={t("runningRegion")}
          {...stripMotion}
          className="sticky top-0 z-20 border-b border-line bg-surface"
        >
          {/* While exiting, AnimatePresence keeps rendering the last `run` it was given. */}
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
  const pct = Math.round(Math.min(1, Math.max(0, run.fraction)) * 100);

  return (
    <>
      <div className="mx-auto flex h-10 max-w-6xl items-center gap-3 px-6 lg:px-10">
        <Tooltip content={t("openScreen")} side="bottom" className="min-w-0 shrink">
          <button
            type="button"
            onClick={() => {
              openComponent(componentOfKind(run.kind));
              setSection("benchmarks");
            }}
            className="-ml-1.5 flex min-w-0 items-center gap-2 rounded-md px-1.5 py-1 text-left text-[13px] transition-colors duration-150 hover:bg-surface-2"
          >
            <span className="truncate">
              <span className="font-semibold text-fg">{label}</span>
              <span className="text-fg-muted">
                {" "}
                · {isCancelling ? stateLabel("cancelling", t) : run.phase || stateLabel(run.state, t)}
              </span>
            </span>
          </button>
        </Tooltip>
        <RunProgress fraction={run.fraction} label={t("progressAria", { kind: label })} className="min-w-20 flex-1" />
        <span className="min-w-30 shrink-0 text-right text-xs text-fg-muted tabular" aria-live="off">
          <span className="font-medium text-fg">{pct}%</span> · {formatElapsed(run.elapsedMs)}
        </span>
        <Button
          variant="secondary"
          size="sm"
          onClick={() => void cancel()}
          disabled={isCancelling}
          aria-busy={isCancelling}
          className="shrink-0"
        >
          <Icon name="stop" size={14} />
          {isCancelling ? t("stopping") : t("stop")}
        </Button>
      </div>
      {cancelError && (
        <div className="mx-auto max-w-6xl px-6 pb-2 lg:px-10">
          <StateView
            compact
            kind="error"
            title={t("stopFailed")}
            error={{ ...cancelError, retriable: true }}
            onRetry={() => void cancel()}
          />
        </div>
      )}
    </>
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
