import { useEffect, useId, useMemo, useState, type ReactNode } from "react";
import { Button } from "@/components/common/Button";
import { Disclosure } from "@/components/common/Disclosure";
import { Icon, type IconName } from "@/components/common/Icon";
import { Panel, Surface } from "@/components/common/Panel";
import { Readout } from "@/components/common/Readout";
import { Skeleton, SkeletonLines } from "@/components/common/Skeleton";
import { Spinner, StateView } from "@/components/common/StateView";
import { StatusBadge, type Tone } from "@/components/common/StatusBadge";
import { Switch } from "@/components/common/Switch";
import { toast } from "@/components/common/Toast";
import { PageHeader } from "@/components/layout/PageHeader";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { useIsSimple } from "@/hooks/useViewMode";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime, formatDuration, formatStorageBytes } from "@/lib/format";
import { usePlatform } from "@/lib/platform";
import type { CoverageItem } from "@/types/generated/CoverageItem";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";
import type { Finding } from "@/types/generated/Finding";
import type { ReportFormat } from "@/types/generated/ReportFormat";
import type { Severity } from "@/types/generated/Severity";
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
  overallHeadline,
  overallState,
  platformCoverage,
  SEVERITY_ORDER,
  severityGroupLabel,
  sortFindings,
} from "./model";
import {
  simpleComponentName,
  simpleCoverageLabel,
  simpleCoverageNote,
  simpleSeverityLabel,
  simpleShortWindowNote,
  simpleSummary,
} from "./simple";

/**
 * Below this the sustained-load rules are not evaluated (`SUSTAINED_MIN_WINDOW_S` in the
 * diagnostics crate). The page says so once, in one note; component notes do not repeat it.
 */
const SHORT_WINDOW_S = 60;

/**
 * Diagnostics: the summary of the latest report is the focal panel (overall state from the
 * findings, counts per severity, time and coverage), then the findings by severity, then
 * coverage, limitations and export. Every non-data state (first load, never run, running,
 * error) takes the summary's place.
 */
export function DiagnosticsPage() {
  const ensureLoaded = useDiag((s) => s.ensureLoaded);
  useEffect(() => ensureLoaded(), [ensureLoaded]);
  const status = useDiag((s) => s.status);
  const report = useDiag((s) => s.report);

  return (
    <>
      <PageHeader title="Diagnostics" actions={<RunButton />} />
      <div className="@container flex min-w-0 flex-col gap-8">
        <RunError />
        {status === "ready" && report ? <ReportSwitch report={report} /> : <LoadState />}
      </div>
    </>
  );
}

export function RunButton({ label }: { label?: string }) {
  const running = useDiag((s) => s.running);
  const run = useDiag((s) => s.run);
  const t = useT("diagnostics");
  return (
    <Button variant="primary" icon="diagnostics" loading={running} onClick={() => void run()}>
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
    <Surface variant="secondary">
      <StateView
        compact
        kind={stateKindForError(error)}
        title={hasReport ? t("runFailedKeptPrevious") : t("runFailed")}
        error={error}
        onRetry={() => void run()}
      />
    </Surface>
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
      <Surface variant="primary">
        <StateView
          layout="hero"
          kind={kind}
          title={kind === "unsupported" ? t("unsupportedTitle") : t("reportUnavailable")}
          {...(kind === "unsupported" ? { description: t("unsupportedDescription") } : {})}
          error={error}
          onRetry={reload}
        />
      </Surface>
    );
  }
  if (status === "ready") {
    return (
      <Surface variant="primary">
        {running ? (
          <StateView layout="hero" kind="loading" title={t("running")} description={t("runningDescription")} />
        ) : (
          <StateView layout="hero" kind="empty" title={t("neverRun")} description={t("neverRunDescription")} />
        )}
      </Surface>
    );
  }
  return <ReportSkeleton />;
}

/** First load: the shape of the summary and of two findings. The status is read out once. */
function ReportSkeleton() {
  const t = useT("diagnostics");
  return (
    <div role="status" aria-busy="true" className="flex flex-col gap-4">
      <span className="sr-only">{t("loadingLatest")}</span>
      <Surface variant="primary" className="flex items-start gap-4">
        <Skeleton round className="size-12 shrink-0" />
        <div className="flex min-w-0 flex-1 flex-col gap-2.5 pt-1">
          <Skeleton className="h-2.5 w-14" />
          <Skeleton className="h-6 w-1/2" />
          <SkeletonLines lines={2} />
        </div>
      </Surface>
      {[0, 1].map((i) => (
        <Surface key={i} variant="secondary">
          <Skeleton round className="h-5 w-24" />
          <Skeleton className="mt-3 h-4 w-2/3" />
          <SkeletonLines lines={3} className="mt-3" />
        </Surface>
      ))}
    </div>
  );
}

function ReportSwitch({ report }: { report: DiagnosticReport }) {
  return useIsSimple() ? <SimpleReportView report={report} /> : <ReportView report={report} />;
}

/** The page's one short-window warning. Text first; the icon only reinforces it. */
function ShortWindowNote({ children }: { children: ReactNode }) {
  return (
    <div role="note" className="flex items-start gap-2 rounded-lg border border-warn/25 bg-warn-soft px-3.5 py-2.5 text-sm text-fg">
      <Icon name="alert" size={14} className="mt-0.5 shrink-0 text-warn" />
      <div className="min-w-0">{children}</div>
    </div>
  );
}

const LIST = "flex list-disc flex-col gap-0.5 pl-4 marker:text-fg-subtle";

/** "Semplice": plain summary (it carries the counts), one card per finding, compact coverage. */
function SimpleReportView({ report }: { report: DiagnosticReport }) {
  const t = useT("diagnostics");
  const findings = useMemo(() => sortFindings(report.findings), [report.findings]);
  const indexOf = useFindingIndex(report);
  const coverage = useReportCoverage(report);
  // `t` changes with the language, so the summary is rebuilt in the new language.
  const summary = useMemo(() => simpleSummary(report.findings, coverage), [report.findings, coverage, t]);
  const headingId = useId();

  return (
    <>
      <div className="flex flex-col gap-4">
        <ReportSummary
          report={report}
          headline={summary[0] ?? ""}
          lines={summary.slice(1)}
          meta={t("simpleCheckedAt", { date: formatDateTime(report.generatedAt) })}
          eyebrow={t("summaryLabel")}
          severityName={simpleSeverityLabel}
        />
        {report.observedWindowS < SHORT_WINDOW_S && <ShortWindowNote>{simpleShortWindowNote()}</ShortWindowNote>}
      </div>

      {findings.length > 0 && (
        <section aria-labelledby={headingId} className="flex flex-col gap-4">
          <SectionHeading id={headingId}>{t("whatWeFound")}</SectionHeading>
          {findings.map((f, i) => (
            <SimpleFindingCard key={`${f.ruleId}-${i}`} finding={f} findingIndex={indexOf(f)} />
          ))}
        </section>
      )}

      <DetailsGroup
        coverage={<CoverageCard items={coverage} simple />}
        limitations={
          report.limitations.length > 0 && (
            <Panel>
              <Disclosure summary={t("simpleLimitations")}>
                <ul className={`${LIST} mt-2 text-sm text-fg-muted`}>
                  {report.limitations.map((l) => (
                    <li key={l}>{l}</li>
                  ))}
                </ul>
              </Disclosure>
            </Panel>
          )
        }
      />
    </>
  );
}

/** "Avanzata": summary with coverage, then the findings grouped by severity (each heading carries its count). */
function ReportView({ report }: { report: DiagnosticReport }) {
  const t = useT("diagnostics");
  const groups = useMemo(() => groupFindings(report.findings), [report.findings]);
  const indexOf = useFindingIndex(report);
  const coverage = useReportCoverage(report);
  const state = useMemo(() => overallState(report.findings), [report.findings]);
  const baseId = useId();
  const quiet = state.counts.critical === 0 && state.counts.warning === 0;
  const coverageLine = coverageSentence(coverageSummary(coverage));

  return (
    <>
      <div className="flex flex-col gap-4">
        <ReportSummary
          report={report}
          headline={overallHeadline(state)}
          lines={quiet ? [t("noFindingsDescription"), coverageLine] : [coverageLine]}
          meta={t("reportMeta", {
            date: formatDateTime(report.generatedAt),
            rules: report.rulesVersion,
            version: report.appVersion,
          })}
          eyebrow={t("outcomeEyebrow")}
          severityName={severityGroupLabel}
        />
        {report.observedWindowS < SHORT_WINDOW_S && (
          <ShortWindowNote>{t("shortWindow", { window: formatDuration(report.observedWindowS), min: SHORT_WINDOW_S })}</ShortWindowNote>
        )}
      </div>

      {groups.map((g) => {
        const id = `${baseId}-${g.severity}`;
        return (
          <section key={g.severity} aria-labelledby={id} className="flex flex-col gap-4">
            <SectionHeading id={id} count={g.findings.length}>
              {severityGroupLabel(g.severity)}
            </SectionHeading>
            {g.findings.map((f, i) => (
              <FindingCard key={`${f.ruleId}-${i}`} finding={f} findingIndex={indexOf(f)} />
            ))}
          </section>
        );
      })}

      <DetailsGroup
        coverage={<CoverageCard items={coverage} simple={false} />}
        limitations={
          report.limitations.length > 0 && (
            <Panel title={t("limitationsTitle")}>
              <ul className={`${LIST} text-sm text-fg-muted`}>
                {report.limitations.map((l) => (
                  <li key={l}>{l}</li>
                ))}
              </ul>
            </Panel>
          )
        }
      />
    </>
  );
}

/** Coverage and limitations side by side when there is room, then the export panel. */
function DetailsGroup({ coverage, limitations }: { coverage: ReactNode; limitations: ReactNode }) {
  return (
    <div className="flex flex-col gap-4">
      <div className={`grid items-start gap-4 ${limitations ? "@4xl:grid-cols-2" : ""}`}>
        {coverage}
        {limitations}
      </div>
      <ExportPanel />
    </div>
  );
}

const TONE_GLYPH: Record<Exclude<Tone, "neutral">, { icon: IconName; className: string }> = {
  ok: { icon: "success", className: "bg-ok-soft text-ok" },
  warn: { icon: "warning", className: "bg-warn-soft text-warn" },
  bad: { icon: "error", className: "bg-bad-soft text-bad" },
};

const SEVERITY_TONE_ICON: Record<Severity, { icon: IconName; className: string }> = {
  critical: { icon: "error", className: "text-bad" },
  warning: { icon: "warning", className: "text-warn" },
  info: { icon: "info", className: "text-fg-subtle" },
};

/**
 * The focal panel: overall state in words (from the findings only, no score) with a status
 * glyph, the counts per severity once, the coverage line and the time of the run. With
 * nothing that asks for attention it becomes a centered hero ("Nessun problema rilevato").
 */
function ReportSummary({
  report,
  headline,
  lines,
  meta,
  eyebrow,
  severityName,
}: {
  report: DiagnosticReport;
  headline: string;
  lines: readonly string[];
  meta: string;
  eyebrow: string;
  severityName: (s: Severity) => string;
}) {
  const t = useT("diagnostics");
  const state = useMemo(() => overallState(report.findings), [report.findings]);
  const running = useDiag((s) => s.running);
  const headingId = useId();
  const tone = state.tone === "neutral" ? "ok" : state.tone;
  const glyph = TONE_GLYPH[tone];
  const quiet = tone === "ok";

  const counts = state.total > 0 && (
    <dl aria-label={t("countsAria")} className="grid grid-cols-3 gap-2 sm:gap-3">
      {SEVERITY_ORDER.map((s) => {
        const n = state.counts[s];
        const icon = SEVERITY_TONE_ICON[s];
        return (
          <div key={s} className="surface-inset min-w-0 px-3 py-2.5 text-left">
            <dt className="flex min-w-0 items-center gap-1.5 text-xs text-fg-muted">
              <Icon name={icon.icon} size={14} className={`shrink-0 ${n > 0 ? icon.className : "text-fg-subtle opacity-60"}`} />
              <span className="min-w-0 truncate" title={severityName(s)}>
                {severityName(s)}
              </span>
            </dt>
            <dd className="mt-0.5">
              <Readout value={n} size="sm" className={n > 0 ? "" : "text-fg-subtle!"} />
            </dd>
          </div>
        );
      })}
    </dl>
  );

  const runningNote = running && (
    <p role="status" className="flex items-center gap-2 text-xs text-fg-muted">
      <Spinner size={14} />
      {t("running")}
    </p>
  );

  if (quiet) {
    return (
      <section aria-labelledby={headingId} className="surface-primary p-5">
        <div className="flex flex-col items-center gap-2 px-2 py-6 text-center sm:px-6 sm:py-8">
          <span aria-hidden="true" className={`mb-1 grid size-14 place-items-center rounded-full ${glyph.className}`}>
            <Icon name={glyph.icon} size={28} strokeWidth={1.5} />
          </span>
          <p className="eyebrow">{eyebrow}</p>
          <h2 id={headingId} className="font-display text-2xl font-semibold text-balance break-words text-fg">
            {headline}
          </h2>
          {lines.map((line) => (
            <p key={line} className="max-w-md text-sm text-fg-muted">
              {line}
            </p>
          ))}
          <p className="mt-1 text-xs text-fg-subtle tabular">{meta}</p>
          {runningNote}
        </div>
        {counts}
      </section>
    );
  }

  return (
    <section aria-labelledby={headingId} className="surface-primary flex flex-col gap-5 p-5">
      <div className="flex items-start gap-4">
        <span aria-hidden="true" className={`grid size-12 shrink-0 place-items-center rounded-full ${glyph.className}`}>
          <Icon name={glyph.icon} size={24} strokeWidth={1.5} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1">
            <p className="eyebrow">{eyebrow}</p>
            {runningNote}
          </div>
          <h2 id={headingId} className="mt-1 font-display text-2xl font-semibold break-words text-fg">
            {headline}
          </h2>
          {lines.map((line) => (
            <p key={line} className="mt-1 max-w-prose text-sm text-fg-muted">
              {line}
            </p>
          ))}
          <p className="mt-2 text-xs text-fg-subtle tabular">{meta}</p>
        </div>
      </div>
      {counts}
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

/** The report's coverage as it applies on this platform (`platformCoverage`). */
function useReportCoverage(report: DiagnosticReport): readonly CoverageItem[] {
  const platform = usePlatform();
  return useMemo(() => platformCoverage(report.coverage, platform), [report.coverage, platform]);
}

/**
 * Coverage at a glance: one badge per component, and a note only for the components that
 * were not fully evaluated (the note says what is missing). In "Avanzata" the summary line
 * is in the summary panel, so it is not repeated here.
 */
function CoverageCard({ items, simple }: { items: readonly CoverageItem[]; simple: boolean }) {
  const t = useT("diagnostics");
  const summary = useMemo(() => coverageSummary(items), [items]);
  const name = (c: CoverageItem) => (simple ? simpleComponentName(c.component) : componentKindLabel(c.component));
  const notes = summary.gaps
    .map((c) => ({ c, note: simple ? simpleCoverageNote(c) : c.note.trim() }))
    .filter((x) => x.note !== "");

  return (
    <Panel title={simple ? t("whatWasChecked") : t("coverageTitle")}>
      {items.length === 0 ? (
        <p className="text-sm text-fg-muted">{t("coverageNotReported")}</p>
      ) : (
        <>
          <ul className="flex flex-wrap gap-1.5">
            {items.map((c, i) => (
              <li key={`${c.component}-${i}`} className="max-w-full">
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
                <li key={`${c.component}-${i}`} className="break-words">
                  <span className="font-medium text-fg">{name(c)}</span>: {note}
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </Panel>
  );
}

function ExportPanel() {
  const state = useDiag((s) => s.export);
  const exportReport = useDiag((s) => s.exportReport);
  const [redact, setRedact] = useState(true);
  const hintId = useId();
  const busy = state.status === "running";
  const t = useT("diagnostics");
  const go = async (format: ReportFormat) => {
    await exportReport(format, redact);
    // Read back the outcome of this very export (the store sets it before resolving).
    const after = diagStore.getState().export;
    if (after.status === "done" && after.format === format) toast.success(t("exportSavedToast"), after.result.path);
  };

  return (
    <Panel title={t("exportTitle")}>
      <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
        <div className="flex flex-wrap gap-2">
          {(["json", "html"] as const).map((format) => {
            const running = busy && state.format === format;
            return (
              <Button key={format} icon="download" loading={running} onClick={() => void go(format)} disabled={busy && !running}>
                {running ? t("exporting") : t("exportAs", { format: format.toUpperCase() })}
              </Button>
            );
          })}
        </div>
        <label className="flex cursor-pointer items-center gap-2.5 text-sm select-none">
          <Switch label={t("redact")} checked={redact} onChange={setRedact} describedBy={hintId} />
          {t("redact")}
        </label>
      </div>
      <p id={hintId} className="mt-2.5 max-w-prose text-xs text-fg-subtle">
        {t("exportHint")}
      </p>
      {state.status === "done" && (
        <div role="status" className="surface-inset mt-3 flex items-start gap-2 px-3 py-2.5">
          <Icon name="success" size={14} className="mt-0.5 shrink-0 text-ok" />
          <div className="min-w-0 text-sm">
            <p>
              {t(state.result.redacted ? "exportSavedRedacted" : "exportSavedPersonal", {
                format: state.format.toUpperCase(),
              })}{" "}
              · <span className="tabular">{formatStorageBytes(state.result.bytes)}</span>
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
    </Panel>
  );
}
