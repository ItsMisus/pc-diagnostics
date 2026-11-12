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
import { formatUnitValue } from "@/lib/format";
import { duration, ease, pageVariants } from "@/lib/motion";
import { useInventory } from "@/features/monitoring/inventoryStore";
import type { BenchmarkCapability } from "@/types/generated/BenchmarkCapability";
import type { BenchmarkKind } from "@/types/generated/BenchmarkKind";
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
import { TestPanel } from "./TestPanel";
import { COMPONENT_ICON } from "./ui";
import { useBenchView } from "./viewStore";

const NO_CAPS: readonly BenchmarkCapability[] = [];

/** Entrance of a block that appears in response to a run starting or finishing. */
const panelIn = {
  initial: { opacity: 0, y: 6 },
  animate: { opacity: 1, y: 0, transition: { duration: duration.component, ease: ease.out } },
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
      className="group surface-secondary pressable flex min-h-44 w-full min-w-0 flex-col items-stretch gap-3 p-4 text-left transition-colors duration-150 hover:border-line-strong hover:bg-surface-2"
    >
      <span className="flex items-start justify-between gap-3">
        <span aria-hidden="true" className="satin grid size-9 shrink-0 place-items-center rounded-full text-fg-muted">
          <Icon name={COMPONENT_ICON[component]} size={18} />
        </span>
        <span className="flex min-w-0 flex-wrap items-center justify-end gap-2">
          {running ? (
            <StatusBadge tone="neutral" label={t("runningKind", { kind: kindLabel(running, t) })} />
          ) : (
            availability.status !== "available" && (
              <StatusBadge
                tone="neutral"
                label={availability.status === "unavailable" && availability.reason?.kind === "notSupported" ? t("unsupportedShort") : t("notAvailable")}
              />
            )
          )}
        </span>
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

/** On a component screen: the same four components as one segmented control. */
function ComponentPicker({ value }: { value: BenchComponent }) {
  const t = useT("benchmarks");
  const open = useBenchView((s) => s.open);
  const options = useMemo(
    () => BENCH_COMPONENTS.map((c) => ({ value: c.id, label: c.label, icon: COMPONENT_ICON[c.id] })),
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
  return (
    <AnimatePresence initial={false}>
      {visible && (
        <m.div key={visible.runId} {...panelIn}>
          <Panel variant="primary" title={t("result")} eyebrow={kindLabel(visible.kind, t)} aside={<ResultMeta result={visible} />}>
            <ResultView
              result={visible}
              meta={false}
              comparison={<ComparisonPanel key={visible.runId} currentRunId={visible.runId} />}
            />
          </Panel>
        </m.div>
      )}
    </AnimatePresence>
  );
}
