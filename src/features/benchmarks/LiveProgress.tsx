import { useEffect, useId, useRef } from "react";
import { Button } from "@/components/common/Button";
import { ProgressBar } from "@/components/common/ProgressBar";
import { StateView } from "@/components/common/StateView";
import { useT } from "@/i18n";
import { formatElapsed, formatPercent } from "@/lib/format";
import { useBench } from "./benchStore";
import { componentOfKind, kindLabel, stateLabel, type BenchComponent } from "./model";
import { MetricsGrid } from "./ResultView";
import { useBenchView } from "./viewStore";

/**
 * The run in progress, if it belongs to `component`: live phase, progress, elapsed time, the
 * Stop action and the interim readings. It takes the place of Start inside the test panel
 * (one action at a time), and while it is on screen the floating strip steps aside, so
 * progress and Stop appear once. Subscribes to the progress slice; values update in place,
 * never animated.
 */
export function LiveProgress({ component }: { component: BenchComponent }) {
  const t = useT("benchmarks");
  const run = useBench((s) => (s.activeRun && componentOfKind(s.activeRun.kind) === component ? s.activeRun : null));
  const cancelling = useBench((s) => s.cancelling);
  const cancelError = useBench((s) => s.cancelError);
  const cancel = useBench((s) => s.cancelRun);
  const titleId = useId();
  const interimId = useId();
  const controlsRef = useReportInView(run !== null);
  if (!run) return null;

  const isCancelling = cancelling || run.state === "cancelling";
  const label = kindLabel(run.kind, t);
  const phase = isCancelling ? stateLabel("cancelling", t) : run.phase || stateLabel(run.state, t);
  const fraction = Math.min(1, Math.max(0, Number.isFinite(run.fraction) ? run.fraction : 0));

  return (
    <section aria-labelledby={titleId} className="flex flex-col gap-5">
      <div ref={controlsRef} className="flex flex-col gap-3">
        <div className="flex flex-wrap items-end justify-between gap-x-4 gap-y-2">
          <div className="flex min-w-0 flex-col gap-0.5">
            <h3 id={titleId} className="eyebrow">
              {t("liveTitle")} · {label}
            </h3>
            {/* The phase changes a few times per run: the one live text worth announcing. */}
            <p className="min-w-0 text-md font-medium break-words text-fg" aria-live="polite">
              {phase}
            </p>
          </div>
          <Button
            variant="danger"
            icon="stop"
            onClick={() => void cancel()}
            disabled={isCancelling}
            loading={isCancelling}
            className="shrink-0"
          >
            {isCancelling ? t("stopping") : t("stop")}
          </Button>
        </div>
        <ProgressBar fraction={fraction} tone="accent" label={t("progressAria", { kind: label })} />
        <p className="flex flex-wrap justify-between gap-x-4 text-xs text-fg-muted tabular" aria-live="off">
          <span className="font-medium text-fg">{formatPercent(fraction * 100)}</span>
          <span>{t("elapsed", { time: formatElapsed(run.elapsedMs) })}</span>
        </p>
        {cancelError && (
          <StateView
            compact
            kind="error"
            title={t("stopFailed")}
            error={{ ...cancelError, retriable: true }}
            onRetry={() => void cancel()}
          />
        )}
      </div>

      <section aria-labelledby={interimId} aria-live="off" className="flex flex-col gap-3">
        <h4 id={interimId} className="text-sm font-semibold text-fg">
          {t("interimTitle")}
        </h4>
        <MetricsGrid metrics={run.interim} interim />
        <p className="text-2xs text-fg-subtle">{t("interimNote")}</p>
      </section>
    </section>
  );
}

/**
 * Tells the view store whether the run block is visible in the scroll area (excluding the
 * sticky 64 px toolbar and the 96 px reserved for the floating strip), so the strip only steps
 * aside while progress and Stop are actually on screen. Without IntersectionObserver the
 * block counts as not visible and the strip stays: Stop is never out of reach.
 */
function useReportInView(active: boolean) {
  const ref = useRef<HTMLDivElement>(null);
  const setRunInView = useBenchView((s) => s.setRunInView);
  useEffect(() => {
    const el = ref.current;
    if (!active || !el || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(([entry]) => setRunInView(entry?.isIntersecting ?? false), {
      rootMargin: "-64px 0px -96px 0px",
    });
    observer.observe(el);
    return () => {
      observer.disconnect();
      setRunInView(false);
    };
  }, [active, setRunInView]);
  return ref;
}
