import { AnimatePresence, m } from "motion/react";
import { memo, useEffect, useId, useMemo, useState } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Icon, type IconName } from "@/components/common/Icon";
import { useUnavailabilityText } from "@/components/common/MetricValue";
import { Skeleton, SkeletonLines } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import type { Tone } from "@/components/common/StatusBadge";
import { useBench } from "@/features/benchmarks/benchStore";
import { airflowFor, useCoolingKind } from "@/features/cooling/useCoolingKind";
import { DeviceShapePicker, SHAPE_ICON } from "@/features/device/DeviceShapePicker";
import { DEVICE_SHAPES, type DeviceShapeResolution } from "@/features/device/formFactor";
import { useDeviceShape } from "@/features/device/useDeviceShape";
import { DeviceStage, type DeviceIndicator } from "@/features/device3d";
import { StageFallback } from "@/features/device3d/StageFallback";
import { useDiag } from "@/features/diagnostics/diagStore";
import { simpleSeverityLabel } from "@/features/diagnostics/simple";
import { useInventoryData } from "@/features/monitoring/hooks";
import { useLive } from "@/features/monitoring/liveStore";
import { primaryGpu } from "@/features/monitoring/selection";
import { valueOf } from "@/features/monitoring/series";
import { InventoryState } from "@/features/monitoring/StatusViews";
import { useAsync } from "@/hooks/useAsync";
import { useIsSimple, useViewMode } from "@/hooks/useViewMode";
import { type Translator, useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatBytes, formatCelsius, formatDateTime, formatPercent } from "@/lib/format";
import { duration, ease } from "@/lib/motion";
import { commands } from "@/services/commands";
import type { DeviceShape } from "@/types/generated/DeviceShape";
import type { FormFactorInfo } from "@/types/generated/FormFactorInfo";
import type { Unavailability } from "@/types/generated/Unavailability";
import { type ChipId, deviceIdentity, heroChips, isRecent, type OverallState, overallState, shapeCaptionKey } from "./model";

/**
 * The Overview's focal element: the 3D model of the user's kind of device, who the device is
 * (from the real inventory) and the overall state taken from the latest diagnosis, plus the
 * quick actions. Stacks under the stage when the content column is narrow.
 */
export function Hero() {
  const t = useT("overview");
  const headingId = useId();
  return (
    <section
      aria-labelledby={headingId}
      className="surface-primary relative grid grid-cols-1 rounded-2xl! @3xl:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)]"
    >
      <h2 id={headingId} className="sr-only">
        {t("heroLabel")}
      </h2>
      <HeroStage />
      <div className="flex min-w-0 flex-col gap-5 p-5 pt-2 @3xl:p-6 @3xl:ps-2">
        <Identity />
        <div className="flex flex-col gap-4 border-t border-line pt-5">
          <OverallStatus />
          <QuickActions />
        </div>
      </div>
    </section>
  );
}

// ---------------------------------------------------------------- Stage

/** Stage area: aspect ratio while stacked, the row's height (with a floor) side by side. */
const STAGE_AREA = "relative min-w-0 aspect-[16/10] max-h-[420px] w-full @3xl:aspect-auto @3xl:max-h-none @3xl:min-h-[360px]";

function HeroStage() {
  const { resolution, setShape, canSet } = useDeviceShape();
  const benchmarkRunning = useBench((s) => s.activeRun !== null);
  // The stage pauses itself when the window is hidden or it is off screen; these are ours.
  const pageVisible = useAppStore((s) => s.section === "overview");
  const t = useT("overview");

  if (resolution.origin === "pending") {
    return (
      <div className={`${STAGE_AREA} p-6`} role="status" aria-busy="true">
        <span className="sr-only">{t("shapePending")}</span>
        <Skeleton className="h-full w-full rounded-xl" />
      </div>
    );
  }
  if (resolution.origin === "unknown") {
    return <ChooseShape disabled={!canSet} onChoose={(s) => void setShape(s)} />;
  }
  return (
    <div className={STAGE_AREA}>
      <StageWithChips kind={resolution.shape} paused={benchmarkRunning || !pageVisible} />
    </div>
  );
}

/**
 * The model could not be decided: the three models as illustrations to pick from (never a
 * guess shown as data). The picker in Settings and in the hero changes it later.
 */
function ChooseShape({ disabled, onChoose }: { disabled: boolean; onChoose: (shape: DeviceShape) => void }) {
  const t = useT("overview");
  const headingId = useId();
  return (
    <div className={`${STAGE_AREA} flex flex-col justify-center gap-4 p-5 @3xl:p-6`} role="group" aria-labelledby={headingId}>
      <div className="flex flex-col gap-1">
        <h3 id={headingId} className="font-display text-lg font-semibold text-fg">
          {t("shapeChooseTitle")}
        </h3>
        <p className="max-w-prose text-sm text-fg-muted">{t("shapeChooseHint")}</p>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-3 gap-3">
        {DEVICE_SHAPES.map((s) => (
          <button
            key={s}
            type="button"
            disabled={disabled}
            onClick={() => onChoose(s)}
            aria-label={t("shapeChooseOption", { shape: t(`shape_${s}`) })}
            className="pressable group flex min-h-0 min-w-0 flex-col items-center gap-2 rounded-lg border border-line bg-surface-2 p-3 transition-colors duration-150 hover:border-line-strong hover:bg-surface-3 disabled:opacity-45"
          >
            <span className="relative block min-h-16 w-full flex-1">
              <StageFallback kind={s} variant="illustration" />
            </span>
            <span className="flex max-w-full items-center gap-1.5 text-sm text-fg">
              <Icon name={SHAPE_ICON[s]} size={14} className="shrink-0 text-fg-subtle" />
              <span className="truncate">{t(`shape_${s}`)}</span>
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

const CHIP_SLOT: Record<ChipId, DeviceIndicator["slot"]> = {
  cpu: "top-start",
  ram: "top-end",
  gpu: "bottom-start",
  cpuTemp: "bottom-end",
};

/**
 * The stage plus up to four live chips. The chip list changes only when a reading appears or
 * disappears (a string key), so the stage does not re-render per sample; each chip
 * subscribes to its own value.
 */
function StageWithChips({ kind, paused }: { kind: DeviceShape; paused: boolean }) {
  const { data } = useInventoryData();
  const airflow = airflowFor(useCoolingKind());
  const gpuId = useMemo(() => (data ? (primaryGpu(data.gpus)?.id ?? null) : null), [data]);
  // A failed or stalled connection keeps its last sample: no chips then, so no stale value looks live.
  const key = useLive((s) =>
    s.stalled || s.status === "unsupported" || s.status === "permission" || s.status === "error" ? "" : heroChips(s.latest, gpuId).join(","),
  );
  const indicators = useMemo<DeviceIndicator[]>(
    () =>
      key
        ? (key.split(",") as ChipId[]).map((id) => ({ id, slot: CHIP_SLOT[id], content: <LiveChip id={id} gpuId={gpuId} /> }))
        : [],
    [key, gpuId],
  );
  return <DeviceStage kind={kind} paused={paused} spin airflow={airflow} indicators={indicators} aspectRatio={null} className="absolute! inset-3 rounded-xl" />;
}

const CHIP_ICON: Record<ChipId, IconName> = { cpu: "cpu", ram: "memory", gpu: "gpu", cpuTemp: "temperature" };

/** One live value next to the model: a satin capsule (no blur: chips repeat), never animated. */
const LiveChip = memo(function LiveChip({ id, gpuId }: { id: ChipId; gpuId: string | null }) {
  const t = useT("overview");
  const value = useLive((s) => {
    const x = s.latest;
    if (!x) return null;
    switch (id) {
      case "cpu":
        return valueOf(x.cpu.usagePercent);
      case "ram": {
        const used = valueOf(x.memory.usedBytes);
        const total = valueOf(x.memory.totalBytes);
        return used !== null && total !== null && total > 0 ? (used / total) * 100 : null;
      }
      case "gpu":
        return valueOf(x.gpus.find((g) => g.id === gpuId)?.usagePercent);
      case "cpuTemp":
        return valueOf(x.cpu.temperatureC);
    }
  });
  if (value === null) return null;
  const label = t(id === "cpu" ? "chipCpu" : id === "ram" ? "chipRam" : id === "gpu" ? "chipGpu" : "chipTemp");
  const text = id === "cpuTemp" ? formatCelsius(value) : formatPercent(value);
  return (
    <span className="satin inline-flex h-7 max-w-[11rem] items-center gap-1.5 rounded-full px-2.5 text-xs">
      <Icon name={CHIP_ICON[id]} size={14} className="shrink-0 text-fg-subtle" />
      <span className="truncate text-fg-muted">{label}</span>
      <span className="text-fg tabular">{text}</span>
    </span>
  );
});

// ---------------------------------------------------------------- Identity

function Identity() {
  // Byte labels follow the view mode (lib/format.ts).
  useViewMode();
  const t = useT("overview");
  const { status, data } = useInventoryData();
  const [sys] = useAsync(commands.getSystemInfo);
  const id = useMemo(() => deviceIdentity(data, sys.status === "ready" ? sys.data : null), [data, sys]);

  if (status !== "ready" || !data) {
    return (
      <InventoryState
        compact
        placeholder={
          <span aria-hidden="true" className="flex flex-col gap-3">
            <Skeleton className="h-7 w-2/3" />
            <SkeletonLines lines={3} />
          </span>
        }
      />
    );
  }

  const cores =
    id.physicalCores !== null && id.logicalCores !== null && id.physicalCores !== id.logicalCores
      ? t("coresThreads", { physical: id.physicalCores, logical: id.logicalCores })
      : id.logicalCores !== null
        ? t("cores", { count: id.logicalCores })
        : null;
  const rows: { label: string; value: string }[] = [];
  if (id.cpu) rows.push({ label: t("identityCpu"), value: cores ? `${id.cpu} · ${cores}` : id.cpu });
  if (id.gpu) rows.push({ label: t("identityGpu"), value: id.gpu });
  if (id.memoryBytes !== null) rows.push({ label: t("identityMemory"), value: formatBytes(id.memoryBytes, 0) });
  if (id.os) rows.push({ label: t("identitySystem"), value: id.os });

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <p className="font-display text-3xl font-semibold break-words text-fg">{id.name ?? t("thisComputer")}</p>
      {rows.length > 0 && (
        <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-sm">
          {rows.map((r) => (
            <div key={r.label} className="contents">
              <dt className="text-fg-subtle">{r.label}</dt>
              <dd className="min-w-0 break-words text-fg tabular">{r.value}</dd>
            </div>
          ))}
        </dl>
      )}
      <ShapeRow formFactor={data.formFactor} />
      <AirflowLegend />
    </div>
  );
}

/** Same hues as the particles on the stage (`device3d/scene/Airflow.tsx`). */
const AIR_DOT = { intake: "bg-[#2f9bef]", exhaust: "bg-[#f5782e]" } as const;

function AirDot({ kind }: { kind: keyof typeof AIR_DOT }) {
  return <span aria-hidden="true" className={`inline-block h-1.5 w-3 shrink-0 rounded-full ${AIR_DOT[kind]}`} />;
}

/** What the streams around the model mean: fans' intake and exhaust, or a fanless case's heat. */
function AirflowLegend() {
  const t = useT("overview");
  const kind = useCoolingKind();
  if (kind === null) return null;
  return (
    <div className="flex min-w-0 flex-col gap-1.5 text-xs">
      <p className="flex items-center gap-1.5 text-fg-muted">
        <Icon name="fan" size={14} className="shrink-0 text-fg-subtle" />
        {t(kind === "fans" ? "airflowTitle_fans" : kind === "passive" ? "airflowTitle_passive" : "airflowTitle_unknown")}
      </p>
      {kind === "fans" && (
        <p className="flex flex-wrap items-center gap-x-4 gap-y-1 text-fg-muted">
          <span className="inline-flex items-center gap-1.5">
            <AirDot kind="intake" />
            {t("airflowIntake")}
          </span>
          <span className="inline-flex items-center gap-1.5">
            <AirDot kind="exhaust" />
            {t("airflowExhaust")}
          </span>
        </p>
      )}
      {kind === "passive" && (
        <p className="flex items-center gap-1.5 text-fg-muted">
          <AirDot kind="exhaust" />
          {t("airflowHeat")}
        </p>
      )}
      <p className="text-fg-subtle">{t(kind === "fans" ? "airflowNote_fans" : kind === "passive" ? "airflowNote_passive" : "airflowNote_unknown")}</p>
    </div>
  );
}

/** Which model is shown and why, with the control to change it. */
function ShapeRow({ formFactor }: { formFactor: FormFactorInfo }) {
  const t = useT("overview");
  const simple = useIsSimple();
  const reasonText = useUnavailabilityText();
  const { resolution, override, canSet, saving, setShape } = useDeviceShape();
  const [open, setOpen] = useState(false);
  const pickerId = useId();
  const caption = t(shapeCaptionKey(resolution));

  if (resolution.origin === "pending") {
    return <p className="text-xs text-fg-subtle">{caption}</p>;
  }
  if (resolution.origin === "unknown") {
    return (
      <div className="flex flex-col gap-1 text-xs text-fg-subtle">
        <p className="flex items-center gap-1.5 text-fg-muted">
          <Icon name="info" size={14} className="shrink-0" />
          {caption}
        </p>
        <p>{unknownWhy(resolution, t, reasonText)}</p>
      </div>
    );
  }

  const evidence = simple ? null : shapeEvidence(formFactor, t);
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-xs">
        <Icon name={SHAPE_ICON[resolution.shape]} size={14} className="shrink-0 text-fg-subtle" />
        <span className="text-fg-muted">{t(`shape_${resolution.shape}`)}</span>
        <span aria-hidden="true" className="text-fg-subtle">
          ·
        </span>
        <span className="text-fg-subtle">{caption}</span>
        <span className="ms-auto flex items-center gap-1">
          {override !== null && (
            <Button variant="ghost" size="sm" disabled={!canSet} loading={saving} onClick={() => void setShape(null)}>
              {t("shapeReset")}
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            disabled={!canSet}
            aria-expanded={open}
            aria-controls={pickerId}
            aria-label={t("shapeChangeAria")}
            onClick={() => setOpen((o) => !o)}
          >
            {t("shapeChange")}
            <Icon name={open ? "chevronUp" : "chevronDown"} size={14} className="-me-0.5" />
          </Button>
        </span>
      </div>
      {evidence && <p className="text-2xs break-words text-fg-subtle">{evidence}</p>}
      <AnimatePresence initial={false}>
        {open && (
          <m.div
            id={pickerId}
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0, transition: { duration: duration.fast, ease: ease.out } }}
            exit={{ opacity: 0, y: -4, transition: { duration: duration.exit, ease: ease.in } }}
          >
            <DeviceShapePicker />
          </m.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function unknownWhy(
  resolution: Extract<DeviceShapeResolution, { origin: "unknown" }>,
  t: Translator<"overview">,
  reasonText: (reason: Unavailability) => string,
): string {
  const why = resolution.why;
  if (why.kind === "unavailable") return reasonText(why.reason);
  return t(why.formFactor === "allInOne" ? "shapeNoModel_allInOne" : "shapeNoModel_other");
}

/** Advanced view: the evidence behind the detection (product name, model id, SMBIOS codes). */
function shapeEvidence(f: FormFactorInfo, t: Translator<"overview">): string | null {
  const parts: string[] = [];
  if (f.productName) parts.push(t("shapeEvidenceProduct", { value: f.productName }));
  if (f.modelIdentifier) parts.push(t("shapeEvidenceModel", { value: f.modelIdentifier }));
  if (f.chassisTypes.length > 0) parts.push(t("shapeEvidenceChassis", { value: f.chassisTypes.join(", ") }));
  return parts.length > 0 ? parts.join(" · ") : null;
}

// ---------------------------------------------------------------- Overall state

const TONE_ICON: Record<Tone, { icon: IconName; className: string }> = {
  ok: { icon: "success", className: "text-ok" },
  warn: { icon: "warning", className: "text-warn" },
  bad: { icon: "error", className: "text-bad" },
  neutral: { icon: "info", className: "text-fg-subtle" },
};

/** Loads the latest report once and derives the state from it. */
function useOverall(): OverallState {
  const ensureLoaded = useDiag((s) => s.ensureLoaded);
  useEffect(() => ensureLoaded(), [ensureLoaded]);
  const status = useDiag((s) => s.status);
  const report = useDiag((s) => s.report);
  const error = useDiag((s) => s.error);
  return useMemo(() => overallState({ status, report, error }), [status, report, error]);
}

function OverallStatus() {
  const t = useT("overview");
  const td = useT("diagnostics");
  const simple = useIsSimple();
  const state = useOverall();
  const running = useDiag((s) => s.running);
  const runError = useDiag((s) => s.runError);
  const reload = useDiag((s) => s.reload);
  const run = useDiag((s) => s.run);
  const hasReport = state.kind === "clear" || state.kind === "findings";

  let body;
  switch (state.kind) {
    case "loading":
      body = (
        <div role="status" aria-busy="true">
          <span className="sr-only">{t("stateLoading")}</span>
          <span aria-hidden="true" className="flex flex-col gap-2">
            <Skeleton className="h-5 w-1/2" />
            <Skeleton className="h-3 w-1/3" />
          </span>
        </div>
      );
      break;
    case "error":
      body = <StateView compact kind={stateKindForError(state.error)} title={t("stateUnavailable")} error={state.error} onRetry={reload} />;
      break;
    case "neverRun":
      body = (
        <div className="flex flex-col gap-1">
          <p className="flex items-center gap-2 font-display text-lg font-semibold text-fg">
            <Icon name="info" size={18} className="shrink-0 text-fg-subtle" />
            {running ? td("running") : td("neverRun")}
          </p>
          <p className="text-sm text-fg-muted">{running ? td("runningDescription") : t("stateNeverRunHint")}</p>
        </div>
      );
      break;
    case "clear":
    case "findings": {
      const tone: Tone = state.kind === "clear" ? "ok" : state.tone;
      const headline = state.kind === "clear" ? t("stateClear") : findingsHeadline(state.counts, simple, td);
      const icon = TONE_ICON[tone];
      body = (
        <div className="flex flex-col gap-1">
          <p className="flex items-start gap-2 font-display text-lg font-semibold text-fg">
            <Icon name={icon.icon} size={18} className={`mt-0.5 shrink-0 ${icon.className}`} />
            <span className="min-w-0">{headline}</span>
          </p>
          <p className="text-xs text-fg-subtle tabular">{t("stateFrom", { date: formatDateTime(state.at) })}</p>
          {!simple && <p className="text-xs text-fg-subtle">{t("stateNotAScore")}</p>}
        </div>
      );
      break;
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <p className="eyebrow">{t("stateEyebrow")}</p>
      {body}
      {runError && (
        <StateView
          compact
          kind={stateKindForError(runError)}
          title={hasReport ? td("runFailedKeptPrevious") : td("runFailed")}
          error={runError}
          onRetry={() => void run()}
        />
      )}
    </div>
  );
}

/** "1 critico, 2 avvisi": only severities that occur, most severe first (counts said once). */
function findingsHeadline(
  counts: Record<"critical" | "warning" | "info", number>,
  simple: boolean,
  td: Translator<"diagnostics">,
): string {
  const parts: string[] = [];
  const add = (n: number, s: "critical" | "warning" | "info", key: "insightsCritical" | "insightsWarning" | "insightsInfo") => {
    if (n === 0) return;
    parts.push(simple ? `${n} ${simpleSeverityLabel(s).toLowerCase()}` : td.plural(key, n));
  };
  add(counts.critical, "critical", "insightsCritical");
  add(counts.warning, "warning", "insightsWarning");
  add(counts.info, "info", "insightsInfo");
  return parts.join(", ");
}

// ---------------------------------------------------------------- Quick actions

function QuickActions() {
  const t = useT("overview");
  const td = useT("diagnostics");
  const state = useOverall();
  const running = useDiag((s) => s.running);
  const run = useDiag((s) => s.run);
  const setSection = useAppStore((s) => s.setSection);
  const benchmarkRunning = useBench((s) => s.activeRun !== null);
  // The one primary action of the page, unless a recent diagnosis already answered.
  const primary = !isRecent(state, Date.now());
  return (
    <div role="group" aria-label={t("quickActions")} className="flex flex-wrap items-center gap-2">
      <Button variant={primary ? "primary" : "secondary"} icon="diagnostics" loading={running} onClick={() => void run()}>
        {running ? td("running") : td("run")}
      </Button>
      <Button icon="benchmarks" onClick={() => setSection("benchmarks")}>
        {benchmarkRunning ? t("benchmarkRunning") : t("runBenchmark")}
      </Button>
    </div>
  );
}
