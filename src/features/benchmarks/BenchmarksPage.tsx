import { AnimatePresence, m } from "motion/react";
import { type ReactNode, useEffect, useId, useMemo, useState } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { UnavailableReason } from "@/components/common/MetricValue";
import { Panel } from "@/components/common/Panel";
import { Readout } from "@/components/common/Readout";
import { Segmented } from "@/components/common/Segmented";
import { Skeleton } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { StatusBadge } from "@/components/common/StatusBadge";
import { Tooltip } from "@/components/common/Tooltip";
import { PageHeader } from "@/components/layout/PageHeader";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime, formatUnitValue } from "@/lib/format";
import { duration, ease, pageVariants } from "@/lib/motion";
import { useInventory } from "@/features/monitoring/inventoryStore";
import type { BenchmarkCapability } from "@/types/generated/BenchmarkCapability";
import type { BenchmarkKind } from "@/types/generated/BenchmarkKind";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";
import { benchStore, useBench } from "./benchStore";
import { ComparisonPanel } from "./ComparisonPanel";
import { componentSummary, DetectedComponent, useEnsureInventory } from "./DetectedComponent";
import { LiveProgress } from "./LiveProgress";
import {
  BENCH_COMPONENTS,
  type BenchComponent,
  componentAvailability,
  componentLabel,
  componentOfKind,
  headlineMetric,
  isAvailable,
  kindLabel,
  latestComponentResult,
  splitUnitValue,
  stateLabel,
  stateTone,
} from "./model";
import { ResultMeta, ResultView } from "./ResultView";
import { BenchIllustration } from "./Illustration";
import { TestPanel } from "./TestPanel";
import { COMPONENT_ICON } from "./ui";
import { useBenchView } from "./viewStore";

const NO_CAPS: readonly BenchmarkCapability[] = [];

/** Entrance of a block that appears in response to a run starting or finishing. */
const panelIn = {
  initial: { opacity: 0, y: 6 },
  animate: {
    opacity: 1,
    y: 0,
    transition: { duration: duration.component, ease: ease.out },
  },
} as const;

/**
 * Benchmarks: selection → configuration → start → progress / Stop → result → comparison.
 * Nothing starts on entering the page or a screen; every run is explicit.
 */
export function BenchmarksPage() {
  const t = useT("benchmarks");
  const component = useBenchView((s) => s.component);
  const open = useBenchView((s) => s.open);
  const setSection = useAppStore((s) => s.setSection);
  const ensureCapabilities = useBench((s) => s.ensureCapabilities);
  useEffect(() => ensureCapabilities(), [ensureCapabilities]);
  useEnsureInventory();

  return (
    <>
      <PageHeader
        title="Benchmarks"
        leading={
          component !== null && (
            <Tooltip content={t("allComponents")} side="bottom">
              <Button variant="ghost" iconOnly icon="chevronLeft" aria-label={t("allComponents")} onClick={() => open(null)} />
            </Tooltip>
          )
        }
        actions={
          <Button variant="ghost" icon="history" onClick={() => setSection("history")}>
            {t("history")}
          </Button>
        }
      />
      <ConnectionNotice />
      <CapabilitiesGate>
        {component !== null && <ComponentPicker value={component} />}
        {/* Overview <-> component screen: a short cross-fade on user navigation only. */}
        <AnimatePresence mode="wait" initial={false}>
          <m.div key={component ?? "grid"} variants={pageVariants} initial="initial" animate="enter" exit="exit">
            {component === null ? <ComponentGrid /> : <ComponentScreen component={component} />}
          </m.div>
        </AnimatePresence>
      </CapabilitiesGate>
    </>
  );
}

/**
 * Progress and results arrive as events. If the subscription failed, say so (with a retry)
 * instead of letting a run look frozen. Outside the desktop app the capability gate already
 * explains that benchmarks are unsupported, so it is not repeated.
 */
function ConnectionNotice() {
  const t = useT("benchmarks");
  const error = useBench((s) => s.connectError);
  const capsFailed = useBench((s) => s.capabilities.status === "error");
  if (!error || capsFailed) return null;
  return (
    <Panel className="mb-4">
      <StateView
        compact
        kind="disconnected"
        title={t("eventsLost")}
        description={t("eventsLostDesc")}
        error={{ ...error, retriable: true }}
        onRetry={() => void benchStore.getState().start()}
      />
    </Panel>
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
      <Panel variant="primary">
        <StateView
          layout="hero"
          kind={kind}
          title={kind === "unsupported" ? t("unsupportedTitle") : t("listUnavailable")}
          {...(kind === "unsupported" ? { description: t("unsupportedDesc") } : {})}
          error={caps.error}
          onRetry={reload}
        />
      </Panel>
    );
  }
  // First load: placeholders shaped like the component tiles.
  return (
    <div role="status" aria-busy="true" className="flex flex-col gap-4">
      <span className="sr-only">{t("loadingTests")}</span>
      <Skeleton className="h-6 w-32" />
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {BENCH_COMPONENTS.map((c) => (
          <div key={c.id} className="surface-secondary flex h-44 flex-col gap-3 p-4">
            <Skeleton className="size-9" round />
            <Skeleton className="h-4 w-16" />
            <Skeleton className="h-3 w-3/4" />
          </div>
        ))}
      </div>
    </div>
  );
}

function useComponentCaps(component: BenchComponent): readonly BenchmarkCapability[] {
  const caps = useBench((s) => (s.capabilities.status === "ready" ? s.capabilities.data : NO_CAPS));
  return useMemo(() => {
    const kinds = BENCH_COMPONENTS.find((c) => c.id === component)?.kinds ?? [];
    return kinds.flatMap((k) => caps.filter((c) => c.kind === k));
  }, [caps, component]);
}

// ---------------------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------------------

/** Landing view: the four components as tiles, each with its device, availability and last run. */
function ComponentGrid() {
  const t = useT("benchmarks");
  const headingId = useId();
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4">
      <div className="flex flex-col gap-1">
        <SectionHeading id={headingId}>{t("componentsHeading")}</SectionHeading>
        <p className="max-w-prose text-sm text-fg-muted">{t("chooseHint")}</p>
      </div>
      <ul className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {BENCH_COMPONENTS.map((c) => (
          <li key={c.id} className="flex min-w-0">
            <ComponentTile component={c.id} />
          </li>
        ))}
      </ul>
      <RecentResults />
    </section>
  );
}

function ComponentTile({ component }: { component: BenchComponent }) {
  const t = useT("benchmarks");
  const open = useBenchView((s) => s.open);
  const caps = useComponentCaps(component);
  const summary = useInventory((s) => (s.data ? componentSummary(s.data, component, t) : null));
  const inventoryFailed = useInventory((s) => s.data === null && s.status === "error");
  const running = useBench((s) => (s.activeRun && componentOfKind(s.activeRun.kind) === component ? s.activeRun.kind : null));
  const availability = componentAvailability(caps);

  return (
    <button
      type="button"
      onClick={() => open(component)}
      className="group widget pressable relative flex w-full min-w-0 flex-col items-stretch gap-3 p-5 text-left transition-[border-color,box-shadow] duration-200 hover:border-line-strong hover:shadow-lg focus-visible:outline-none"
    >
      <span aria-hidden="true" className="-mx-1 -mt-1 block h-32">
        <BenchIllustration component={component} label="" active={running !== null} />
      </span>
      <span className="absolute top-4 right-4 flex min-w-0 flex-wrap items-center justify-end gap-2">
        {running ? (
          <StatusBadge tone="neutral" label={t("runningKind", { kind: kindLabel(running, t) })} />
        ) : (
          availability.status !== "available" && (
            <StatusBadge
              tone="neutral"
              label={
                availability.status === "unavailable" && availability.reason?.kind === "notSupported"
                  ? t("unsupportedShort")
                  : t("notAvailable")
              }
            />
          )
        )}
      </span>

      <span className="flex min-w-0 flex-col gap-0.5">
        <span className="flex items-center gap-1 font-display text-lg font-semibold text-fg">
          {componentLabel(component)}
          <Icon
            name="chevronRight"
            size={14}
            className="text-fg-subtle transition-[translate,color] duration-150 ease-out group-hover:translate-x-0.5 group-hover:text-fg-muted"
          />
        </span>
        {summary !== null ? (
          <span className="line-clamp-2 text-sm break-words text-fg-muted">{summary}</span>
        ) : inventoryFailed ? (
          <span className="text-sm text-fg-subtle">{t("detectionFailed")}</span>
        ) : (
          <span className="flex h-5 items-center">
            <Skeleton className="h-3 w-32" />
            <span className="sr-only">{t("detecting")}</span>
          </span>
        )}
      </span>

      {availability.status === "none" && <span className="text-xs text-fg-subtle">{t("noTestsInVersion")}</span>}
      {availability.status === "unavailable" && availability.reason && (
        <UnavailableReason className="text-xs text-fg-subtle" reason={availability.reason} />
      )}
      {availability.status === "available" && (
        <span className="text-xs text-fg-subtle">{availability.kinds.map((k) => kindLabel(k, t)).join(" · ")}</span>
      )}
      <LastHeadline component={component} />
    </button>
  );
}

function LastHeadline({ component }: { component: BenchComponent }) {
  const t = useT("benchmarks");
  const last = useBench((s) => latestComponentResult(s.lastResultByKind, component));
  if (!last) return null;
  const head = headlineMetric(last);
  const value = head ? splitUnitValue(formatUnitValue(head.value, head.unit)) : null;
  return (
    <span className="mt-auto flex min-w-0 flex-col gap-0.5 border-t border-line pt-3">
      <span className="text-2xs text-fg-subtle">
        {t("lastRun")} {kindLabel(last.kind, t)}
      </span>
      {head && value ? (
        <span className="flex min-w-0 flex-wrap items-baseline gap-x-2">
          <Readout size="sm" value={value.value} {...(value.unit ? { unit: value.unit } : {})} />
          <span className="truncate text-xs text-fg-muted">{head.label}</span>
        </span>
      ) : (
        <span>
          <StatusBadge tone={stateTone(last.state)} label={stateLabel(last.state, t)} glyph size="sm" />
        </span>
      )}
    </span>
  );
}

/**
 * The latest result of every test kind, newest first: one row each with the component's art,
 * the headline value and when it ran. Opens that component's screen. Before any run, a short
 * explanation of how the tests work takes the place of the list.
 */
function RecentResults() {
  const t = useT("benchmarks");
  const headingId = useId();
  const open = useBenchView((s) => s.open);
  const byKind = useBench((s) => s.lastResultByKind);
  const results = useMemo(
    () =>
      Object.values(byKind)
        .filter((r): r is BenchmarkResult => r !== undefined)
        .sort((a, b) => b.startedAt - a.startedAt),
    [byKind],
  );
  return (
    <section aria-labelledby={headingId} className="mt-6 flex flex-col gap-4">
      <SectionHeading id={headingId}>{t("recentResults")}</SectionHeading>
      {results.length === 0 ? (
        <div className="widget grid grid-cols-1 gap-6 p-6 md:grid-cols-3">
          {(["howStep1", "howStep2", "howStep3"] as const).map((k, i) => (
            <div key={k} className="flex min-w-0 gap-3">
              <span className="satin grid size-8 shrink-0 place-items-center rounded-full text-sm font-semibold text-fg tabular">
                {i + 1}
              </span>
              <p className="text-sm text-fg-muted">{t(k)}</p>
            </div>
          ))}
        </div>
      ) : (
        <ul className="widget flex flex-col p-2">
          {results.map((r) => {
            const component = componentOfKind(r.kind);
            const head = headlineMetric(r);
            const value = head ? splitUnitValue(formatUnitValue(head.value, head.unit)) : null;
            return (
              <li key={r.runId}>
                <button
                  type="button"
                  onClick={() => open(component)}
                  className="group grid w-full grid-cols-[3.5rem_minmax(0,1fr)_auto] items-center gap-4 rounded-xl px-3 py-2 text-left transition-colors duration-150 hover:bg-control-hover"
                >
                  <span aria-hidden="true" className="block h-10">
                    <BenchIllustration component={component} label="" />
                  </span>
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate text-sm font-medium text-fg">{kindLabel(r.kind, t)}</span>
                    <span className="truncate text-xs text-fg-subtle tabular">{formatDateTime(r.startedAt)}</span>
                  </span>
                  <span className="flex min-w-0 items-center gap-3">
                    {head && value ? (
                      <span className="flex flex-col items-end">
                        <Readout size="sm" value={value.value} {...(value.unit ? { unit: value.unit } : {})} />
                        <span className="truncate text-2xs text-fg-subtle">{head.label}</span>
                      </span>
                    ) : (
                      <StatusBadge tone={stateTone(r.state)} label={stateLabel(r.state, t)} glyph size="sm" />
                    )}
                    <Icon name="chevronRight" size={14} className="text-fg-subtle" />
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

/** On a component screen: the same four components as one segmented control. */
function ComponentPicker({ value }: { value: BenchComponent }) {
  const t = useT("benchmarks");
  const open = useBenchView((s) => s.open);
  const options = useMemo(
    () =>
      BENCH_COMPONENTS.map((c) => ({
        value: c.id,
        label: c.label,
        icon: COMPONENT_ICON[c.id],
      })),
    [],
  );
  return (
    <div className="mb-6 flex min-w-0">
      <Segmented label={t("componentsHeading")} options={options} value={value} onChange={open} className="max-w-full" />
    </div>
  );
}

// ---------------------------------------------------------------------------------------
// Component screen
// ---------------------------------------------------------------------------------------

function ComponentScreen({ component }: { component: BenchComponent }) {
  const t = useT("benchmarks");
  const caps = useComponentCaps(component);
  const activeKind = useBench((s) => s.activeRun?.kind ?? null);
  // Primitive: stable for the whole run, so progress events do not re-render this screen.
  const liveRunId = useBench((s) => (s.activeRun && componentOfKind(s.activeRun.kind) === component ? s.activeRun.runId : null));
  const [kind, setKind] = useState<BenchmarkKind | null>(
    () =>
      (activeKind && componentOfKind(activeKind) === component ? activeKind : null) ??
      caps.find(isAvailable)?.kind ??
      caps[0]?.kind ??
      null,
  );
  const cap = caps.find((c) => c.kind === kind) ?? caps[0] ?? null;
  const options = useMemo(() => caps.map((c) => ({ value: c.kind, label: kindLabel(c.kind, t) })), [caps, t]);
  const titleId = useId();

  return (
    <div className="flex flex-col gap-8">
      <div className="grid grid-cols-1 items-start gap-4 xl:grid-cols-[minmax(0,1fr)_20rem]">
        {caps.length === 0 || !cap ? (
          <Panel variant="primary">
            <StateView layout="hero" kind="notImplemented" title={t("noTestsForComponent")} />
          </Panel>
        ) : (
          <Panel
            variant="primary"
            className="min-w-0"
            eyebrow={componentLabel(component)}
            title={kindLabel(cap.kind, t)}
            titleId={titleId}
            aside={<span className="text-2xs text-fg-subtle tabular">{t("methodologyVersion", { version: cap.methodologyVersion })}</span>}
          >
            <div className="flex flex-col gap-5">
              {caps.length > 1 && (
                <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-2">
                  <span className="text-xs text-fg-muted">{t("testType")}</span>
                  <Segmented label={t("testType")} options={options} value={cap.kind} onChange={setKind} size="sm" className="max-w-full" />
                </div>
              )}
              {/* Sibling keys must differ: both remount per kind. */}
              <TestPanel key={`test-${cap.kind}`} cap={cap} />
              {/* Appears when a run starts (not when returning to a screen mid-run). */}
              <AnimatePresence initial={false}>
                {liveRunId !== null && (
                  <m.div key={liveRunId} {...panelIn} className="border-t border-line pt-5">
                    <LiveProgress component={component} />
                  </m.div>
                )}
              </AnimatePresence>
            </div>
          </Panel>
        )}

        <Panel title={t("detectedTitle")} icon={COMPONENT_ICON[component]} className="min-w-0">
          <div aria-hidden="true" className="-mx-2 mb-4 h-36">
            <BenchIllustration component={component} label="" active={liveRunId !== null} />
          </div>
          <DetectedComponent component={component} />
        </Panel>
      </div>

      {cap && <LatestResult key={`result-${cap.kind}`} kind={cap.kind} />}
    </div>
  );
}

/** Remounted per kind (key), so only a result that arrives while the screen is open fades in. */
function LatestResult({ kind }: { kind: BenchmarkKind }) {
  const t = useT("benchmarks");
  const result = useBench((s) => s.lastResultByKind[kind] ?? null);
  const runningThis = useBench((s) => s.activeRun?.kind === kind);
  const visible = result && !runningThis ? result : null;
  if (!result && !runningThis) {
    return (
      <Panel title={t("result")} eyebrow={kindLabel(kind, t)}>
        <StateView kind="empty" title={t("noResultYet")} description={t("noResultYetDesc")} />
      </Panel>
    );
  }
  return (
    <AnimatePresence initial={false}>
      {visible && (
        <m.div key={visible.runId} {...panelIn}>
          {/* Secondary: the test panel above stays the screen's one focal (primary) panel. */}
          <Panel title={t("result")} eyebrow={kindLabel(visible.kind, t)} aside={<ResultMeta result={visible} />}>
            <ResultView result={visible} meta={false} comparison={<ComparisonPanel key={visible.runId} currentRunId={visible.runId} />} />
          </Panel>
        </m.div>
      )}
    </AnimatePresence>
  );
}
