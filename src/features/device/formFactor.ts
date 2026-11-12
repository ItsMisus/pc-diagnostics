import type { AppError } from "@/types/generated/AppError";
import type { DeviceShape } from "@/types/generated/DeviceShape";
import type { FormFactor } from "@/types/generated/FormFactor";
import type { FormFactorBasis } from "@/types/generated/FormFactorBasis";
import type { FormFactorInfo } from "@/types/generated/FormFactorInfo";
import type { Unavailability } from "@/types/generated/Unavailability";

/**
 * Which 3D device model the Overview shows: tower, laptop or mini PC.
 *
 * Order: the user's choice in Settings (`AppSettings.deviceShape`) > the detected form
 * factor (`HardwareInventory.formFactor`) > "unknown", in which case the UI asks the user
 * instead of picking a model. Never derived from the window size.
 */

/** Every model, in the order a picker lists them. */
export const DEVICE_SHAPES: readonly DeviceShape[] = ["tower", "laptop", "mini"];

export function isDeviceShape(value: unknown): value is DeviceShape {
  return typeof value === "string" && (DEVICE_SHAPES as readonly string[]).includes(value);
}

/** The model for a detected form factor; null for all-in-one and "other" (no model). */
export function shapeForFormFactor(formFactor: FormFactor): DeviceShape | null {
  switch (formFactor) {
    case "desktop":
      return "tower";
    case "laptop":
      return "laptop";
    case "mini":
      return "mini";
    case "allInOne":
    case "other":
      return null;
  }
}

export type UnknownShapeReason =
  /** Detection worked, but there is no 3D model for it (all-in-one, rack, ...). */
  | { kind: "noModel"; formFactor: FormFactor }
  /** Detection could not decide: sources disagree, none is decisive, a read failed, or the inventory failed to load. */
  | { kind: "unavailable"; reason: Unavailability };

export type DeviceShapeResolution =
  /** The user chose it in Settings. */
  | { origin: "override"; shape: DeviceShape }
  /** Detected; `basis` says from what (product name, model identifier, chassis type). */
  | { origin: "detected"; shape: DeviceShape; formFactor: FormFactor; basis: FormFactorBasis | null }
  /**
   * Settings or inventory still loading: show a neutral placeholder, do not ask yet and
   * do not show the detected model (a saved choice may still replace it).
   */
  | { origin: "pending"; shape: null }
  /** Ask the user to choose. */
  | { origin: "unknown"; shape: null; why: UnknownShapeReason };

/** What the resolver needs from the settings and inventory stores. */
export interface DeviceShapeInput {
  /**
   * The saved choice (`AppSettings.deviceShape`). `"loading"` until the settings are
   * known; `null` for no choice, and also when the settings failed to load (then detection
   * decides). Anything that is not a known shape (e.g. from an older backend) counts as no
   * choice.
   */
  override: DeviceShape | null | "loading";
  /** The inventory's `formFactor`, or null while it is loading or after it failed. */
  detected: FormFactorInfo | null | undefined;
  /** Set when loading the inventory failed: nothing more is coming, so ask the user. */
  inventoryError?: AppError | null;
}

/** Resolves the model to show: override > detected > unknown (ask the user). */
export function resolveDeviceShape({ override, detected, inventoryError }: DeviceShapeInput): DeviceShapeResolution {
  if (isDeviceShape(override)) return { origin: "override", shape: override };
  if (override === "loading") return { origin: "pending", shape: null };
  if (!detected) {
    if (inventoryError) {
      return { origin: "unknown", shape: null, why: { kind: "unavailable", reason: { kind: "failed", detail: inventoryError } } };
    }
    return { origin: "pending", shape: null };
  }
  const m = detected.formFactor;
  if (m.status === "unavailable") return { origin: "unknown", shape: null, why: { kind: "unavailable", reason: m.reason } };
  const shape = shapeForFormFactor(m.value);
  if (shape === null) return { origin: "unknown", shape: null, why: { kind: "noModel", formFactor: m.value } };
  return { origin: "detected", shape, formFactor: m.value, basis: detected.basis };
}
