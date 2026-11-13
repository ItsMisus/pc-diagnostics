import { useAppStore } from "@/app/store";
import { ObjectStage } from "@/components/stage3d/ObjectStage";
import { useBench } from "@/features/benchmarks/benchStore";
import { usePrefersReducedMotion } from "@/features/device3d/hooks";
import { CoolingIllustration } from "./Illustration";
import { rotorSpeed, type CoolingType } from "./model";
import type { CoolingSceneProps } from "./scene/CoolingScene";

/** Module scope: a stable loader keeps the lazy scene from reloading on every render. */
const loadScene = () => import("./scene/CoolingScene");

const CAMERA: Record<CoolingType, { distance: number; height: number }> = {
  fan: { distance: 9, height: 1.6 },
  tower: { distance: 9.5, height: 2.6 },
  aio: { distance: 10, height: 3.4 },
  passive: { distance: 9, height: 3.6 },
};

/**
 * One cooling model on the shared 3D stage. `rpm` is the measured speed (fans spin only with a
 * real reading); the passive chassis animates its heat glow. Paused while another page is
 * shown or a benchmark runs; no motion with reduced motion (the stage and the scene both
 * check it).
 */
export function CoolingStage({
  type,
  rpm = null,
  label,
  aspectRatio = "4 / 3",
  className = "",
}: {
  type: CoolingType;
  rpm?: number | null;
  label: string;
  aspectRatio?: string | null;
  className?: string;
}) {
  const reduced = usePrefersReducedMotion();
  const pageVisible = useAppStore((s) => s.section === "cooling");
  const benchmarkRunning = useBench((s) => s.activeRun !== null);
  const speed = rotorSpeed(rpm, reduced);
  const sceneProps: CoolingSceneProps = { type, speed };
  return (
    <ObjectStage
      label={label}
      loadScene={loadScene}
      sceneProps={sceneProps}
      animate={speed > 0 || type === "passive"}
      paused={!pageVisible || benchmarkRunning}
      camera={CAMERA[type]}
      fallback={<CoolingIllustration type={type} />}
      aspectRatio={aspectRatio}
      className={className}
    />
  );
}
