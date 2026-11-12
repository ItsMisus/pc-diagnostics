import { useMemo } from "react";
import type { IconName } from "@/components/common/Icon";
import { Segmented, type SegmentedOption } from "@/components/common/Segmented";
import { StateView } from "@/components/common/StateView";
import { stateKindForError } from "@/lib/errors";
import { useT } from "@/i18n";
import type { DeviceShape } from "@/types/generated/DeviceShape";
import { DEVICE_SHAPES } from "./formFactor";
import { useDeviceShape } from "./useDeviceShape";

type PickerValue = DeviceShape | "auto";

/** Glyph of each device model, from the one icon family. */
export const SHAPE_ICON: Record<DeviceShape, IconName> = { tower: "desktop", laptop: "laptop", mini: "miniPc" };

/**
 * Which 3D device model to show: "Automatico" (follow the detected form factor, i.e. no saved
 * choice) or one of the models. Saves to the settings (`deviceShape`) through
 * `useDeviceShape()`, so the Overview and Settings stay in sync. Disabled until the settings
 * are loaded; a failed save rolls back and is stated under the control.
 *
 * Never derives anything from the window size: the choice is the user's.
 */
export function DeviceShapePicker({
  size = "sm",
  fullWidth = false,
  className = "",
  label,
}: {
  size?: "sm" | "md";
  fullWidth?: boolean;
  className?: string;
  /** Accessible name of the group (default "Modello del dispositivo"). */
  label?: string;
}) {
  const t = useT("overview");
  const { override, canSet, saveError, setShape } = useDeviceShape();
  const options = useMemo<SegmentedOption<PickerValue>[]>(
    () => [
      { value: "auto", label: t("shapeAuto"), disabled: !canSet },
      ...DEVICE_SHAPES.map((s) => ({ value: s, label: t(`shape_${s}`), icon: SHAPE_ICON[s], disabled: !canSet })),
    ],
    [t, canSet],
  );
  return (
    <div className={`flex min-w-0 flex-col gap-2 ${className}`}>
      <Segmented<PickerValue>
        label={label ?? t("shapeTitle")}
        options={options}
        value={override ?? "auto"}
        onChange={(v) => void setShape(v === "auto" ? null : v)}
        size={size}
        fullWidth={fullWidth}
        className="max-w-full"
      />
      {saveError && <StateView compact kind={stateKindForError(saveError)} title={t("shapeSaveFailed")} error={saveError} />}
    </div>
  );
}
