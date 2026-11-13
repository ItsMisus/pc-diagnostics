import type { Domain, Point } from "@/components/charts/scale";
import type { Tone } from "@/components/common/StatusBadge";
import type { DeviceShapeResolution } from "@/features/device/formFactor";
import { primaryGpu } from "@/features/monitoring/selection";
import type { AppError } from "@/types/generated/AppError";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";
import type { FormFactorBasis } from "@/types/generated/FormFactorBasis";
import type { HardwareInventory } from "@/types/generated/HardwareInventory";
import type { LiveSample } from "@/types/generated/LiveSample";
import type { Severity } from "@/types/generated/Severity";
import type { SystemInfo } from "@/types/generated/SystemInfo";

/**
 * Pure helpers of the Overview: the overall state taken from the latest diagnosis, the
 * device identity from the inventory, the device-model caption and the short sparkline
 * window. No translation here: callers turn the results into text.
 */

// ---------------------------------------------------------------- Overall state

/** A diagnosis newer than this counts as recent: "Esegui diagnosi" is then not the primary action. */
export const RECENT_REPORT_MS = 24 * 60 * 60 * 1000;

export type OverallState =
  /** The latest stored report is being read. */
  | { kind: "loading" }
  /** Reading the latest report failed. */
  | { kind: "error"; error: AppError }
  /** No report exists yet (a first run may be in progress: see `running`). */
  | { kind: "neverRun" }
  /** The latest report has no findings at all. */
  | { kind: "clear"; at: number }
  /** The latest report has findings: counts per severity and the most severe one. */
  | { kind: "findings"; at: number; counts: Record<Severity, number>; highest: Severity; tone: Tone };

export interface OverallInput {
  status: "idle" | "loading" | "ready" | "error";
  report: DiagnosticReport | null;
  error: AppError | null;
}

/**
 * The Overview's overall state: only what the latest diagnosis says. No score and no health
 * percentage: with no report, the state is "not run yet".
 */
export function overallState({ status, report, error }: OverallInput): OverallState {
  // A report already on screen stays while it is reloaded (or a reload fails).
  if (report) {
    const counts: Record<Severity, number> = { critical: 0, warning: 0, info: 0 };
    for (const f of report.findings) counts[f.severity] += 1;
    if (report.findings.length === 0) return { kind: "clear", at: report.generatedAt };
    const highest: Severity = counts.critical > 0 ? "critical" : counts.warning > 0 ? "warning" : "info";
    const tone: Tone = highest === "critical" ? "bad" : highest === "warning" ? "warn" : "neutral";
    return { kind: "findings", at: report.generatedAt, counts, highest, tone };
  }
  if (status === "error" && error) return { kind: "error", error };
  if (status === "ready") return { kind: "neverRun" };
  return { kind: "loading" };
}

/** True when a diagnosis ran within `RECENT_REPORT_MS` of `now`. */
export function isRecent(state: OverallState, now: number): boolean {
  return (state.kind === "clear" || state.kind === "findings") && now - state.at < RECENT_REPORT_MS;
}

// ---------------------------------------------------------------- Device identity

export interface DeviceIdentity {
  /** Product name (macOS, e.g. "MacBook Air (13-inch, M5)"), else the OS name; null when neither is reported. */
  name: string | null;
  /** OS name and version, when the name above is the product (otherwise it repeats it). */
  os: string | null;
  cpu: string | null;
  /** Logical processors as reported by the inventory. */
  logicalCores: number | null;
  physicalCores: number | null;
  /** Primary physical GPU name; omitted when it only repeats the CPU name (Apple silicon). */
  gpu: string | null;
  memoryBytes: number | null;
}

const clean = (s: string | null | undefined): string | null => {
  const v = s?.replace(/\s+/g, " ").trim();
  return v ? v : null;
};

/** What the hero says about the machine. Unknown parts are null (omitted), never guessed. */
export function deviceIdentity(inv: HardwareInventory | null, sys: SystemInfo | null): DeviceIdentity {
  const product = clean(inv?.formFactor.productName);
  const osName = clean(sys?.osName);
  // The long version is the marketing name ("macOS 26.1", "Windows 11 Pro 24H2"); osName alone
  // is the kernel family on macOS ("Darwin").
  const osFull = clean(sys?.osLongVersion) ?? (osName ? clean(`${osName} ${sys?.osVersion ?? ""}`) : null);
  const cpu = clean(inv?.cpu.brand);
  const gpuInfo = inv ? primaryGpu(inv.gpus) : null;
  const gpu = clean(gpuInfo?.name);
  return {
    name: product ?? osFull,
    os: product ? osFull : null,
    cpu,
    logicalCores: inv && inv.cpu.logicalCores > 0 ? inv.cpu.logicalCores : null,
    physicalCores: inv?.cpu.physicalCores ?? null,
    gpu: gpu && gpu !== cpu ? gpu : null,
    memoryBytes: inv && inv.memory.totalBytes > 0 ? inv.memory.totalBytes : null,
  };
}

// ---------------------------------------------------------------- Device model caption

/** i18n key (overview namespace) of the quiet caption under the device model. */
export type ShapeCaptionKey =
  | "shapeDetected_productName"
  | "shapeDetected_modelIdentifier"
  | "shapeDetected_chassisType"
  | "shapeDetected_unknownBasis"
  | "shapeOverride"
  | "shapePending"
  | "shapeUnknown";

export function shapeCaptionKey(resolution: DeviceShapeResolution): ShapeCaptionKey {
  switch (resolution.origin) {
    case "override":
      return "shapeOverride";
    case "pending":
      return "shapePending";
    case "unknown":
      return "shapeUnknown";
    case "detected":
      return basisKey(resolution.basis);
  }
}

function basisKey(basis: FormFactorBasis | null): ShapeCaptionKey {
  switch (basis) {
    case "productName":
      return "shapeDetected_productName";
    case "modelIdentifier":
      return "shapeDetected_modelIdentifier";
    case "chassisType":
      return "shapeDetected_chassisType";
    case null:
      return "shapeDetected_unknownBasis";
  }
}

// ---------------------------------------------------------------- Sparklines

/** Span of the Overview sparklines: the last minute. */
export const SPARK_WINDOW_MS = 60_000;

/**
 * The last `windowMs` of a series and the matching fixed x domain, so the line never
 * stretches while the buffer fills and never draws outside its box. The window ends at the
 * newest point; an empty series gets a [0, windowMs] domain.
 */
export function recentWindow(points: readonly Point[], windowMs = SPARK_WINDOW_MS): { points: Point[]; domain: Domain } {
  const last = points[points.length - 1];
  if (!last) return { points: [], domain: [0, windowMs] };
  const from = last.t - windowMs;
  let i = points.length - 1;
  while (i > 0 && (points[i - 1] as Point).t >= from) i -= 1;
  return { points: points.slice(i), domain: [from, last.t] };
}

// ---------------------------------------------------------------- Chips around the device

/** A live reading shown as a small chip around the 3D device. */
export type ChipId = "cpu" | "ram" | "gpu" | "cpuTemp";

const available = (m: { status: string } | undefined): boolean => m?.status === "available";

/**
 * Which chips to show around the device: only readings the latest sample actually carries
 * (an unavailable value is not shown here; the panels below give its reason), at most 4.
 */
export function heroChips(sample: LiveSample | null | undefined, gpuId: string | null): ChipId[] {
  if (!sample) return [];
  const out: ChipId[] = [];
  if (available(sample.cpu.usagePercent)) out.push("cpu");
  if (available(sample.memory.usedBytes) && available(sample.memory.totalBytes)) out.push("ram");
  if (gpuId !== null && available(sample.gpus.find((g) => g.id === gpuId)?.usagePercent)) out.push("gpu");
  if (available(sample.cpu.temperatureC)) out.push("cpuTemp");
  return out.slice(0, 4);
}
