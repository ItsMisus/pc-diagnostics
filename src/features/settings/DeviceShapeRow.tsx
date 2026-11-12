import { useMemo } from "react";
import { Button } from "@/components/common/Button";
import type { IconName } from "@/components/common/Icon";
import { useUnavailabilityText } from "@/components/common/MetricValue";
import { Segmented } from "@/components/common/Segmented";
import { Spinner } from "@/components/common/StateView";
import { useDeviceShape } from "@/features/device/useDeviceShape";
import { useInventory } from "@/features/monitoring/inventoryStore";
import { useInventoryData } from "@/features/monitoring/hooks";
import { useIsSimple } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import type { DeviceShape } from "@/types/generated/DeviceShape";
import { Row } from "./controls";
import {
  SHAPE_CHOICES,
  type ShapeChoice,
  choiceForOverride,
  detectionEvidence,
  detectionResult,
  overrideForChoice,
} from "./deviceShape";

const SHAPE_ICONS: Record<DeviceShape, IconName> = { tower: "desktop", laptop: "laptop", mini: "miniPc" };

/**
 * "Modello del dispositivo": Automatico (follow detection) or a fixed model for the Overview's
 * 3D device. Under the label the row always says what detection found and from what, so the
 * user can tell whether "Automatico" is right; when detection could not decide it says why.
 */
export function DeviceShapeRow() {
  const t = useT("settings");
  const { override, canSet, setShape } = useDeviceShape();
  const { data } = useInventoryData();
  const inventoryError = useInventory((s) => (s.status === "error" ? s.error : null));
  const reloadInventory = useInventory((s) => s.reload);
  const reasonText = useUnavailabilityText();
  const simple = useIsSimple();

  const info = data?.formFactor;
  const detection = useMemo(() => detectionResult(info, inventoryError), [info, inventoryError]);
  const evidence = useMemo(() => detectionEvidence(info), [info]);

  const options = useMemo(
    () =>
      SHAPE_CHOICES.map((value) => ({
        value,
        label: t(`shape_${value}`),
        disabled: !canSet,
        ...(value === "auto" ? {} : { icon: SHAPE_ICONS[value] }),
      })),
    [t, canSet],
  );

  let status;
  switch (detection.origin) {
    case "pending":
      status = (
        <span className="inline-flex items-center gap-1.5">
          <Spinner size={12} />
          {t("detecting")}
        </span>
      );
      break;
    case "detected": {
      const shape = t(`shape_${detection.shape}`);
      status = detection.basis
        ? t("detectedAs", { shape, basis: t(`basis_${detection.basis}`) })
        : t("detectedAsNoBasis", { shape });
      break;
    }
    case "unknown":
      status =
        detection.why.kind === "noModel"
          ? t(detection.why.formFactor === "allInOne" ? "noModel_allInOne" : "noModel_other")
          : t("notDetected", { reason: reasonText(detection.why.reason) });
      break;
  }

  // Chassis codes are technical: only in "Avanzata". Product names and model ids are readable.
  const showEvidence = evidence !== null && detection.origin !== "pending" && !(simple && evidence.kind === "chassisType");

  return (
    <Row
      label={t("deviceModel")}
      hint={
        <>
          <span className="block">{t("deviceModelHint")}</span>
          <span className="mt-1 block font-medium text-fg" aria-live="polite">
            {status}
          </span>
          {showEvidence && (
            <span className="block break-words text-fg-subtle">
              {t(`evidence_${evidence.kind}`, { value: evidence.value })}
            </span>
          )}
          {inventoryError && (
            <Button size="sm" variant="ghost" icon="refresh" className="mt-1.5 -ml-3" onClick={reloadInventory}>
              {t("retryDetection")}
            </Button>
          )}
        </>
      }
    >
      <Segmented<ShapeChoice>
        label={t("deviceModel")}
        options={options}
        value={choiceForOverride(override)}
        onChange={(choice) => void setShape(overrideForChoice(choice))}
      />
    </Row>
  );
}
