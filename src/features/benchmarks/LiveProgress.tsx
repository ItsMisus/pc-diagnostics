import { Card } from "@/components/common/Card";
import { useT } from "@/i18n";
import { useBench } from "./benchStore";
import { componentOfKind, kindLabel, stateLabel, type BenchComponent } from "./model";
import { MetricsGrid } from "./ResultView";

/**
 * Interim readings of the active run, if it belongs to `component`. Progress, elapsed time
 * and the Stop action live in the running bar at the top of the window (one place only).
 * Subscribes to the progress slice; values update in place, never animated.
 */
export function LiveProgress({ component }: { component: BenchComponent }) {
  const t = useT("benchmarks");
  const run = useBench((s) => (s.activeRun && componentOfKind(s.activeRun.kind) === component ? s.activeRun : null));
  const cancelling = useBench((s) => s.cancelling);
  if (!run) return null;

  const isCancelling = cancelling || run.state === "cancelling";

  return (
    <Card
      title={t("interimTitle")}
      aside={
        <span className="truncate text-xs text-fg-muted">
          {isCancelling ? stateLabel("cancelling", t) : run.phase || stateLabel(run.state, t)}
        </span>
      }
    >
      <section aria-label={t("runAria", { kind: kindLabel(run.kind, t) })} aria-live="off">
        <MetricsGrid metrics={run.interim} interim />
        <p className="mt-3 text-2xs text-fg-subtle">{t("interimNote")}</p>
      </section>
    </Card>
  );
}
