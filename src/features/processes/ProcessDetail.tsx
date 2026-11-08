import { useViewMode } from "@/hooks/useViewMode";
import { memo, type ReactNode, useCallback, useId, useMemo } from "react";
import { LineChart, type Series } from "@/components/charts/LineChart";
import type { Domain, Point } from "@/components/charts/scale";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { DefinitionList } from "@/components/common/DefinitionList";
import { Icon } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { UsageBar } from "@/components/common/UsageBar";
import { usePolling } from "@/hooks/usePolling";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatBytes, formatBytesPerSec, formatPercent } from "@/lib/format";
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

function findRow(snapshot: ProcessSnapshot | null, key: string): ProcessRow | null {
  if (!snapshot) return null;
  return snapshot.rankingInstant.find((r) => r.key === key) ?? snapshot.rankingWindow.find((r) => r.key === key) ?? null;
}

/** Detail of the selected process. Keyed by process key, so state resets on selection. */
export function ProcessDetailPanel() {
  const t = useT("processes");
  const selected = useProcessView((s) => s.selected);
  if (!selected) {
    return (
      <Card title={t("detailTitle")}>
        <p className="text-sm text-fg-muted">{t("detailEmpty")}</p>
      </Card>
    );
  }
  return <ProcessDetail key={selected.key} lastSeen={selected} />;
}

function ProcessDetail({ lastSeen }: { lastSeen: ProcessRow }) {
  const t = useT("processes");
  const common = useT("common");
  const select = useProcessView((s) => s.select);
  // Re-renders once per snapshot; the panel also polls at the same cadence.
  const current = useProcesses((s) => findRow(s.snapshot, lastSeen.key));
  const row = current ?? lastSeen;
  const fetchHistory = useCallback(() => commands.getProcessHistory(lastSeen.key), [lastSeen.key]);
  const [history, retry] = usePolling(fetchHistory, POLL_MS);

  return (
    <Card
      title={row.name}
      aside={
        <Button variant="ghost" size="sm" onClick={() => select(null)} className="-my-1 -mr-2">
          <Icon name="close" size={14} />
          {common("close")}
        </Button>
      }
    >
      <div className="flex flex-col gap-5" aria-live="off">
        <p className="-mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs break-all text-fg-subtle tabular">
          <span>
            PID {row.pid} · {categoryLabel(row.category, t)}
            {!row.accessible && ` · ${t("protectedDetail")}`}
          </span>
          {row.isSelf && <SelfTag />}
        </p>

        {history.status === "loading" && <StateView compact kind="loading" title={t("historyLoading")} />}
        {history.status === "error" &&
          (history.error.code === "notFound" ? (
            <StateView
              compact
              kind="empty"
              title={t("endedTitle")}
              description={t("endedDescription")}
            />
          ) : (
            <StateView compact kind={stateKindForError(history.error)} error={history.error} onRetry={retry} />
          ))}
        {history.status === "ready" && <HistoryView history={history.data} />}

        <PressureBreakdown row={row} stale={current === null} />

        <Section label={t("memorySection")}>
          <DefinitionList
            items={[
              { label: "Working set", value: <MetricValue metric={row.workingSetBytes} format={bytes} /> },
              { label: t("privateMemory"), value: <MetricValue metric={row.privateBytes} format={bytes} /> },
            ]}
          />
        </Section>

        <Section label={t("exePathSection")} ariaLabel={t("localInfoAria")} className="border-t border-line pt-4">
          <div className="font-mono text-xs break-all text-fg">
            {row.exePath ?? <span className="font-sans text-fg-subtle">{t("exePathUnavailable")}</span>}
          </div>
          <p className="mt-1.5 text-2xs text-fg-subtle">{t("localInfoNote")}</p>
        </Section>
      </div>
    </Card>
  );
}

/** A labelled block of the detail panel: `eyebrow` label, named region. */
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
    <section
      {...(ariaLabel ? { "aria-label": ariaLabel } : { "aria-labelledby": id })}
      className={className}
    >
      <h4 id={id} className="eyebrow mb-2">
        {label}
      </h4>
      {children}
    </section>
  );
}

function PressureBreakdown({ row, stale }: { row: ProcessRow; stale: boolean }) {
  const t = useT("processes");
  const items = [
    { label: "CPU", value: row.pressure.cpu },
    { label: "RAM", value: row.pressure.ram },
    { label: "I/O", value: row.pressure.io },
    { label: "Impact Score", value: row.pressure.impact },
  ];
  return (
    <Section label={t("pressureSection")} ariaLabel={t("pressureAria")}>
      <ul className="flex flex-col gap-2">
        {items.map((i) => (
          <li key={i.label} className="grid grid-cols-[6.5rem_1fr_2.5rem] items-center gap-3 text-sm">
            <span className={i.label === "Impact Score" ? "font-medium text-fg" : "text-fg-muted"}>{i.label}</span>
            <UsageBar
              percent={i.value}
              label={t("pressureOf", { label: i.label })}
              valueText={t("outOf100", { value: i.value.toFixed(1) })}
            />
            <span className="text-right tabular" aria-hidden="true">
              {i.value.toFixed(1)}
            </span>
          </li>
        ))}
      </ul>
      {stale && (
        <p className="mt-2 text-2xs text-fg-subtle">{t("staleNote")}</p>
      )}
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
      ram: [{ id: "ram", label: "Working set", points: ram }] satisfies Series[],
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
