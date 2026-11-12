import type { Tone } from "@/components/common/StatusBadge";
import { type Translator, translatorFor } from "@/i18n";
import type { DiskHealthStatus } from "@/types/generated/DiskHealthStatus";
import type { GpuInfo } from "@/types/generated/GpuInfo";
import type { Metric } from "@/types/generated/Metric";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";

const isPhysical = (g: GpuInfo) => !g.isSoftware && !g.isVirtual;

/** First physical adapter in enumeration order (DXGI lists the primary one first). */
export function primaryGpu(gpus: readonly GpuInfo[]): GpuInfo | null {
  return gpus.find(isPhysical) ?? null;
}

/** Physical adapters first, then virtual displays, then software rasterizers. */
export function orderGpus(gpus: readonly GpuInfo[]): GpuInfo[] {
  return [
    ...gpus.filter(isPhysical),
    ...gpus.filter((g) => g.isVirtual && !g.isSoftware),
    ...gpus.filter((g) => g.isSoftware),
  ];
}

/**
 * The volume the OS boots from: `C:` on Windows (any spelling: `C:\`, `c:`, `C:/`), the
 * root `/` on macOS (other volumes are mounted under `/Volumes`).
 */
export function isSystemMount(mountPoint: string): boolean {
  const m = mountPoint.trim();
  if (m === "/") return true;
  return m.replace(/[\\/]+$/, "").toUpperCase() === "C:";
}

/**
 * How a volume is named in lists. Drive letters keep the Windows layout ("C:\ Windows").
 * POSIX mount points (macOS) put the name first, "Macintosh HD (/)", and drop a label that the
 * path already ends with ("/Volumes/Backup", not "/Volumes/Backup Backup").
 */
export function volumeDisplayName(v: Pick<VolumeInfo, "mountPoint" | "label">): string {
  const label = v.label?.trim() ?? "";
  if (!label) return v.mountPoint;
  if (!v.mountPoint.startsWith("/")) return `${v.mountPoint} ${label}`;
  return v.mountPoint.endsWith(`/${label}`) ? v.mountPoint : `${label} (${v.mountPoint})`;
}

function largest(volumes: readonly VolumeInfo[]): VolumeInfo | null {
  let best: VolumeInfo | null = null;
  for (const v of volumes) if (!best || v.totalBytes > best.totalBytes) best = v;
  return best;
}

/** `C:\` (or `/`) when present, otherwise the largest fixed volume, otherwise the largest one. */
export function systemVolume(volumes: readonly VolumeInfo[]): VolumeInfo | null {
  return (
    volumes.find((v) => isSystemMount(v.mountPoint)) ??
    largest(volumes.filter((v) => !v.removable)) ??
    largest(volumes)
  );
}

/** PCI vendor id of Apple. */
export const APPLE_VENDOR_ID = 0x106b;

/**
 * True for an Apple silicon GPU reported without dedicated memory. Those GPUs have no VRAM:
 * they share the unified memory with the CPU, so "0 dedicated" is the architecture, not a
 * missing reading. Recognised by the Apple vendor id or by the chip name ("Apple M5"), since
 * the inventory has no vendor value for Apple yet.
 */
export function hasUnifiedMemory(gpu: Pick<GpuInfo, "vendorId" | "name" | "dedicatedVramBytes">): boolean {
  const apple = gpu.vendorId === APPLE_VENDOR_ID || /^Apple M\d/.test(gpu.name.trim());
  return apple && gpu.dedicatedVramBytes === 0;
}

export function healthTone(status: DiskHealthStatus): Tone {
  switch (status) {
    case "healthy":
      return "ok";
    case "warning":
      return "warn";
    case "unhealthy":
      return "bad";
    case "unknown":
      return "neutral";
  }
}

export function healthLabel(
  status: DiskHealthStatus,
  t: Translator<"monitoring"> = translatorFor("monitoring"),
): string {
  return t(`health_${status}`);
}

/**
 * True when the OS reports a frequency limit below the nominal maximum. The 1% tolerance
 * absorbs rounding between the two counters. It says nothing about the cause.
 */
export function isFrequencyLimited(limitMhz: Metric<number>, maxMhz: Metric<number>): boolean {
  if (limitMhz.status !== "available" || maxMhz.status !== "available") return false;
  if (!(maxMhz.value > 0) || !(limitMhz.value > 0)) return false;
  return limitMhz.value < maxMhz.value * 0.99;
}
