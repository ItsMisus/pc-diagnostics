import { useEffect, useMemo, type ReactNode } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { useIsSimple } from "@/hooks/useViewMode";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime } from "@/lib/format";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";
import type { Finding } from "@/types/generated/Finding";
import { RisolviButton } from "@/features/assistant/RisolviButton";
import { useT } from "@/i18n";
import { RunButton, useFindingIndex } from "./DiagnosticsPage";
import { useDiag } from "./diagStore";
import { KindBadge, SimpleKindHint } from "./FindingCard";
import {
  coverageSentence,
  coverageSummary,
  noFindingsCopy,
  severityCounts,
  severityLabel,
  severityTone,
  topFindings,
} from "./model";
import { simpleFindingText, simpleNotAGuarantee, simpleNothingFound, simpleSeverityLabel } from "./simple";

const DOT = { bad: "bg-bad", warn: "bg-warn", ok: "bg-ok", neutral: "bg-fg-subtle" } as const;

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
 * One top finding in one compact row: severity dot (its label is read out to assistive
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
      <span aria-hidden="true" className={`mt-[7px] size-1.5 shrink-0 rounded-full ${DOT[severityTone(finding.severity)]}`} />
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-3 gap-y-1.5">
        <p className="min-w-[min(100%,12rem)] flex-1 text-[13px] leading-5 text-fg">
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
  const coverage = useMemo(() => coverageSummary(report.coverage), [report.coverage]);
  const simple = useIsSimple();
  const t = useT("diagnostics");

  if (simple) {
    const badge = (n: number, s: "critical" | "warning" | "info") => `${n} ${simpleSeverityLabel(s).toLowerCase()}`;
    return (
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center gap-1.5">
          <StatusBadge tone={counts.critical > 0 ? "bad" : "neutral"} label={badge(counts.critical, "critical")} />
          <StatusBadge tone={counts.warning > 0 ? "warn" : "neutral"} label={badge(counts.warning, "warning")} />
          <StatusBadge tone="neutral" label={badge(counts.info, "info")} />
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
        <StatusBadge tone={counts.critical > 0 ? "bad" : "neutral"} label={t.plural("insightsCritical", counts.critical)} />
        <StatusBadge tone={counts.warning > 0 ? "warn" : "neutral"} label={t.plural("insightsWarning", counts.warning)} />
        <StatusBadge tone="neutral" label={t.plural("insightsInfo", counts.info)} />
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
              kind={<KindBadge kind={f.kind} />}
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
