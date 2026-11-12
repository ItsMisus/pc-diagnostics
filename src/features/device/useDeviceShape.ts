import { useCallback, useMemo } from "react";
import { useAppStore } from "@/app/store";
import { useInventory } from "@/features/monitoring/inventoryStore";
import { useInventoryData } from "@/features/monitoring/hooks";
import type { AppError } from "@/types/generated/AppError";
import type { DeviceShape } from "@/types/generated/DeviceShape";
import { type DeviceShapeResolution, isDeviceShape, resolveDeviceShape } from "./formFactor";

/**
 * The device model to show (see `resolveDeviceShape`), plus a setter for the user's
 * choice: `setShape(null)` goes back to the detected form factor. The choice is saved in
 * the settings (`deviceShape`).
 */
export function useDeviceShape(): {
  resolution: DeviceShapeResolution;
  /** The stored choice, null when following detection or when settings are not loaded. */
  override: DeviceShape | null;
  /** False while the settings are loading or failed to load: a picker should be disabled. */
  canSet: boolean;
  /** A save is in flight. */
  saving: boolean;
  /** Why the last save failed (the choice was rolled back); null otherwise. */
  saveError: AppError | null;
  /**
   * Saves the choice. Resolves true once stored, false when it was not saved (settings
   * not ready, or the save failed: see `saveError`).
   */
  setShape: (shape: DeviceShape | null) => Promise<boolean>;
} {
  const settings = useAppStore((s) => s.settings);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const { status, data } = useInventoryData();
  const inventoryError = useInventory((s) => s.error);
  const detected = data?.formFactor;

  const stored = settings.status === "ready" ? settings.value.deviceShape : null;
  const override: DeviceShape | null = isDeviceShape(stored) ? stored : null;
  const overrideInput = settings.status === "loading" ? "loading" : override;
  const failed = status === "error" ? inventoryError : null;

  const resolution = useMemo(
    () => resolveDeviceShape({ override: overrideInput, detected, inventoryError: failed }),
    [overrideInput, detected, failed],
  );

  const setShape = useCallback(
    async (shape: DeviceShape | null) => {
      if (useAppStore.getState().settings.status !== "ready") return false;
      await updateSettings({ deviceShape: shape });
      const after = useAppStore.getState().settings;
      return after.status === "ready" && after.saveError === null && (after.value.deviceShape ?? null) === shape;
    },
    [updateSettings],
  );

  return {
    resolution,
    override,
    canSet: settings.status === "ready",
    saving: settings.status === "ready" && settings.saving,
    saveError: settings.status === "ready" ? settings.saveError : null,
    setShape,
  };
}
