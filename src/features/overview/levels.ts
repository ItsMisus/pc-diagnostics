import type { MemoryPressure } from "@/types/generated/MemoryPressure";

/**
 * Status level of a dashboard meter: whether its fill stays grey (`normal`) or takes the
 * status colour, always together with a text label (docs/design-system.md, "Color").
 *
 * The thresholds mirror the diagnostics rules (crates/diagnostics/src/rules, documented in
 * docs/diagnostics-rules.md), so a coloured bar on the dashboard and a finding in Diagnostics
 * agree on what "too much" means. The dashboard reacts to the current sample, the rules to a
 * sustained window: a bar can turn amber for a moment without producing a finding.
 *
 * Only values where "high" means a problem get a level. Load (CPU, GPU, disk active time) and
 * power do not: a game at 100 % GPU is normal. Temperature does not either: there is no
 * diagnostics threshold for it, and the safe limit depends on the processor (some run at
 * 95 °C under load by design).
 */
export type Level = "normal" | "warn" | "bad";

const GIB = 1024 ** 3;

/** memory.rs: MEM_AVAILABLE_LOW_PERCENT / MEM_AVAILABLE_CRITICAL_PERCENT. */
export const MEM_AVAILABLE_WARN_PERCENT = 10;
export const MEM_AVAILABLE_BAD_PERCENT = 5;
/** memory.rs: MEM_COMMIT_HIGH_PERCENT (Warning only). */
export const MEM_COMMIT_WARN_PERCENT = 90;
/** gpu.rs: GPU_VRAM_HIGH_PERCENT (Warning only). */
export const VRAM_WARN_PERCENT = 90;
/** storage.rs: FREE_WARNING_PERCENT / FREE_CRITICAL_PERCENT / FREE_WARNING_BYTES. */
export const FREE_WARN_PERCENT = 10;
export const FREE_BAD_PERCENT = 5;
export const FREE_WARN_BYTES = 15 * GIB;
/** storage.rs: FREE_ABSOLUTE_MIN_VOLUME_BYTES, the absolute rule applies only above it. */
export const FREE_ABSOLUTE_MIN_VOLUME_BYTES = 2 * FREE_WARN_BYTES;

function share(part: number | null, total: number | null): number | null {
  return part !== null && total !== null && total > 0 ? (part / total) * 100 : null;
}

/** Physical memory, from what is still available (as the rule measures it). */
export function memoryLevel(availableBytes: number | null, totalBytes: number | null): Level {
  const free = share(availableBytes, totalBytes);
  if (free === null) return "normal";
  if (free < MEM_AVAILABLE_BAD_PERCENT) return "bad";
  if (free < MEM_AVAILABLE_WARN_PERCENT) return "warn";
  return "normal";
}

export function commitLevel(usedBytes: number | null, limitBytes: number | null): Level {
  const used = share(usedBytes, limitBytes);
  return used !== null && used >= MEM_COMMIT_WARN_PERCENT ? "warn" : "normal";
}

/**
 * macOS memory pressure: the kernel's own level, as memory.rs reads it
 * (MEM_PRESSURE_HIGH_RANK / MEM_PRESSURE_CRITICAL_RANK).
 */
export function pressureLevel(pressure: MemoryPressure | null): Level {
  if (pressure === "critical") return "bad";
  if (pressure === "warning") return "warn";
  return "normal";
}

export function vramLevel(usedBytes: number | null, totalBytes: number | null): Level {
  const used = share(usedBytes, totalBytes);
  return used !== null && used >= VRAM_WARN_PERCENT ? "warn" : "normal";
}

/** Free space on a volume: a percentage, plus 15 GiB on volumes of at least 30 GiB. */
export function volumeLevel(availableBytes: number, totalBytes: number): Level {
  const free = share(availableBytes, totalBytes);
  if (free === null) return "normal";
  if (free < FREE_BAD_PERCENT) return "bad";
  if (free < FREE_WARN_PERCENT) return "warn";
  if (totalBytes >= FREE_ABSOLUTE_MIN_VOLUME_BYTES && availableBytes < FREE_WARN_BYTES) return "warn";
  return "normal";
}
