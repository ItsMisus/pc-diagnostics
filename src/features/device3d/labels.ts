import type { Translator } from "@/i18n";
import type { DeviceKind, StageMode } from "./types";

/** Why the static illustration is shown instead of the 3D scene. */
export type FallbackReason = "noWebgl" | "contextLost";

export interface StageLabels {
  /** Accessible name of the stage. */
  name: string;
  /** Accessible description: how to rotate, what is loading, or why there is no 3D. */
  description: string;
}

/** Accessible name and description for a mode (pure: the caller passes the translator). */
export function stageLabels(t: Translator<"device3d">, kind: DeviceKind, mode: StageMode, reason: FallbackReason = "noWebgl"): StageLabels {
  if (mode === "fallback") {
    return { name: t(`illustration_${kind}`), description: t(reason) };
  }
  return { name: t(`model_${kind}`), description: mode === "loading" ? t("loading") : t("rotateHint") };
}
