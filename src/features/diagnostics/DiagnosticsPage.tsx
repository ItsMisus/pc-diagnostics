import { useEffect, useId, useMemo, useState, type ReactNode } from "react";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Disclosure } from "@/components/common/Disclosure";
import { Icon } from "@/components/common/Icon";
import { Spinner, StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { toast } from "@/components/common/Toast";
import { ViewModeToggle } from "@/components/common/ViewModeToggle";
import { PageHeader } from "@/components/layout/PageHeader";
import { useIsSimple } from "@/hooks/useViewMode";
import { stateKindForError } from "@/lib/errors";
import { formatBytes, formatDateTime, formatDuration } from "@/lib/format";
import type { CoverageItem } from "@/types/generated/CoverageItem";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";
import type { Finding } from "@/types/generated/Finding";
import type { ReportFormat } from "@/types/generated/ReportFormat";
import type { Severity } from "@/types/generated/Severity";
import { useT } from "@/i18n";
import { diagStore, useDiag } from "./diagStore";
import { FindingCard, SeverityRail, SimpleFindingCard } from "./FindingCard";
import {
  componentKindLabel,
  coverageLabel,
  coverageSentence,
  coverageSummary,
  coverageTone,
  groupFindings,
  noFindingsCopy,
  SEVERITY_ORDER,
  severityCounts,
  severityGroupLabel,
  severityTone,
  sortFindings,
} from "./model";
import { simpleCoverageSentence, simpleShortWindowNote, simpleSummary } from "./simple";

/** Below this the trend-based rules have little history to look at. */
const SHORT_WINDOW_S = 60;

const DOT = { bad: "bg-bad", warn: "bg-warn", ok: "bg-ok", neutral: "bg-fg-subtle" } as const;

export function DiagnosticsPage() {
  const ensureLoaded = useDiag((s) => s.ensureLoaded);
  useEffect(() => ensureLoaded(), [ensureLoaded]);
  const status = useDiag((s) => s.status);
  const report = useDiag((s) => s.report);
  const t = useT("diagnostics");

  return (
    <>
      <PageHeader
        title="Diagnostics"
        description={t("pageDescription")}
        actions={
          <>
            <ViewModeToggle />
            <RunButton />
          </>
        }
      />
      <RunError />
      {status === "ready" && report ? <ReportSwitch report={report} /> : <LoadState />}
    </>
  );
}

export function RunButton({ label }: { label?: string }) {
  const running = useDiag((s) => s.running);
  const run = useDiag((s) => s.run);
  const t = useT("diagnostics");
  return (
    <Button variant="primary" onClick={() => void run()} disabled={running} aria-busy={running}>
      {/* The spinner follows the button text color (it defaults to a muted tone). */}
      {running ? <Spinner className="text-current!" /> : <Icon name="diagnostics" />}
      {running ? t("running") : (label ?? t("run"))}
    </Button>
  );
}

function RunError() {
  const error = useDiag((s) => s.runError);
  const run = useDiag((s) => s.run);
  const hasReport = useDiag((s) => s.report !== null);
  const t = useT("diagnostics");
  if (!error) return null;
  return (
    <Card className="mb-4">
      <StateView
        compact
        kind={stateKindForError(error)}
        title={hasReport ? t("runFailedKeptPrevious") : t("runFailed")}
        error={error}
        onRetry={() => void run()}
      />
    </Card>
  );
}

function LoadState() {
  const status = useDiag((s) => s.status);
  const error = useDiag((s) => s.error);
  const reload = useDiag((s) => s.reload);
  const running = useDiag((s) => s.running);
  const t = useT("diagnostics");

  if (status === "error" && error) {
    const kind = stateKindForError(error);
    return (
      <Card>
        <StateView
          kind={kind}
          title={kind === "unsupported" ? t("unsupportedTitle") : t("reportUnavailable")}
          {...(kind === "unsupported" ? { description: t("unsupportedDescription") } : {})}
          error={error}
          onRetry={reload}
        />
      </Card>
    );
  }
  if (status === "ready") {
    return (
      <Card>
        {running ? (
          <StateView kind="loading" title={t("running")} description={t("runningDescription")} />
        ) : (
          <StateView kind="empty" title={t("neverRun")} description={t("neverRunDescription")} />
        )}
      </Card>
    );
  }
  return (
    <Card>
      <StateView kind="loading" title={t("loadingLatest")} />
    </Card>
  );
}

function ReportSwitch({ report }: { report: DiagnosticReport }) {
  return useIsSimple() ? <SimpleReportView report={report} /> : <ReportView report={report} />;
}

/** Warning-toned note (short observation window). Text first; the icon only reinforces it. */
function Note({ children }: { children: ReactNode }) {
  return (
    <div role="note" className="flex items-start gap-2.5 rounded-lg border border-warn/25 bg-warn-soft px-3.5 py-3 text-[13px] text-fg">
      <Icon name="alert" className="mt-0.5 shrink-0 text-warn" />
      <div className="min-w-0">{children}</div>
    </div>
  );
}

function MetaLine({ children }: { children: ReactNode }) {
  return <p className="text-xs text-fg-muted tabular">{children}</p>;
}

/** Heading of a group of finding cards, with its count. */
function GroupHeading({ label, count }: { label: string; count: number }) {
  return (
    <h2 className="flex items-center gap-2 text-[13px] leading-5 font-medium text-fg">
      {label}
      <span className="inline-flex h-[18px] min-w-[18px] items-center justify-center rounded-full bg-surface-3 px-1.5 text-2xs font-medium text-fg-muted tabular">
        {count}
      </span>
    </h2>
  );
}

const LIST = "flex list-disc flex-col gap-0.5 pl-4 marker:text-fg-subtle";

/** "Semplice": plain summary, one card per finding, coverage as plain sentences. */
function SimpleReportView({ report }: { report: DiagnosticReport }) {
  const t = useT("diagnostics");
  const findings = useMemo(() => sortFindings(report.findings), [report.findings]);
  const indexOf = useFindingIndex(report);
  // `t` changes with the language, so the summary is rebuilt in the new language.
  const summary = useMemo(() => simpleSummary(report.findings, report.coverage), [report.findings, report.coverage, t]);
  const counts = useMemo(() => severityCounts(report.findings), [report.findings]);
  const tone = counts.critical > 0 ? "bad" : counts.warning > 0 ? "warn" : null;

  return (
    <div className="flex flex-col gap-4">
      <MetaLine>{t("simpleCheckedAt", { date: formatDateTime(report.generatedAt) })}</MetaLine>

      <section
        aria-label={t("summaryLabel")}
        className="relative rounded-xl border border-line bg-surface p-4 pl-5 shadow-[inset_0_1px_0_var(--highlight)]"
      >
        {tone && <SeverityRail tone={tone} />}
        <p className="text-[15px] leading-6 font-medium text-fg">{summary[0]}</p>
        {summary.slice(1).map((line) => (
          <p key={line} className="mt-1 text-sm text-fg-muted">
            {line}
          </p>
        ))}
      </section>

      {report.observedWindowS < SHORT_WINDOW_S && <Note>{simpleShortWindowNote()}</Note>}

      {findings.length > 0 && (
        <section aria-label={t("whatWeFound")} className="mt-2 flex flex-col gap-3">
          <GroupHeading label={t("whatWeFound")} count={findings.length} />
          {findings.map((f, i) => (
            <SimpleFindingCard key={`${f.ruleId}-${i}`} finding={f} findingIndex={indexOf(f)} />
          ))}
        </section>
      )}

      <Card title={t("whatWasChecked")} className={findings.length > 0 ? "mt-2" : ""}>
        {report.coverage.length === 0 ? (
          <p className="text-sm text-fg-muted">{t("coverageNotReported")}</p>
        ) : (
          <ul className="flex flex-col gap-2 text-sm">
            {report.coverage.map((c, i) => (
              <li key={`${c.component}-${i}`} className="flex items-start gap-2.5">
                <span
                  aria-hidden="true"
                  className={`mt-[7px] size-1.5 shrink-0 rounded-full ${c.status === "evaluated" ? "bg-fg-subtle" : "bg-warn"}`}
                />
                <span>{simpleCoverageSentence(c)}</span>
              </li>
            ))}
          </ul>
        )}
        <p className="mt-3 border-t border-line pt-3 text-xs text-fg-muted">{t("simpleCoverageCaveat")}</p>
      </Card>

      {report.limitations.length > 0 && (
        <Card>
          <Disclosure summary={t("simpleLimitations")}>
            <ul className={`${LIST} text-sm text-fg-muted`}>
              {report.limitations.map((l) => (
                <li key={l}>{l}</li>
              ))}
            </ul>
          </Disclosure>
        </Card>
      )}

      <ExportPanel />
    </div>
  );
}

function ReportView({ report }: { report: DiagnosticReport }) {
  const t = useT("diagnostics");
  const groups = useMemo(() => groupFindings(report.findings), [report.findings]);
  const indexOf = useFindingIndex(report);
  const coverage = useMemo(() => coverageSummary(report.coverage), [report.coverage]);
  const counts = useMemo(() => severityCounts(report.findings), [report.findings]);

  return (
    <div className="flex flex-col gap-4">
      <MetaLine>
        {t("reportMeta", {
          date: formatDateTime(report.generatedAt),
          window: formatDuration(report.observedWindowS),
          rules: report.rulesVersion,
          version: report.appVersion,
        })}
      </MetaLine>
      {report.observedWindowS < SHORT_WINDOW_S && (
        <Note>{t("shortWindow", { window: formatDuration(report.observedWindowS) })}</Note>
      )}

      <Card title={t("coverageTitle")}>
        <p className="mb-3 text-sm">{coverageSentence(coverage)}</p>
        {report.coverage.length > 0 && <CoverageList items={report.coverage} />}
        {!coverage.complete && (
          <p className="mt-3 text-xs text-fg-muted">{t("coverageCaveat")}</p>
        )}
      </Card>

      {report.findings.length === 0 ? (
        <Card title={t("findingsTitle")}>
          <StateView
            kind="empty"
            title={noFindingsCopy()}
            description={
              <div className="flex flex-col gap-2">
                <p>{t("noFindingsDescription")}</p>
                {coverage.gaps.length > 0 && (
                  <ul className={LIST}>
                    {coverage.gaps.map((g, i) => (
                      <li key={`${g.component}-${i}`}>
                        {componentKindLabel(g.component)} — {coverageLabel(g.status).toLowerCase()}
                        {g.note ? `: ${g.note}` : ""}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            }
          />
        </Card>
      ) : (
        <>
          <SeveritySummary counts={counts} />
          {groups.map((g) => (
            <section key={g.severity} aria-label={severityGroupLabel(g.severity)} className="mt-2 flex flex-col gap-3">
              <GroupHeading label={severityGroupLabel(g.severity)} count={g.findings.length} />
              {g.findings.map((f, i) => (
                <FindingCard key={`${f.ruleId}-${i}`} finding={f} findingIndex={indexOf(f)} />
              ))}
            </section>
          ))}
        </>
      )}

      {report.limitations.length > 0 && (
        <Card title={t("limitationsTitle")} className="mt-2">
          <ul className={`${LIST} text-sm text-fg-muted`}>
            {report.limitations.map((l) => (
              <li key={l}>{l}</li>
            ))}
          </ul>
        </Card>
      )}

      <ExportPanel />
    </div>
  );
}

/** Counts per severity, one tile each. The dot is tinted only when the count is non-zero. */
function SeveritySummary({ counts }: { counts: Record<Severity, number> }) {
  const t = useT("diagnostics");
  return (
    <section
      aria-label={t("severitySummaryLabel")}
      className="rounded-xl border border-line bg-surface shadow-[inset_0_1px_0_var(--highlight)]"
    >
      <dl className="grid grid-cols-3 divide-x divide-line">
        {SEVERITY_ORDER.map((s) => {
          const n = counts[s];
          return (
            <div key={s} className="min-w-0 px-4 py-3">
              <dt className="flex items-center gap-1.5 text-xs text-fg-muted">
                <span aria-hidden="true" className={`size-1.5 shrink-0 rounded-full ${n > 0 ? DOT[severityTone(s)] : "bg-line-strong"}`} />
                {severityGroupLabel(s)}
              </dt>
              <dd className={`mt-1 font-display text-2xl leading-7 font-semibold tabular ${n > 0 ? "text-fg" : "text-fg-subtle"}`}>
                {n}
              </dd>
            </div>
          );
        })}
      </dl>
    </section>
  );
}

/**
 * Position of a finding in `report.findings` (the order `preview_fix` uses), independent
 * of the display sort. -1 when not found.
 */
export function useFindingIndex(report: DiagnosticReport): (f: Finding) => number {
  return useMemo(() => {
    const map = new Map(report.findings.map((f, i) => [f, i] as const));
    return (f: Finding) => map.get(f) ?? -1;
  }, [report.findings]);
}

function CoverageList({ items }: { items: readonly CoverageItem[] }) {
  // Subscribes to the language: the labels below come from the model helpers.
  useT("diagnostics");
  return (
    <ul className="grid grid-cols-1 gap-2 md:grid-cols-2">
      {items.map((c, i) => (
        <li key={`${c.component}-${i}`} className="flex flex-col gap-1 rounded-lg border border-line bg-surface-2/30 px-3 py-2.5">
          <div className="flex items-center justify-between gap-2">
            <span className="text-[13px] font-medium">{componentKindLabel(c.component)}</span>
            <StatusBadge tone={coverageTone(c.status)} label={coverageLabel(c.status)} />
          </div>
          {c.note && <span className="text-xs text-fg-muted">{c.note}</span>}
        </li>
      ))}
    </ul>
  );
}

function ExportPanel() {
  const state = useDiag((s) => s.export);
  const exportReport = useDiag((s) => s.exportReport);
  const [redact, setRedact] = useState(true);
  const id = useId();
  const busy = state.status === "running";
  const t = useT("diagnostics");
  const go = async (format: ReportFormat) => {
    await exportReport(format, redact);
    // Read back the outcome of this very export (the store sets it before resolving).
    const after = diagStore.getState().export;
    if (after.status === "done" && after.format === format) toast.success(t("exportSavedToast"), after.result.path);
  };

  return (
    <Card title={t("exportTitle")}>
      <div className="flex flex-wrap items-center gap-x-5 gap-y-3">
        <div className="flex flex-wrap gap-2">
          {(["json", "html"] as const).map((format) => {
            const running = busy && state.format === format;
            return (
              <Button key={format} onClick={() => void go(format)} disabled={busy} aria-busy={running}>
                {running ? <Spinner /> : <Icon name="download" className="text-fg-muted" />}
                {running ? t("exporting") : t("exportAs", { format: format.toUpperCase() })}
              </Button>
            );
          })}
        </div>
        <label htmlFor={id} className="flex cursor-pointer items-center gap-2 text-[13px] select-none">
          <input id={id} type="checkbox" checked={redact} onChange={(e) => setRedact(e.target.checked)} />
          {t("redact")}
        </label>
      </div>
      <p className="mt-2.5 max-w-prose text-xs text-fg-muted">{t("exportHint")}</p>
      {state.status === "done" && (
        <div role="status" className="mt-3 flex items-start gap-2.5 rounded-lg border border-line bg-surface-2/40 px-3 py-2.5">
          <Icon name="check" className="mt-0.5 shrink-0 text-ok" />
          <div className="min-w-0 text-[13px]">
            <p>
              {t(state.result.redacted ? "exportSavedRedacted" : "exportSavedPersonal", {
                format: state.format.toUpperCase(),
              })}{" "}
              · <span className="tabular">{formatBytes(state.result.bytes)}</span>
            </p>
            <p className="mt-0.5 font-mono text-2xs break-all text-fg-muted">{state.result.path}</p>
          </div>
        </div>
      )}
      {state.status === "error" && (
        <div className="mt-3">
          <StateView
            compact
            kind={stateKindForError(state.error)}
            title={state.error.code === "notFound" ? t("exportNothing") : t("exportFailed")}
            error={state.error}
            {...(state.error.code === "notFound" ? {} : { onRetry: () => void go(state.format) })}
          />
        </div>
      )}
    </Card>
  );
}
