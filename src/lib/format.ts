import type { ErrorCode } from "@/types/generated/ErrorCode";
import type { Metric } from "@/types/generated/Metric";
import type { Source } from "@/types/generated/Source";
import type { Unavailability } from "@/types/generated/Unavailability";
import type { Unit } from "@/types/generated/Unit";
import { localeTag, translatorFor } from "@/i18n";

/**
 * How byte sizes are labelled. Both scale by 1024 (Windows reports sizes in powers of 1024);
 * only the label differs:
 * - `windows`: KB, MB, GB, TB, the labels File Explorer and Task Manager use, so the number
 *   matches what the user sees in Windows. Used in the "Semplice" view.
 * - `iec`: KiB, MiB, GiB, TiB, the exact technical names. Used in the "Avanzata" view.
 * The backend follows the same rule (`fmt::bytes_plain` / `fmt::bytes` in crates/diagnostics).
 */
export type ByteUnits = "windows" | "iec";

const BYTE_LABELS: Record<ByteUnits, readonly string[]> = {
  windows: ["B", "KB", "MB", "GB", "TB", "PB"],
  iec: ["B", "KiB", "MiB", "GiB", "TiB", "PiB"],
};

let byteUnits: ByteUnits = "windows";

/**
 * Sets the labels `formatBytes` uses. Called by the app store whenever the view mode changes,
 * before React re-renders, so the next render already shows the new labels.
 */
export function setByteUnits(units: ByteUnits): void {
  byteUnits = units;
}

export function getByteUnits(): ByteUnits {
  return byteUnits;
}

/** Powers of 1024, labelled per `units` (default: the current view mode's). */
export function formatBytes(bytes: number, fractionDigits = 1, units: ByteUnits = byteUnits): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  const labels = BYTE_LABELS[units];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < labels.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : fractionDigits;
  return `${value.toFixed(digits)} ${labels[unit]}`;
}

/** A size given in MiB (benchmark file sizes), labelled per the current view mode: "256 MB". */
export function formatMib(mib: number): string {
  return formatBytes(mib * 1024 * 1024, 0);
}

export function formatBytesPerSec(bytesPerSec: number, fractionDigits = 1, units: ByteUnits = byteUnits): string {
  const bytes = formatBytes(bytesPerSec, fractionDigits, units);
  return bytes === "—" ? bytes : `${bytes}/s`;
}

export function formatMhz(mhz: number): string {
  if (!Number.isFinite(mhz) || mhz < 0) return "—";
  return `${Math.round(mhz)} MHz`;
}

export function formatCelsius(celsius: number): string {
  if (!Number.isFinite(celsius)) return "—";
  return `${celsius.toFixed(0)} °C`;
}

export function formatWatts(watts: number): string {
  if (!Number.isFinite(watts) || watts < 0) return "—";
  return `${watts.toFixed(1)} W`;
}

/** Memory module transfer rate (`Win32_PhysicalMemory` reports MT/s). */
export function formatMts(mts: number): string {
  if (!Number.isFinite(mts) || mts <= 0) return "—";
  return `${Math.round(mts)} MT/s`;
}

/** Events per second, e.g. hard page faults. */
export function formatPerSec(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "—";
  return `${value < 10 ? value.toFixed(1) : Math.round(value)}/s`;
}

/** Wall-clock time of an epoch-ms timestamp, in the current language's locale. */
export function formatTime(epochMs: number): string {
  if (!Number.isFinite(epochMs)) return "—";
  return new Date(epochMs).toLocaleTimeString(localeTag());
}

/** Date and time of an epoch-ms timestamp, current locale (Italian: "03/11/2026, 14:05:09"). */
export function formatDateTime(epochMs: number): string {
  if (!Number.isFinite(epochMs)) return "—";
  return new Date(epochMs).toLocaleString(localeTag());
}

/**
 * Elapsed time with second precision, for running operations: "8 s", "1 min 05 s".
 * Unlike `formatDuration` it never drops the seconds.
 */
export function formatElapsed(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "—";
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s} s`;
  const minutes = Math.floor(s / 60);
  return `${minutes} min ${String(s % 60).padStart(2, "0")} s`;
}

/**
 * Formats an ISO 8601 calendar date ("2024-05-01", optionally with a time part) in the
 * current language's order ("01/05/2024" in Italian, "05/01/2024" in English) without
 * going through `Date`, so no time zone can shift the day.
 * Unparseable input is returned unchanged rather than hidden.
 */
export function formatIsoDate(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  if (!m) return iso;
  return translatorFor("errors")("isoDate", { year: m[1] ?? "", month: m[2] ?? "", day: m[3] ?? "" });
}

/**
 * Smallest "nice" number (1, 2, 2.5 or 5 × 10^k) that is >= `x`. Keeps chart ceilings and
 * ticks readable. Non-positive or invalid input returns 0.
 */
export function niceCeil(x: number): number {
  if (!Number.isFinite(x) || x <= 0) return 0;
  const base = 10 ** Math.floor(Math.log10(x));
  for (const m of [1, 2, 2.5, 5, 10]) {
    const candidate = Number((m * base).toPrecision(12));
    if (candidate >= x) return candidate;
  }
  return Number((10 * base).toPrecision(12));
}

/** Product and API names, identical in every language. `d3dkmt` is a description: translated. */
const SOURCE_LABELS: Record<Exclude<Source, "d3dkmt">, string> = {
  sysinfo: "sysinfo",
  pdh: "PDH",
  dxgi: "DXGI",
  nvml: "NVML",
  powerApi: "Power API",
  wmi: "WMI",
  configManager: "Configuration Manager",
  windowsUpdate: "Windows Update",
  benchmark: "Benchmark",
  kernel: "Kernel (NtQuerySystemInformation)",
};

export function sourceLabel(source: Source): string {
  return source === "d3dkmt" ? translatorFor("errors")("source_d3dkmt") : SOURCE_LABELS[source];
}

export function formatPercent(value: number, fractionDigits = 0): string {
  if (!Number.isFinite(value)) return "—";
  return `${value.toFixed(fractionDigits)}%`;
}

/** Compact duration: "3 g 4 h" ("3 d 4 h" in English), "2 h 5 min", "42 s". */
export function formatDuration(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds < 0) return "—";
  const s = Math.floor(totalSeconds);
  const days = Math.floor(s / 86_400);
  const hours = Math.floor((s % 86_400) / 3_600);
  const minutes = Math.floor((s % 3_600) / 60);
  if (days > 0) return translatorFor("errors")("durationDaysHours", { days, hours });
  if (hours > 0) return `${hours} h ${minutes} min`;
  if (minutes > 0) return `${minutes} min`;
  return `${s} s`;
}

/** Technical reason a value is missing ("Avanzata"), in the current language. */
export function unavailabilityText(reason: Unavailability): string {
  const t = translatorFor("errors");
  switch (reason.kind) {
    case "notSupported":
      return t("reason_notSupported");
    case "providerMissing":
      return t("reason_providerMissing", { detail: reason.detail });
    case "permissionDenied":
      return t("reason_permissionDenied");
    case "notImplemented":
      return t("reason_notImplemented");
    case "failed":
      return t("reason_failed", { message: reason.detail.message });
  }
}

/** Formats an available metric, or returns null so the caller renders the reason. */
export function formatMetric<T>(metric: Metric<T>, format: (value: T) => string): string | null {
  return metric.status === "available" ? format(metric.value) : null;
}

/** 2 decimals below 10, 1 below 100, none above: keeps result numbers readable. */
function sig(value: number): string {
  const abs = Math.abs(value);
  return value.toFixed(abs < 10 ? 2 : abs < 100 ? 1 : 0);
}

/** Compact decimal number with SI prefix: "950", "12.3 k", "4.56 M", "1.20 G", "21.8 T". */
export function formatCompact(value: number): string {
  if (!Number.isFinite(value)) return "—";
  const abs = Math.abs(value);
  if (abs >= 1e12) return `${sig(value / 1e12)} T`;
  if (abs >= 1e9) return `${sig(value / 1e9)} G`;
  if (abs >= 1e6) return `${sig(value / 1e6)} M`;
  if (abs >= 1e3) return `${sig(value / 1e3)} k`;
  return sig(value);
}

/** Operations per second, compact: "4.56 Mop/s", "950 op/s". */
export function formatOpsPerSec(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "—";
  const compact = formatCompact(value);
  return /[kMGT]$/.test(compact) ? `${compact}op/s` : `${compact} op/s`;
}

/** Floating-point operations per second, compact SI: "21.8 TFLOP/s", "950 FLOP/s". */
export function formatFlopsPerSec(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "—";
  const compact = formatCompact(value);
  return /[kMGT]$/.test(compact) ? `${compact}FLOP/s` : `${compact} FLOP/s`;
}

/** Fan speed in revolutions per minute. `0 rpm` is a real reading (fan stopped). */
export function formatRpm(rpm: number): string {
  if (!Number.isFinite(rpm) || rpm < 0) return "—";
  return `${Math.round(rpm)} rpm`;
}

/** Latency in nanoseconds; switches to µs from 1000 ns. */
export function formatNanoseconds(ns: number): string {
  if (!Number.isFinite(ns) || ns < 0) return "—";
  if (ns >= 1000) return formatMicroseconds(ns / 1000);
  return `${sig(ns)} ns`;
}

/** Microseconds; switches to ms from 1000 µs. */
export function formatMicroseconds(us: number): string {
  if (!Number.isFinite(us) || us < 0) return "—";
  if (us >= 1000) return formatMilliseconds(us / 1000);
  return `${sig(us)} µs`;
}

/** Milliseconds; switches to seconds from 1000 ms. */
export function formatMilliseconds(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "—";
  if (ms >= 1000) return `${sig(ms / 1000)} s`;
  return `${sig(ms)} ms`;
}

/** Signed percentage change with one decimal: "+4.2%", "−3.0%", "0.0%". */
export function formatSignedPercent(value: number): string {
  if (!Number.isFinite(value)) return "—";
  const rounded = Math.round(value * 10) / 10;
  if (rounded === 0) return "0.0%";
  return `${rounded > 0 ? "+" : "−"}${Math.abs(rounded).toFixed(1)}%`;
}

/** Formats a benchmark result value according to its declared unit. */
export function formatUnitValue(value: number, unit: Unit): string {
  switch (unit) {
    case "none":
      return formatCompact(value);
    case "percent":
      return formatPercent(value, 1);
    case "bytes":
      return formatBytes(value);
    case "bytesPerSecond":
      return formatBytesPerSec(value);
    case "megahertz":
      return formatMhz(value);
    case "celsius":
      return formatCelsius(value);
    case "watts":
      return formatWatts(value);
    case "nanoseconds":
      return formatNanoseconds(value);
    case "microseconds":
      return formatMicroseconds(value);
    case "milliseconds":
      return formatMilliseconds(value);
    case "seconds":
      return Number.isFinite(value) && value >= 0 ? `${sig(value)} s` : "—";
    case "count":
      if (!Number.isFinite(value)) return "—";
      return Math.abs(value) >= 1e4 ? formatCompact(value) : String(Math.round(value));
    case "operationsPerSecond":
      return formatOpsPerSec(value);
    case "flopsPerSecond":
      return formatFlopsPerSec(value);
  }
}

/** Short technical title of an error code ("Avanzata"), in the current language. */
export function errorTitle(code: ErrorCode): string {
  return translatorFor("errors")(`title_${code}`);
}
