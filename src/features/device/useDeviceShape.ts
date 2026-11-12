import { useCallback, useMemo } from "react";
import { useAppStore } from "@/app/store";
import { useInventoryData } from "@/features/monitoring/hooks";
import type { DeviceShape } from "@/types/generated/DeviceShape";
import { type DeviceShapeResolution, resolveDeviceShape } from "./formFactor";

/**
 * The device model to show (see `resolveDeviceShape`), plus a setter for the user's
 * choice: `setShape(null)` goes back to the detected form factor. The choice is saved in
 * the settings (`deviceShape`); it is a no-op while the settings are not loaded.
 */
export function useDeviceShape(): {
  resolution: DeviceShapeResolution;
  /** The stored choice, null when following detection (or settings not loaded). */
  override: DeviceShape | null;
  setShape: (shape: DeviceShape | null) => void;
} {
  const override = useAppStore((s) => (s.settings.status === "ready" ? s.settings.value.deviceShape : null));
  const updateSettings = useAppStore((s) => s.updateSettings);
  const { data } = useInventoryData();
  const detected = data?.formFactor;
  const resolution = useMemo(() => resolveDeviceShape(override, detected), [override, detected]);
  const setShape = useCallback(
    (shape: DeviceShape | null) => void updateSettings({ deviceShape: shape }),
    [updateSettings],
  );
  return { resolution, override: override ?? null, setShape };
}
