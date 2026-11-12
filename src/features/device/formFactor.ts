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
  /** Detection could not decide: sources disagree, none is decisive, or a read failed. */
  | { kind: "unavailable"; reason: Unavailability };

export type DeviceShapeResolution =
  /** The user chose it in Settings. */
  | { origin: "override"; shape: DeviceShape }
  /** Detected; `basis` says from what (product name, model identifier, chassis type). */
  | { origin: "detected"; shape: DeviceShape; formFactor: FormFactor; basis: FormFactorBasis | null }
  /** Inventory not loaded yet: show a neutral placeholder, do not ask yet. */
  | { origin: "pending"; shape: null }
  /** Ask the user to choose. */
  | { origin: "unknown"; shape: null; why: UnknownShapeReason };

/**
 * Resolves the model to show. `override`: `AppSettings.deviceShape` (anything that is not
 * a known shape, e.g. from an older backend, counts as no choice). `detected`: the
 * inventory's `formFactor`, or null/undefined while the inventory is loading.
 */
export function resolveDeviceShape(
  override: DeviceShape | null | undefined,
  detected: FormFactorInfo | null | undefined,
): DeviceShapeResolution {
  if (isDeviceShape(override)) return { origin: "override", shape: override };
  if (!detected) return { origin: "pending", shape: null };
  const m = detected.formFactor;
  if (m.status === "unavailable") return { origin: "unknown", shape: null, why: { kind: "unavailable", reason: m.reason } };
  const shape = shapeForFormFactor(m.value);
  if (shape === null) return { origin: "unknown", shape: null, why: { kind: "noModel", formFactor: m.value } };
  return { origin: "detected", shape, formFactor: m.value, basis: detected.basis };
}
