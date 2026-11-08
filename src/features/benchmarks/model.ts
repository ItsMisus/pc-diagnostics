import type { BenchmarkCapability } from "@/types/generated/BenchmarkCapability";
import type { BenchmarkGpuTarget } from "@/types/generated/BenchmarkGpuTarget";
import type { BenchmarkKind } from "@/types/generated/BenchmarkKind";
import type { BenchmarkRequest } from "@/types/generated/BenchmarkRequest";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";
import type { BenchmarkState } from "@/types/generated/BenchmarkState";
import type { PowerSource } from "@/types/generated/PowerSource";
import type { ResultMetric } from "@/types/generated/ResultMetric";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";
import { type Translator, translatorFor } from "@/i18n";
import { formatMib, unavailabilityText } from "@/lib/format";

/** The four benchmarkable components shown as cards. */
export type BenchComponent = "cpu" | "gpu" | "memory" | "storage";

export const BENCH_COMPONENTS: readonly { id: BenchComponent; label: string; kinds: readonly BenchmarkKind[] }[] = [
  { id: "cpu", label: "CPU", kinds: ["cpuSingleCore", "cpuMultiCore", "cpuSustained"] },
  { id: "gpu", label: "GPU", kinds: ["gpuCompute"] },
  { id: "memory", label: "RAM", kinds: ["memoryBandwidth", "memoryLatency"] },
  { id: "storage", label: "SSD", kinds: ["storageSequential", "storageRandom"] },
];

export const ALL_KINDS: readonly BenchmarkKind[] = BENCH_COMPONENTS.flatMap((c) => c.kinds);

export function componentOfKind(kind: BenchmarkKind): BenchComponent {
  switch (kind) {
    case "cpuSingleCore":
    case "cpuMultiCore":
    case "cpuSustained":
      return "cpu";
    case "memoryBandwidth":
    case "memoryLatency":
      return "memory";
    case "storageSequential":
    case "storageRandom":
      return "storage";
    case "gpuCompute":
      return "gpu";
  }
}

export function componentLabel(component: BenchComponent): string {
  return BENCH_COMPONENTS.find((c) => c.id === component)?.label ?? component;
}

/**
 * Text helpers take the translator of the calling component (`useT("benchmarks")`), so the
 * component re-renders on a language change; without one they use the current language.
 */
type T = Translator<"benchmarks">;
const current = (): T => translatorFor("benchmarks");

export function kindLabel(kind: BenchmarkKind, t: T = current()): string {
  return t(`kind_${kind}`);
}

export function isStorageKind(kind: BenchmarkKind): boolean {
  return kind === "storageSequential" || kind === "storageRandom";
}

export function isTerminalState(state: BenchmarkState): boolean {
  return state === "completed" || state === "cancelled" || state === "failed";
}

export function stateLabel(state: BenchmarkState, t: T = current()): string {
  return t(`state_${state}`);
}

export function stateTone(state: BenchmarkState): "ok" | "warn" | "bad" | "neutral" {
  if (state === "failed") return "bad";
  if (state === "cancelled") return "warn";
  return "neutral";
}

export function powerSourceLabel(source: PowerSource, t: T = current()): string {
  return t(`power_${source}`);
}

export function isDebugBuild(result: Pick<BenchmarkResult, "buildProfile">): boolean {
  return result.buildProfile.toLowerCase() !== "release";
}

/** The first metric is the headline value of a run (the backend orders them). */
export function headlineMetric(result: BenchmarkResult): ResultMetric | null {
  return result.state === "completed" ? (result.metrics[0] ?? null) : null;
}

// ---------------------------------------------------------------------------------------
// Request building and validation
// ---------------------------------------------------------------------------------------

/** Lower bounds the capability does not state; the backend validates again anyway. */
export const MIN_DURATION_S = 1;
export const MIN_FILE_MIB = 1;
const MIB = 1024 * 1024;

/** What the user picked on screen. `null` = "use the default / all". */
export interface RunDraft {
  durationS: number | null;
  threads: number | null;
  storageVolume: string | null;
  storageFileMib: number | null;
  consent: boolean;
  /** GPU tests: `BenchmarkGpuTarget.id`; `null` = the default target. */
  gpuId: string | null;
}

export type BuildResult = { ok: true; request: BenchmarkRequest } | { ok: false; reason: string };

function clampInt(value: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, Math.round(value)));
}

export function isAvailable(cap: BenchmarkCapability): boolean {
  return cap.availability.status === "available" && cap.availability.value;
}

/**
 * Duration bounds, or null for tests that are not duration-based. With a GPU target the
 * target's own cap applies (shorter when the driver gives no temperature threshold).
 */
export function durationBounds(
  cap: BenchmarkCapability,
  gpuTarget?: Pick<BenchmarkGpuTarget, "maxDurationS"> | null,
): { min: number; max: number; def: number } | null {
  if (cap.defaultDurationS === null) return null;
  const capMax = cap.maxDurationS ?? cap.defaultDurationS;
  const targetMax = gpuTarget ? Math.min(capMax, gpuTarget.maxDurationS) : capMax;
  const max = Math.max(targetMax, MIN_DURATION_S);
  return { min: MIN_DURATION_S, max, def: clampInt(cap.defaultDurationS, MIN_DURATION_S, max) };
}

export function fileBounds(cap: BenchmarkCapability): { min: number; max: number; def: number } | null {
  if (!isStorageKind(cap.kind) || cap.defaultFileMib === null) return null;
  const max = Math.max(cap.maxFileMib ?? cap.defaultFileMib, MIN_FILE_MIB);
  return { min: MIN_FILE_MIB, max, def: clampInt(cap.defaultFileMib, MIN_FILE_MIB, max) };
}

/** Effective thread limit for multi-threaded tests (`maxThreads` or the logical CPU count). */
export function threadLimit(cap: BenchmarkCapability, logicalCores: number): number | null {
  if (!cap.supportsThreads) return null;
  return Math.max(1, cap.maxThreads ?? logicalCores);
}

/** Presets offered in the selectors, always within bounds and always including the default. */
export function presets(bounds: { min: number; max: number; def: number }, candidates: readonly number[]): number[] {
  const set = new Set(candidates.filter((v) => v >= bounds.min && v <= bounds.max));
  set.add(bounds.def);
  set.add(bounds.max);
  return [...set].sort((a, b) => a - b);
}

export const DURATION_PRESETS = [5, 10, 15, 30, 60, 120, 300, 600];
export const FILE_PRESETS = [64, 128, 256, 512, 1024, 2048, 4096, 8192];

/** Preferred default volume: the system drive, else the first fixed one, else any. */
export function defaultVolume(volumes: readonly VolumeInfo[]): string | null {
  const system = volumes.find((v) => v.mountPoint.toUpperCase().startsWith("C:"));
  return (system ?? volumes.find((v) => !v.removable) ?? volumes[0])?.mountPoint ?? null;
}

export function isTargetAvailable(target: BenchmarkGpuTarget): boolean {
  return target.availability.status === "available" && target.availability.value;
}

/**
 * Target preselected in the GPU selector: the backend's default when it is usable,
 * otherwise the first usable one. `null` when there are no targets or none is usable.
 */
export function defaultGpuTarget(targets: readonly BenchmarkGpuTarget[]): BenchmarkGpuTarget | null {
  const usable = targets.filter(isTargetAvailable);
  return usable.find((t) => t.isDefault) ?? usable[0] ?? null;
}

/**
 * The target a draft refers to: the picked id if it is still listed, else the default.
 * Capabilities can be reloaded (adapter ids change across reboots), so a stale id falls back.
 */
export function resolveGpuTarget(
  targets: readonly BenchmarkGpuTarget[],
  gpuId: string | null,
): BenchmarkGpuTarget | null {
  const picked = gpuId === null ? undefined : targets.find((t) => t.id === gpuId);
  return picked ?? defaultGpuTarget(targets);
}

/** Copy under the GPU selector about the thermal stop of the selected target. */
export function gpuStopCopy(target: Pick<BenchmarkGpuTarget, "stopTemperatureC">, t: T = current()): string {
  return target.stopTemperatureC === null
    ? t("gpuNoThreshold")
    : t("gpuStopAt", { temp: Math.round(target.stopTemperatureC) });
}

export function defaultDraft(cap: BenchmarkCapability, volumes: readonly VolumeInfo[]): RunDraft {
  return {
    durationS: durationBounds(cap)?.def ?? null,
    threads: null,
    storageVolume: isStorageKind(cap.kind) ? defaultVolume(volumes) : null,
    storageFileMib: fileBounds(cap)?.def ?? null,
    consent: false,
    gpuId: defaultGpuTarget(cap.gpuTargets)?.id ?? null,
  };
}

/** Resolved file size the consent text refers to. */
export function resolvedFileMib(cap: BenchmarkCapability, draft: RunDraft): number | null {
  const bounds = fileBounds(cap);
  if (!bounds) return null;
  return draft.storageFileMib === null ? bounds.def : clampInt(draft.storageFileMib, bounds.min, bounds.max);
}

/**
 * Turns the on-screen draft into a request, clamping every value into the capability's
 * bounds. Storage writes are only requested with explicit consent.
 */
export function buildRequest(
  cap: BenchmarkCapability,
  draft: RunDraft,
  ctx: { logicalCores: number; volumes: readonly VolumeInfo[] },
  t: T = current(),
): BuildResult {
  if (!isAvailable(cap)) return { ok: false, reason: t("reasonUnavailable") };

  const selectedGpu = cap.gpuTargets.length > 0 ? resolveGpuTarget(cap.gpuTargets, draft.gpuId) : null;
  const dBounds = durationBounds(cap, selectedGpu);
  const durationS = dBounds
    ? draft.durationS === null
      ? dBounds.def
      : clampInt(draft.durationS, dBounds.min, dBounds.max)
    : null;

  const limit = threadLimit(cap, ctx.logicalCores);
  const threads = limit === null || draft.threads === null ? null : clampInt(draft.threads, 1, limit);

  // No targets listed (non-GPU kinds): `null` lets the backend use its default adapter.
  let gpuId: string | null = null;
  if (cap.gpuTargets.length > 0) {
    const target = resolveGpuTarget(cap.gpuTargets, draft.gpuId);
    if (!target) return { ok: false, reason: t("reasonNoGpu") };
    if (target.availability.status === "unavailable") {
      return {
        ok: false,
        reason: t("reasonGpuUnusableWhy", { reason: unavailabilityText(target.availability.reason).toLowerCase() }),
      };
    }
    if (!isTargetAvailable(target)) return { ok: false, reason: t("reasonGpuUnusable") };
    gpuId = target.id;
  }

  if (!isStorageKind(cap.kind)) {
    return {
      ok: true,
      request: {
        kind: cap.kind,
        durationS,
        threads,
        storageVolume: null,
        storageFileMib: null,
        storageWriteConsent: false,
        gpuId,
      },
    };
  }

  if (!draft.storageVolume) return { ok: false, reason: t("reasonPickVolume") };
  const volume = ctx.volumes.find((v) => v.mountPoint === draft.storageVolume);
  if (!volume) return { ok: false, reason: t("reasonVolumeGone") };
  const fileMib = resolvedFileMib(cap, draft);
  if (fileMib !== null && fileMib * MIB > volume.availableBytes) {
    return { ok: false, reason: t("reasonNoSpace") };
  }
  if (!draft.consent) return { ok: false, reason: t("reasonConsent") };

  return {
    ok: true,
    request: {
      kind: cap.kind,
      durationS,
      threads,
      storageVolume: volume.mountPoint,
      storageFileMib: fileMib,
      storageWriteConsent: true,
      gpuId,
    },
  };
}

export function storageConsentCopy(fileMib: number | null, t: T = current()): string {
  const size = fileMib === null ? t("consentDefaultSize") : formatMib(fileMib);
  return t("consentCopy", { size });
}
