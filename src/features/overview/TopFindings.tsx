import { useEffect, useMemo } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { Panel } from "@/components/common/Panel";
import { SkeletonLines } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { RisolviButton } from "@/features/assistant/RisolviButton";
import { useFindingIndex } from "@/features/diagnostics/DiagnosticsPage";
import { useDiag } from "@/features/diagnostics/diagStore";
import { KindBadge, SimpleKindHint } from "@/features/diagnostics/FindingCard";
import { coverageSentence, coverageSummary, noFindingsCopy, platformCoverage, severityLabel, severityTone, topFindings } from "@/features/diagnostics/model";
import { simpleFindingText, simpleNotAGuarantee, simpleNothingFound, simpleSeverityLabel } from "@/features/diagnostics/simple";
import { useIsSimple } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { usePlatform } from "@/lib/platform";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";

/** Findings shown on the Overview; the full report is on Diagnostics. */
const TOP = 3;

/**
 * The top findings of the latest diagnosis: severity badge with glyph, one-line headline, the
 * kind marker and "Risolvi" where offered, and a link to Diagnostics. Counts and the time of
 * the run are said once, in the hero's overall state. A load error is stated here too (with
 * Retry), because this panel has nothing else to show.
 */
export function TopFindings({ className = "" }: { className?: string }) {
  const t = useT("overview");
  const td = useT("diagnostics");
  const ensureLoaded = useDiag((s) => s.ensureLoaded);
  useEffect(() => ensureLoaded(), [ensureLoaded]);
  const status = useDiag((s) => s.status);
  const report = useDiag((s) => s.report);
  const error = useDiag((s) => s.error);
  const reload = useDiag((s) => s.reload);
  const setSection = useAppStore((s) => s.setSection);

  return (
    <Panel
      title={t("findingsTitle")}
      icon="diagnostics"
      className={className}
      aside={
        <Button variant="ghost" size="sm" className="-my-1 -me-2" onClick={() => setSection("diagnostics")}>
          {td("openDiagnostics")}
          <Icon name="chevronRight" size={14} className="-me-0.5" />
        </Button>
      }
    >
      {report ? (
        <FindingList report={report} />
      ) : status === "error" && error ? (
        <StateView compact kind={stateKindForError(error)} title={td("insightsUnavailable")} error={error} onRetry={reload} />
      ) : status === "ready" ? (
        <StateView compact kind="empty" title={td("neverRun")} description={t("findingsNeverRun")} />
      ) : (
        <div role="status" aria-busy="true">
          <span className="sr-only">{td("loadingLatest")}</span>
          <SkeletonLines lines={3} />
        </div>
      )}
    </Panel>
  );
}

function FindingList({ report }: { report: DiagnosticReport }) {
  const td = useT("diagnostics");
  const simple = useIsSimple();
  const platform = usePlatform();
  const top = useMemo(() => topFindings(report.findings, TOP), [report.findings]);
  const coverage = useMemo(() => coverageSummary(platformCoverage(report.coverage, platform)), [report.coverage, platform]);
  const indexOf = useFindingIndex(report);

  if (top.length === 0) {
    return (
      <StateView
        compact
        kind="empty"
        title={simple ? simpleNothingFound() : noFindingsCopy()}
        {...(simple ? { description: simpleNotAGuarantee() } : { description: td("insightsCoverage", { sentence: coverageSentence(coverage) }) })}
      />
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <ul className="flex flex-col">
        {top.map((f, i) => {
          const title = simple ? simpleFindingText(f).headline : f.title;
          const severity = simple ? simpleSeverityLabel(f.severity) : severityLabel(f.severity);
          return (
            <li key={`${f.ruleId}-${i}`} className="flex flex-col gap-1.5 border-t border-line py-2.5 first:border-t-0 first:pt-0 last:pb-0">
              <div className="flex min-w-0 items-start gap-2.5">
                <StatusBadge tone={severityTone(f.severity)} label={severity} glyph size="sm" />
                <p className="min-w-0 flex-1 text-sm break-words text-fg">{title}</p>
              </div>
              <div className="flex flex-wrap items-center gap-1.5 empty:hidden">
                {simple ? <SimpleKindHint finding={f} /> : <KindBadge kind={f.kind} />}
                <RisolviButton compact finding={f} findingIndex={indexOf(f)} label={title} />
              </div>
            </li>
          );
        })}
      </ul>
      {!simple && <p className="border-t border-line pt-2.5 text-xs text-fg-subtle">{td("insightsCoverage", { sentence: coverageSentence(coverage) })}</p>}
    </div>
  );
}
