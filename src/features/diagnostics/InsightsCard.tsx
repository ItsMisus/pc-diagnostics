import { useEffect, useMemo, type ReactNode } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Icon, type IconName } from "@/components/common/Icon";
import { Panel as Card } from "@/components/common/Panel";
import { StateView } from "@/components/common/StateView";
import { StatusBadge, type Tone } from "@/components/common/StatusBadge";
import { useIsSimple } from "@/hooks/useViewMode";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime } from "@/lib/format";
import { usePlatform } from "@/lib/platform";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";
import type { Finding } from "@/types/generated/Finding";
import { RisolviButton } from "@/features/assistant/RisolviButton";
import { useT } from "@/i18n";
import { RunButton, useFindingIndex } from "./DiagnosticsPage";
import { useDiag } from "./diagStore";
import { CauseBadge, SimpleKindHint } from "./FindingCard";
import {
  coverageSentence,
  coverageSummary,
  noFindingsCopy,
  platformCoverage,
  severityCounts,
  severityLabel,
  severityTone,
  topFindings,
} from "./model";
import { simpleFindingText, simpleNotAGuarantee, simpleNothingFound, simpleSeverityLabel } from "./simple";

/** Severity shape cue (cross / triangle / info), decorative: the severity is also read out in words. */
const GLYPH: Record<Tone, { icon: IconName; className: string }> = {
  bad: { icon: "error", className: "text-bad" },
  warn: { icon: "warning", className: "text-warn" },
  ok: { icon: "success", className: "text-ok" },
  neutral: { icon: "info", className: "text-fg-subtle" },
};

/** Overview "System Insights": summary of the latest diagnostic report. */
export function InsightsCard({ className = "" }: { className?: string }) {
  const ensureLoaded = useDiag((s) => s.ensureLoaded);
  useEffect(() => ensureLoaded(), [ensureLoaded]);
  const status = useDiag((s) => s.status);
  const report = useDiag((s) => s.report);
  const error = useDiag((s) => s.error);
  const reload = useDiag((s) => s.reload);
  const runError = useDiag((s) => s.runError);
  const setSection = useAppStore((s) => s.setSection);
  const t = useT("diagnostics");

  return (
    <Card
      title={t("insightsTitle")}
      className={className}
      aside={
        status === "ready" && report ? (
          <Button variant="ghost" size="sm" className="-my-1 -mr-2" onClick={() => setSection("diagnostics")}>
            {t("openDiagnostics")}
            <Icon name="chevronRight" size={14} className="-mr-0.5" />
          </Button>
        ) : null
      }
    >
      {status === "ready" && report ? (
        <Summary report={report} />
      ) : status === "error" && error ? (
        <StateView compact kind={stateKindForError(error)} title={t("insightsUnavailable")} error={error} onRetry={reload} />
      ) : status === "ready" ? (
        <div className="flex flex-col items-start gap-3">
          <StateView compact kind="empty" title={t("neverRun")} description={t("insightsNeverRunDescription")} />
          <RunButton />
          {runError && <StateView compact kind={stateKindForError(runError)} title={t("runFailed")} error={runError} />}
        </div>
      ) : (
        <StateView compact kind="loading" title={t("loadingLatest")} />
      )}
    </Card>
  );
}

/**
 * One top finding in one compact row: severity glyph (its label is read out to assistive
 * technology, so color is not the only signal), title, then the kind marker and "Risolvi",
 * which wrap under the title only in a narrow column.
 */
function TopFinding({
  finding,
  severity,
  title,
  kind,
  action,
}: {
  finding: Finding;
  /** Severity in words, for screen readers. */
  severity: string;
  title: string;
  kind: ReactNode;
  action: ReactNode;
}) {
  return (
    <li className="flex items-start gap-2.5 border-t border-line py-2 first:border-t-0 first:pt-0 last:pb-0">
      <Icon name={GLYPH[severityTone(finding.severity)].icon} size={14} className={`mt-0.5 shrink-0 ${GLYPH[severityTone(finding.severity)].className}`} />
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-3 gap-y-1.5">
        <p className="min-w-[min(100%,12rem)] flex-1 text-sm leading-5 text-fg">
          <span className="sr-only">{severity}: </span>
          {title}
        </p>
        <div className="flex flex-wrap items-center gap-1.5 empty:hidden">
          {kind}
          {action}
        </div>
      </div>
    </li>
  );
}

function Summary({ report }: { report: DiagnosticReport }) {
  const counts = useMemo(() => severityCounts(report.findings), [report.findings]);
  const top = useMemo(() => topFindings(report.findings, 3), [report.findings]);
  const indexOf = useFindingIndex(report);
  const platform = usePlatform();
  const coverage = useMemo(() => coverageSummary(platformCoverage(report.coverage, platform)), [report.coverage, platform]);
  const simple = useIsSimple();
  const t = useT("diagnostics");

  if (simple) {
    const badge = (n: number, s: "critical" | "warning" | "info") => `${n} ${simpleSeverityLabel(s).toLowerCase()}`;
    return (
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center gap-1.5">
          {counts.critical > 0 && <StatusBadge glyph tone="bad" label={badge(counts.critical, "critical")} />}
          {counts.warning > 0 && <StatusBadge glyph tone="warn" label={badge(counts.warning, "warning")} />}
          {counts.info > 0 && <StatusBadge glyph tone="neutral" label={badge(counts.info, "info")} />}
          <span className="ml-auto text-2xs text-fg-subtle tabular">
            {t("insightsCheckedAt", { date: formatDateTime(report.generatedAt) })}
          </span>
        </div>
        {top.length === 0 ? (
          <p className="text-sm text-fg-muted">
            {simpleNothingFound()} {simpleNotAGuarantee()}
          </p>
        ) : (
          <ul className="flex flex-col">
            {top.map((f, i) => {
              const headline = simpleFindingText(f).headline;
              return (
                <TopFinding
                  key={`${f.ruleId}-${i}`}
                  finding={f}
                  severity={simpleSeverityLabel(f.severity)}
                  title={headline}
                  kind={<SimpleKindHint finding={f} />}
                  action={<RisolviButton compact finding={f} findingIndex={indexOf(f)} label={headline} />}
                />
              );
            })}
          </ul>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-1.5">
        {/* Only categories that have findings: "0 critici" is noise (rule R5, counts once). */}
        {counts.critical > 0 && <StatusBadge glyph tone="bad" label={t.plural("insightsCritical", counts.critical)} />}
        {counts.warning > 0 && <StatusBadge glyph tone="warn" label={t.plural("insightsWarning", counts.warning)} />}
        {counts.info > 0 && <StatusBadge glyph tone="neutral" label={t.plural("insightsInfo", counts.info)} />}
        <span className="ml-auto text-2xs text-fg-subtle tabular">
          {t("insightsReportAt", { date: formatDateTime(report.generatedAt) })}
        </span>
      </div>
      {top.length === 0 ? (
        <p className="text-sm text-fg-muted">{noFindingsCopy()}.</p>
      ) : (
        <ul className="flex flex-col">
          {top.map((f, i) => (
            <TopFinding
              key={`${f.ruleId}-${i}`}
              finding={f}
              severity={severityLabel(f.severity)}
              title={f.title}
              kind={f.kind === "observation" ? null : <CauseBadge kind={f.kind} />}
              action={<RisolviButton compact finding={f} findingIndex={indexOf(f)} label={f.title} />}
            />
          ))}
        </ul>
      )}
      <p className="border-t border-line pt-2.5 text-xs text-fg-subtle">
        {t("insightsCoverage", { sentence: coverageSentence(coverage) })}
      </p>
    </div>
  );
}
