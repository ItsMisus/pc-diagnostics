import { SectionHeading } from "@/components/layout/SectionHeading";
import { useEffect, useId, useMemo, useState, type ReactNode } from "react";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Disclosure } from "@/components/common/Disclosure";
import { Icon } from "@/components/common/Icon";
import { Spinner, StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { toast } from "@/components/common/Toast";
import { PageHeader } from "@/components/layout/PageHeader";
import { useIsSimple } from "@/hooks/useViewMode";
import { stateKindForError } from "@/lib/errors";
import { formatBytes, formatDateTime, formatDuration } from "@/lib/format";
import type { CoverageItem } from "@/types/generated/CoverageItem";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";
import type { Finding } from "@/types/generated/Finding";
import type { ReportFormat } from "@/types/generated/ReportFormat";
import { useT } from "@/i18n";
import { diagStore, useDiag } from "./diagStore";
import { FindingCard, SimpleFindingCard } from "./FindingCard";
import {
  componentKindLabel,
  coverageLabel,
  coverageSentence,
  coverageSummary,
  coverageTone,
  groupFindings,
  noFindingsCopy,
  severityGroupLabel,
  sortFindings,
} from "./model";
import { simpleComponentName, simpleCoverageLabel, simpleCoverageNote, simpleShortWindowNote, simpleSummary } from "./simple";

/**
 * Below this the sustained-load rules are not evaluated (`SUSTAINED_MIN_WINDOW_S` in the
 * diagnostics crate). The page says so once, in one note; component notes do not repeat it.
 */
const SHORT_WINDOW_S = 60;

export function DiagnosticsPage() {
  const ensureLoaded = useDiag((s) => s.ensureLoaded);
  useEffect(() => ensureLoaded(), [ensureLoaded]);
  const status = useDiag((s) => s.status);
  const report = useDiag((s) => s.report);

  return (
    <>
      <PageHeader title="Diagnostics" actions={<RunButton />} />
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

/** The page's one short-window warning. Text first; the icon only reinforces it. */
function ShortWindowNote({ children }: { children: ReactNode }) {
  return (
    <div role="note" className="flex items-start gap-2 rounded-md border border-warn/25 bg-warn-soft px-3 py-2 text-[13px] text-fg">
      <Icon name="alert" size={14} className="mt-0.5 shrink-0 text-warn" />
      <div className="min-w-0">{children}</div>
    </div>
  );
}

function MetaLine({ children }: { children: ReactNode }) {
  return <p className="text-xs text-fg-subtle tabular">{children}</p>;
}

/** Heading of a group of finding cards; `count` only where no summary already states it. */
function GroupHeading({ id, label, count }: { id: string; label: string; count?: number }) {
  return (
    <SectionHeading id={id} {...(count !== undefined ? { count } : {})}>
      {label}
    </SectionHeading>
  );
}

const LIST = "flex list-disc flex-col gap-0.5 pl-4 marker:text-fg-subtle";

/** "Semplice": plain summary (it carries the counts), one card per finding, compact coverage. */
function SimpleReportView({ report }: { report: DiagnosticReport }) {
  const t = useT("diagnostics");
  const findings = useMemo(() => sortFindings(report.findings), [report.findings]);
  const indexOf = useFindingIndex(report);
  // `t` changes with the language, so the summary is rebuilt in the new language.
  const summary = useMemo(() => simpleSummary(report.findings, report.coverage), [report.findings, report.coverage, t]);
  const headingId = useId();

  return (
    <div className="flex flex-col gap-4">
      <MetaLine>{t("simpleCheckedAt", { date: formatDateTime(report.generatedAt) })}</MetaLine>

      <Card title={t("summaryLabel")}>
        <p className="text-[15px] leading-6 font-medium text-fg">{summary[0]}</p>
        {summary.slice(1).map((line) => (
          <p key={line} className="mt-1 text-sm text-fg-muted">
            {line}
          </p>
        ))}
      </Card>

      {report.observedWindowS < SHORT_WINDOW_S && <ShortWindowNote>{simpleShortWindowNote()}</ShortWindowNote>}

      {findings.length > 0 && (
        <section aria-labelledby={headingId} className="flex flex-col gap-3">
          <GroupHeading id={headingId} label={t("whatWeFound")} />
          {findings.map((f, i) => (
            <SimpleFindingCard key={`${f.ruleId}-${i}`} finding={f} findingIndex={indexOf(f)} />
          ))}
        </section>
      )}

      <CoverageCard items={report.coverage} simple />

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

/** "Avanzata": coverage, then the findings grouped by severity (each heading carries its count). */
function ReportView({ report }: { report: DiagnosticReport }) {
  const t = useT("diagnostics");
  const groups = useMemo(() => groupFindings(report.findings), [report.findings]);
  const indexOf = useFindingIndex(report);
  const baseId = useId();

  return (
    <div className="flex flex-col gap-4">
      <MetaLine>
        {t("reportMeta", {
          date: formatDateTime(report.generatedAt),
          rules: report.rulesVersion,
          version: report.appVersion,
        })}
      </MetaLine>
      {report.observedWindowS < SHORT_WINDOW_S && (
        <ShortWindowNote>{t("shortWindow", { window: formatDuration(report.observedWindowS), min: SHORT_WINDOW_S })}</ShortWindowNote>
      )}

      <CoverageCard items={report.coverage} simple={false} />

      {report.findings.length === 0 ? (
        <Card title={t("findingsTitle")}>
          <StateView compact kind="empty" title={noFindingsCopy()} description={t("noFindingsDescription")} />
        </Card>
      ) : (
        groups.map((g) => {
          const id = `${baseId}-${g.severity}`;
          return (
            <section key={g.severity} aria-labelledby={id} className="flex flex-col gap-3">
              <GroupHeading id={id} label={severityGroupLabel(g.severity)} count={g.findings.length} />
              {g.findings.map((f, i) => (
                <FindingCard key={`${f.ruleId}-${i}`} finding={f} findingIndex={indexOf(f)} />
              ))}
            </section>
          );
        })
      )}

      {report.limitations.length > 0 && (
        <Card title={t("limitationsTitle")}>
          <ul className={`${LIST} text-[13px] text-fg-muted`}>
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

/**
 * Coverage at a glance: a summary line ("Avanzata"), one badge per component, and a note
 * only for the components that were not fully evaluated (the note says what is missing).
 */
function CoverageCard({ items, simple }: { items: readonly CoverageItem[]; simple: boolean }) {
  const t = useT("diagnostics");
  const summary = useMemo(() => coverageSummary(items), [items]);
  const name = (c: CoverageItem) => (simple ? simpleComponentName(c.component) : componentKindLabel(c.component));
  const notes = summary.gaps
    .map((c) => ({ c, note: simple ? simpleCoverageNote(c) : c.note.trim() }))
    .filter((x) => x.note !== "");

  return (
    <Card title={simple ? t("whatWasChecked") : t("coverageTitle")}>
      {items.length === 0 ? (
        <p className="text-sm text-fg-muted">{simple ? t("coverageNotReported") : coverageSentence(summary)}</p>
      ) : (
        <>
          {!simple && <p className="mb-2.5 text-[13px] text-fg-muted">{coverageSentence(summary)}</p>}
          <ul className="flex flex-wrap gap-1.5">
            {items.map((c, i) => (
              <li key={`${c.component}-${i}`}>
                <StatusBadge
                  tone={coverageTone(c.status)}
                  label={t("coverageBadge", {
                    name: name(c),
                    status: simple ? simpleCoverageLabel(c.status) : coverageLabel(c.status),
                  })}
                />
              </li>
            ))}
          </ul>
          {notes.length > 0 && (
            <ul className="mt-3 flex flex-col gap-1 border-t border-line pt-3 text-xs text-fg-muted">
              {notes.map(({ c, note }, i) => (
                <li key={`${c.component}-${i}`}>
                  <span className="font-medium text-fg">{name(c)}</span>: {note}
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </Card>
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
      <p className="mt-2 max-w-prose text-xs text-fg-subtle">{t("exportHint")}</p>
      {state.status === "done" && (
        <div role="status" className="mt-3 flex items-start gap-2 rounded-md border border-line bg-surface-2/40 px-3 py-2">
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
