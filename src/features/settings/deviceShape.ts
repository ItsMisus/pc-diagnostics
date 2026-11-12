import { DEVICE_SHAPES, type DeviceShapeResolution, resolveDeviceShape } from "@/features/device/formFactor";
import type { AppError } from "@/types/generated/AppError";
import type { DeviceShape } from "@/types/generated/DeviceShape";
import type { FormFactorBasis } from "@/types/generated/FormFactorBasis";
import type { FormFactorInfo } from "@/types/generated/FormFactorInfo";

/** The Settings picker: "Automatico" (follow detection) or one of the models. */
export type ShapeChoice = "auto" | DeviceShape;

export const SHAPE_CHOICES: readonly ShapeChoice[] = ["auto", ...DEVICE_SHAPES];

/** Picker value for the stored choice (`null` = follow detection). */
export function choiceForOverride(override: DeviceShape | null): ShapeChoice {
  return override ?? "auto";
}

/** Value to store for a picker choice. */
export function overrideForChoice(choice: ShapeChoice): DeviceShape | null {
  return choice === "auto" ? null : choice;
}

/** What detection alone says, whatever the user chose: the line under the picker. */
export type DetectionResult = Exclude<DeviceShapeResolution, { origin: "override" }>;

export function detectionResult(detected: FormFactorInfo | null | undefined, inventoryError: AppError | null): DetectionResult {
  const r = resolveDeviceShape({ override: null, detected, inventoryError });
  // With no override the resolver never returns "override"; narrow the type for callers.
  return r.origin === "override" ? { origin: "pending", shape: null } : r;
}

/**
 * The raw value behind the detection, so the user can check it: the macOS product name or
 * model identifier, or the Windows SMBIOS chassis type codes. Follows `basis` when known.
 */
export function detectionEvidence(info: FormFactorInfo | null | undefined): { kind: FormFactorBasis; value: string } | null {
  if (!info) return null;
  const product = info.productName?.trim() || null;
  const model = info.modelIdentifier?.trim() || null;
  const chassis = info.chassisTypes.length > 0 ? info.chassisTypes.join(", ") : null;
  const pick = (kind: FormFactorBasis) =>
    kind === "productName" ? product : kind === "modelIdentifier" ? model : chassis;
  if (info.basis) {
    const value = pick(info.basis);
    return value ? { kind: info.basis, value } : null;
  }
  for (const kind of ["productName", "modelIdentifier", "chassisType"] as const) {
    const value = pick(kind);
    if (value) return { kind, value };
  }
  return null;
}
