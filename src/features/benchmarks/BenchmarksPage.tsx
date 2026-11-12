import { AnimatePresence, m } from "motion/react";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { UnavailableReason } from "@/components/common/MetricValue";
import { Segmented } from "@/components/common/Segmented";
import { Skeleton } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { PageHeader } from "@/components/layout/PageHeader";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatUnitValue } from "@/lib/format";
import { duration, ease, pageVariants } from "@/lib/motion";
import { useInventory } from "@/features/monitoring/inventoryStore";
import type { BenchmarkCapability } from "@/types/generated/BenchmarkCapability";
import type { BenchmarkKind } from "@/types/generated/BenchmarkKind";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";
import { useBench } from "./benchStore";
import { ComparisonPanel } from "./ComparisonPanel";
import { componentSummary, DetectedComponent, useEnsureInventory } from "./DetectedComponent";
import { LiveProgress } from "./LiveProgress";
import {
  BENCH_COMPONENTS,
  type BenchComponent,
  componentLabel,
  componentOfKind,
  headlineMetric,
  isAvailable,
  kindLabel,
  stateLabel,
} from "./model";
import { ResultView } from "./ResultView";
import { TestPanel } from "./TestPanel";
import { PANEL, PANEL_TITLE } from "./ui";
import { useBenchView } from "./viewStore";

const NO_CAPS: readonly BenchmarkCapability[] = [];

/** Entrance of a panel that appears in response to a run starting or finishing. */
const panelIn = {
  initial: { opacity: 0, y: 6 },
  animate: { opacity: 1, y: 0, transition: { duration: duration.component, ease: ease.out } },
} as const;

/** Benchmarks: nothing starts on entering a page or a screen; every run is explicit. */
export function BenchmarksPage() {
  const component = useBenchView((s) => s.component);
  const ensureCapabilities = useBench((s) => s.ensureCapabilities);
  useEffect(() => ensureCapabilities(), [ensureCapabilities]);
  useEnsureInventory();

  return (
    <>
      <PageHeader title="Benchmarks" />
      <CapabilitiesGate>
        {/* Grid <-> component screen: a short cross-fade on user navigation only. */}
        <AnimatePresence mode="wait" initial={false}>
          <m.div key={component ?? "grid"} variants={pageVariants} initial="initial" animate="enter" exit="exit">
            {component === null ? <ComponentGrid /> : <ComponentScreen component={component} />}
          </m.div>
        </AnimatePresence>
      </CapabilitiesGate>
    </>
  );
}

function CapabilitiesGate({ children }: { children: ReactNode }) {
  const t = useT("benchmarks");
  const caps = useBench((s) => s.capabilities);
  const reload = useBench((s) => s.reloadCapabilities);
  if (caps.status === "ready") return <>{children}</>;
  if (caps.status === "error") {
    const kind = stateKindForError(caps.error);
    return (
      <Card>
        <StateView
          kind={kind}
          title={kind === "unsupported" ? t("unsupportedTitle") : t("listUnavailable")}
          {...(kind === "unsupported" ? { description: t("unsupportedDesc") } : {})}
          error={caps.error}
          onRetry={reload}
        />
      </Card>
    );
  }
  return (
    <Card>
      <StateView kind="loading" title={t("loadingTests")} />
    </Card>
  );
}

function useComponentCaps(component: BenchComponent): readonly BenchmarkCapability[] {
  const caps = useBench((s) => (s.capabilities.status === "ready" ? s.capabilities.data : NO_CAPS));
  return useMemo(() => {
    const kinds = BENCH_COMPONENTS.find((c) => c.id === component)?.kinds ?? [];
    return kinds.flatMap((k) => caps.filter((c) => c.kind === k));
  }, [caps, component]);
}

function ComponentGrid() {
  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
      {BENCH_COMPONENTS.map((c) => (
        <ComponentCard key={c.id} component={c.id} />
      ))}
    </div>
  );
}

function ComponentCard({ component }: { component: BenchComponent }) {
  const t = useT("benchmarks");
  const open = useBenchView((s) => s.open);
  const caps = useComponentCaps(component);
  const summary = useInventory((s) => (s.data ? componentSummary(s.data, component, t) : null));
  const inventoryFailed = useInventory((s) => s.data === null && s.status === "error");
  const running = useBench((s) => (s.activeRun && componentOfKind(s.activeRun.kind) === component ? s.activeRun.kind : null));
  const available = caps.filter(isAvailable);
  const unavailable = caps.find((c) => c.availability.status === "unavailable");

  return (
    <button
      type="button"
      onClick={() => open(component)}
      className={`group pressable flex flex-col items-stretch gap-3 p-4 text-left hover:border-line-strong hover:bg-surface-2/60 ${PANEL}`}
    >
      <span className="flex items-start justify-between gap-3">
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className={PANEL_TITLE}>{componentLabel(component)}</span>
          {summary !== null ? (
            <span className="text-[13px] break-words text-fg-muted">{summary}</span>
          ) : inventoryFailed ? (
            <span className="text-[13px] text-fg-subtle">{t("detectionFailed")}</span>
          ) : (
            <span className="flex h-5 items-center">
              <Skeleton className="h-3 w-40" />
              <span className="sr-only">{t("detecting")}</span>
            </span>
          )}
        </span>
        <span className="flex shrink-0 items-center gap-2 pt-0.5">
          {running ? (
            <StatusBadge tone="neutral" label={t("runningKind", { kind: kindLabel(running, t) })} />
          ) : (
            available.length === 0 && <StatusBadge tone="neutral" label={t("notAvailable")} />
          )}
          <Icon
            name="chevronRight"
            className="text-fg-subtle transition-[translate,color] duration-150 ease-out group-hover:translate-x-0.5 group-hover:text-fg-muted"
          />
        </span>
      </span>

      {caps.length === 0 && <span className="text-xs text-fg-subtle">{t("noTestsInVersion")}</span>}
      {available.length === 0 && unavailable?.availability.status === "unavailable" && (
        <UnavailableReason className="text-xs text-fg-subtle" reason={unavailable.availability.reason} />
      )}
      {available.length > 0 && (
        <span className="text-xs text-fg-subtle">{available.map((c) => kindLabel(c.kind, t)).join(" · ")}</span>
      )}
      <LastHeadline component={component} />
    </button>
  );
}

function LastHeadline({ component }: { component: BenchComponent }) {
  const t = useT("benchmarks");
  const last = useBench((s) => {
    const kinds = BENCH_COMPONENTS.find((c) => c.id === component)?.kinds ?? [];
    let best: BenchmarkResult | null = null;
    for (const k of kinds) {
      const r = s.lastResultByKind[k];
      if (r && (!best || r.finishedAt > best.finishedAt)) best = r;
    }
    return best;
  });
  if (!last) return null;
  const head = headlineMetric(last);
  return (
    <span className="mt-auto flex flex-wrap items-baseline gap-x-1.5 border-t border-line pt-3 text-xs text-fg-muted tabular">
      <span className="text-fg-subtle">{t("lastRun")}</span>
      <span>
        {kindLabel(last.kind, t)} ·{" "}
        {head ? (
          <>
            {head.label} <span className="font-medium text-fg">{formatUnitValue(head.value, head.unit)}</span>
          </>
        ) : (
          stateLabel(last.state, t)
        )}
      </span>
    </span>
  );
}

function ComponentScreen({ component }: { component: BenchComponent }) {
  const t = useT("benchmarks");
  const open = useBenchView((s) => s.open);
  const caps = useComponentCaps(component);
  const activeKind = useBench((s) => s.activeRun?.kind ?? null);
  // Primitive: stable for the whole run, so progress events do not re-render this screen.
  const liveRunId = useBench((s) =>
    s.activeRun && componentOfKind(s.activeRun.kind) === component ? s.activeRun.runId : null,
  );
  const [kind, setKind] = useState<BenchmarkKind | null>(
    () =>
      (activeKind && componentOfKind(activeKind) === component ? activeKind : null) ??
      caps.find(isAvailable)?.kind ??
      caps[0]?.kind ??
      null,
  );
  const cap = caps.find((c) => c.kind === kind) ?? caps[0] ?? null;
  const options = useMemo(() => caps.map((c) => ({ value: c.kind, label: kindLabel(c.kind, t) })), [caps, t]);
  const setSection = useAppStore((s) => s.setSection);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <Button variant="ghost" className="-ml-3" onClick={() => open(null)}>
          <Icon name="chevronRight" className="rotate-180" />
          {t("allComponents")}
        </Button>
        <Button variant="ghost" className="-mr-3" onClick={() => setSection("history")}>
          <Icon name="history" />
          {t("history")}
        </Button>
      </div>

      <Card title={componentLabel(component)}>
        <DetectedComponent component={component} />
      </Card>

      {caps.length === 0 || !cap ? (
        <Card>
          <StateView kind="notImplemented" title={t("noTestsForComponent")} />
        </Card>
      ) : (
        <>
          {caps.length > 1 && (
            <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
              <span className="text-xs text-fg-muted">{t("testType")}</span>
              <Segmented label={t("testType")} options={options} value={cap.kind} onChange={setKind} />
            </div>
          )}
          {/* Sibling keys must differ: both remount per kind. */}
          <TestPanel key={`test-${cap.kind}`} cap={cap} />
          {/* Appears when a run starts (not when returning to a screen mid-run). */}
          <AnimatePresence initial={false}>
            {liveRunId !== null && (
              <m.div key={liveRunId} {...panelIn}>
                <LiveProgress component={component} />
              </m.div>
            )}
          </AnimatePresence>
          <LatestResult key={`result-${cap.kind}`} kind={cap.kind} />
        </>
      )}
    </div>
  );
}

/** Remounted per kind (key), so only a result that arrives while the screen is open fades in. */
function LatestResult({ kind }: { kind: BenchmarkKind }) {
  const t = useT("benchmarks");
  const result = useBench((s) => s.lastResultByKind[kind] ?? null);
  const runningThis = useBench((s) => s.activeRun?.kind === kind);
  const visible = result && !runningThis ? result : null;
  return (
    <AnimatePresence initial={false}>
      {visible && (
        <m.div key={visible.runId} {...panelIn}>
          <Card title={t("result")}>
            <ResultView
              result={visible}
              comparison={<ComparisonPanel key={visible.runId} currentRunId={visible.runId} />}
            />
          </Card>
        </m.div>
      )}
    </AnimatePresence>
  );
}
