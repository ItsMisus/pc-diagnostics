import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { ProgressBar } from "@/components/common/ProgressBar";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useT } from "@/i18n";
import { formatElapsed } from "@/lib/format";
import { useBench } from "./benchStore";
import { componentOfKind, kindLabel, stateLabel, type BenchComponent } from "./model";
import { MetricsGrid } from "./ResultView";
import { PANEL } from "./ui";

/** Progress of the active run, if it belongs to `component`. Subscribes to the progress slice. */
export function LiveProgress({ component }: { component: BenchComponent }) {
  const t = useT("benchmarks");
  const run = useBench((s) => (s.activeRun && componentOfKind(s.activeRun.kind) === component ? s.activeRun : null));
  const cancelling = useBench((s) => s.cancelling);
  const cancel = useBench((s) => s.cancelRun);
  if (!run) return null;

  const isCancelling = cancelling || run.state === "cancelling";
  const label = kindLabel(run.kind, t);
  const pct = Math.round(Math.min(1, Math.max(0, run.fraction)) * 100);

  return (
    <section aria-label={t("runAria", { kind: label })} className={`${PANEL} p-4`}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2.5">
          <h3 className="truncate text-[15px] leading-6 font-semibold text-fg">{label}</h3>
          <StatusBadge tone="neutral" label={isCancelling ? stateLabel("cancelling", t) : stateLabel(run.state, t)} />
        </div>
        {/* Always reachable while the run is in flight (also from the strip at the top). */}
        <Button variant="danger" onClick={() => void cancel()} disabled={isCancelling} aria-busy={isCancelling}>
          <Icon name="stop" />
          {isCancelling ? t("stopping") : t("stop")}
        </Button>
      </div>
      <div className="mt-4 flex items-baseline justify-between gap-4 text-xs text-fg-muted tabular">
        <span className="min-w-0 truncate text-[13px]">{run.phase || stateLabel(run.state, t)}</span>
        <span aria-live="off" className="shrink-0">
          <span className="font-medium text-fg">{pct}%</span> · {t("elapsed", { elapsed: formatElapsed(run.elapsedMs) })}
        </span>
      </div>
      <ProgressBar fraction={run.fraction} label={t("progressAria", { kind: label })} className="mt-2" />
      <div className="mt-4">
        <MetricsGrid metrics={run.interim} interim />
      </div>
      <p className="mt-3 text-2xs text-fg-subtle">{t("interimNote")}</p>
    </section>
  );
}
