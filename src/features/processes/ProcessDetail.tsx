import { useViewMode } from "@/hooks/useViewMode";
import { memo, type ReactNode, useCallback, useId, useMemo, useState } from "react";
import { LineChart, type Series } from "@/components/charts/LineChart";
import type { Domain, Point } from "@/components/charts/scale";
import { Button } from "@/components/common/Button";
import { DefinitionList } from "@/components/common/DefinitionList";
import { Icon } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { Modal } from "@/components/common/Modal";
import { Readout } from "@/components/common/Readout";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { Tooltip } from "@/components/common/Tooltip";
import { UsageBar } from "@/components/common/UsageBar";
import { usePolling } from "@/hooks/usePolling";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatBytes, formatBytesPerSec, formatPercent, formatNumber } from "@/lib/format";
import { commands } from "@/services/commands";
import type { ProcessHistory } from "@/types/generated/ProcessHistory";
import type { ProcessRow } from "@/types/generated/ProcessRow";
import type { ProcessSnapshot } from "@/types/generated/ProcessSnapshot";
import { SelfTag } from "./ProcessTable";
import { useProcesses } from "./processStore";
import { categoryLabel, zeroBasedDomain } from "./rows";
import { useProcessView } from "./viewStore";

const POLL_MS = 2000;
const CHART_HEIGHT = 96;
const PERCENT_DOMAIN: Domain = [0, 100];

const bytes = (v: number) => formatBytes(v);
const pct = (v: number) => formatPercent(v, 1);
const rate = (v: number) => formatBytesPerSec(v);

function findRow(snapshot: ProcessSnapshot | null, key: string): ProcessRow | null {
  if (!snapshot) return null;
  return snapshot.rankingInstant.find((r) => r.key === key) ?? snapshot.rankingWindow.find((r) => r.key === key) ?? null;
}

/**
 * Detail of the selected process, in a trailing sheet. Opens when a row is selected; closing
 * (Esc, the close button) clears the selection and focus returns to the row. The last shown
 * process stays rendered during the exit animation, then unmounts (which stops its polling).
 */
export function ProcessDetailSheet() {
  const selected = useProcessView((s) => s.selected);
  const select = useProcessView((s) => s.select);
  const titleId = useId();
  const [shown, setShown] = useState<ProcessRow | null>(selected);
  // A new selection replaces what the sheet shows (adjusting state while rendering).
  if (selected && selected.key !== shown?.key) setShown(selected);

  return (
    <Modal
      open={selected !== null}
      onClose={() => select(null)}
      onExitComplete={() => setShown(null)}
      presentation="sheet"
      size="lg"
      closeOnBackdrop
      bare
      header={shown ? <SheetHeader key={shown.key} lastSeen={shown} titleId={titleId} /> : undefined}
      labelledBy={titleId}
    >
      {shown ? <ProcessDetail key={shown.key} lastSeen={shown} /> : null}
    </Modal>
  );
}

/** Live row of the process, or the last one seen once it leaves the ranking. */
function useLiveRow(lastSeen: ProcessRow): { row: ProcessRow; stale: boolean } {
  const current = useProcesses((s) => findRow(s.snapshot, lastSeen.key));
  return { row: current ?? lastSeen, stale: current === null };
}

/**
 * Sheet header: the full process name (wraps, never clipped), PID and category, then the
 * close button. Built here because `Modal`'s default header truncates its subtitle.
 */
function SheetHeader({ lastSeen, titleId }: { lastSeen: ProcessRow; titleId: string }) {
  const t = useT("processes");
  const common = useT("common");
  const select = useProcessView((s) => s.select);
  const { row } = useLiveRow(lastSeen);
  return (
    <header className="flex shrink-0 items-start gap-3 border-b border-line px-6 pt-5 pb-4">
      <div className="min-w-0 flex-1">
        <h2 id={titleId} className="font-display text-lg font-semibold break-words text-fg">
          {row.name}
        </h2>
        <p className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-fg-muted tabular">
          <span>
            PID {row.pid} · {categoryLabel(row.category, t)}
          </span>
          {row.isSelf && <SelfTag />}
        </p>
      </div>
      <Tooltip content={common("close")} side="left">
        <Button variant="ghost" size="sm" iconOnly icon="close" aria-label={common("close")} onClick={() => select(null)} className="-mt-0.5 -mr-2" />
      </Tooltip>
    </header>
  );
}

function ProcessDetail({ lastSeen }: { lastSeen: ProcessRow }) {
  const t = useT("processes");
  const mode = useProcessView((s) => s.mode);
  // Re-renders once per snapshot; the sheet also polls the history at the same cadence.
  const { row, stale } = useLiveRow(lastSeen);
  const fetchHistory = useCallback(() => commands.getProcessHistory(lastSeen.key), [lastSeen.key]);
  const [history, retry] = usePolling(fetchHistory, POLL_MS);
  const impact = row.pressure.impact;

  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
      <div className="flex flex-col gap-7" aria-live="off">
        {/* Focal reading: the score, with the scale written out. */}
        <div className="flex flex-col gap-2">
          <span className="text-xs text-fg-muted">Impact Score</span>
          <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
            <Readout value={formatNumber(impact, 1)} size="lg" />
            <span className="text-sm text-fg-muted tabular">
              {t("impactScale")}
              {mode === "window" && ` · ${t("peakValue", { value: formatNumber(row.impactPeak, 1) })}`}
            </span>
          </div>
          {!row.accessible && (
            <p className="flex items-start gap-1.5 text-xs text-fg-muted">
              <Icon name="lock" size={12} className="mt-0.5 shrink-0 text-fg-subtle" />
              {t("protectedDetail")}
            </p>
          )}
          {stale && <p className="text-xs text-fg-subtle">{t("staleNote")}</p>}
        </div>

        <PressureBreakdown row={row} />

        <Section label={t("nowSection")}>
          <DefinitionList
            divided
            items={[
              { label: "CPU", value: <MetricValue metric={row.cpuPercent} format={pct} /> },
              { label: t("ioReadNow"), value: <MetricValue metric={row.ioReadBytesPerSec} format={rate} /> },
              { label: t("ioWriteNow"), value: <MetricValue metric={row.ioWriteBytesPerSec} format={rate} /> },
            ]}
          />
        </Section>

        <Section label={t("memorySection")}>
          <DefinitionList
            divided
            items={[
              { label: t("workingSet"), value: <MetricValue metric={row.workingSetBytes} format={bytes} /> },
              { label: t("privateMemory"), value: <MetricValue metric={row.privateBytes} format={bytes} /> },
            ]}
          />
        </Section>

        <Section label={t("historySection")}>
          {history.status === "loading" && <StateView compact kind="loading" title={t("historyLoading")} />}
          {history.status === "error" &&
            (history.error.code === "notFound" ? (
              <StateView compact kind="empty" title={t("endedTitle")} description={t("endedDescription")} />
            ) : (
              <StateView compact kind={stateKindForError(history.error)} error={history.error} onRetry={retry} />
            ))}
          {history.status === "ready" && <HistoryView history={history.data} />}
        </Section>

        <Section label={t("exePathSection")} ariaLabel={t("localInfoAria")} className="border-t border-line pt-5">
          <div className="surface-inset px-3 py-2 font-mono text-xs break-all text-fg">
            {row.exePath ?? <span className="font-sans text-fg-subtle">{t("exePathUnavailable")}</span>}
          </div>
          <p className="mt-2 text-2xs text-fg-subtle">{t("localInfoNote")}</p>
        </Section>
      </div>
    </div>
  );
}

/** A labelled block of the detail sheet: `eyebrow` heading, named region. */
function Section({
  label,
  ariaLabel,
  className = "",
  children,
}: {
  label: string;
  /** Accessible name when it differs from the visible label. */
  ariaLabel?: string;
  className?: string;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section {...(ariaLabel ? { "aria-label": ariaLabel } : { "aria-labelledby": id })} className={className}>
      <h3 id={id} className="eyebrow mb-2.5">
        {label}
      </h3>
      {children}
    </section>
  );
}

function PressureBreakdown({ row }: { row: ProcessRow }) {
  const t = useT("processes");
  const items = [
    { label: "CPU", value: row.pressure.cpu },
    { label: "RAM", value: row.pressure.ram },
    { label: "I/O", value: row.pressure.io },
  ];
  return (
    <Section label={t("pressureSection")} ariaLabel={t("pressureAria")}>
      <ul className="flex flex-col gap-2.5">
        {items.map((i) => (
          <li key={i.label} className="grid grid-cols-[4rem_minmax(0,1fr)_2.75rem] items-center gap-3 text-sm">
            <span className="text-fg-muted">{i.label}</span>
            <UsageBar
              percent={i.value}
              label={t("pressureOf", { label: i.label })}
              valueText={t("outOf100", { value: formatNumber(i.value, 1) })}
            />
            <span className="text-right tabular" aria-hidden="true">
              {formatNumber(i.value, 1)}
            </span>
          </li>
        ))}
      </ul>
    </Section>
  );
}

const HistoryView = memo(function HistoryView({ history }: { history: ProcessHistory }) {
  // Byte labels follow the view mode (lib/format.ts): re-render when it changes.
  useViewMode();
  const t = useT("processes");
  const { points } = history;
  const charts = useMemo(() => {
    const cpu: Point[] = [];
    const ram: Point[] = [];
    const read: Point[] = [];
    const write: Point[] = [];
    for (const p of points) {
      cpu.push({ t: p.at, v: p.cpuPercent });
      ram.push({ t: p.at, v: p.workingSetBytes });
      read.push({ t: p.at, v: p.ioReadBytesPerSec });
      write.push({ t: p.at, v: p.ioWriteBytesPerSec });
    }
    const first = points[0]?.at ?? 0;
    const last = points[points.length - 1]?.at ?? 0;
    const xDomain: Domain = last > first ? [first, last] : [last - POLL_MS, last];
    return {
      xDomain,
      cpu: [{ id: "cpu", label: "CPU", points: cpu }] satisfies Series[],
      ram: [{ id: "ram", label: t("workingSet"), points: ram }] satisfies Series[],
      io: [
        { id: "write", label: t("seriesWrite"), points: write },
        { id: "read", label: t("seriesRead"), points: read, tone: "secondary" },
      ] satisfies Series[],
      ramDomain: zeroBasedDomain(
        ram.map((p) => p.v),
        1024 ** 2,
      ),
      ioDomain: zeroBasedDomain([...read, ...write].map((p) => p.v), 1024),
    };
  }, [points, t]);

  return (
    <div className="flex flex-col gap-4">
      {!history.running && (
        <div>
          <StatusBadge tone="neutral" label={t("historyEnded")} />
        </div>
      )}
      {points.length === 0 ? (
        <p className="text-sm text-fg-subtle">{t("noSamples")}</p>
      ) : (
        <>
          <MiniChart title={t("cpuChartTitle")}>
            <LineChart
              series={charts.cpu}
              xDomain={charts.xDomain}
              yDomain={PERCENT_DOMAIN}
              formatY={(v) => formatPercent(v)}
              height={CHART_HEIGHT}
              ariaLabel={t("cpuChartAria")}
            />
          </MiniChart>
          <MiniChart title={t("ramChartTitle")}>
            <LineChart
              series={charts.ram}
              xDomain={charts.xDomain}
              yDomain={charts.ramDomain}
              formatY={(v) => formatBytes(v, 0)}
              height={CHART_HEIGHT}
              ariaLabel={t("ramChartAria")}
            />
          </MiniChart>
          <MiniChart title={t("ioChartTitle")} legend={charts.io}>
            <LineChart
              series={charts.io}
              xDomain={charts.xDomain}
              yDomain={charts.ioDomain}
              formatY={(v) => formatBytesPerSec(v, 0)}
              height={CHART_HEIGHT}
              ariaLabel={t("ioChartAria")}
            />
          </MiniChart>
        </>
      )}
    </div>
  );
});

/**
 * Chart with a caption. Multi-series charts get a key drawn like the lines themselves
 * (solid, dashed); it is decorative, since the caption already names them.
 */
function MiniChart({
  title,
  legend,
  children,
}: {
  title: string;
  legend?: readonly Series[];
  children: ReactNode;
}) {
  return (
    <figure>
      <figcaption className="mb-1.5 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5">
        <span className="text-xs text-fg-muted">{title}</span>
        {legend && (
          <span aria-hidden="true" className="flex items-center gap-3 text-2xs text-fg-subtle">
            {legend.map((s) => (
              <span key={s.id} className="inline-flex items-center gap-1.5">
                <LineSwatch secondary={s.tone === "secondary"} />
                {s.label}
              </span>
            ))}
          </span>
        )}
      </figcaption>
      {children}
    </figure>
  );
}

/** Same stroke as the chart's series: solid for primary, dashed for secondary. */
function LineSwatch({ secondary }: { secondary: boolean }) {
  return (
    <svg width="14" height="4" viewBox="0 0 14 4" className="shrink-0">
      <line
        x1={1}
        x2={13}
        y1={2}
        y2={2}
        stroke={secondary ? "var(--meter)" : "var(--fg-muted)"}
        strokeWidth={secondary ? 1.25 : 1.5}
        strokeDasharray={secondary ? "3 2" : undefined}
        strokeLinecap="round"
      />
    </svg>
  );
}
