import { useThree } from "@react-three/fiber";
import { useEffect, useMemo } from "react";
import type { StageSceneProps } from "@/components/stage3d/ObjectStage";
import type { CoolingType } from "../model";
import { AioRadiator } from "./AioRadiator";
import { AxialFan } from "./AxialFan";
import { CoolingMaterialsContext, createCoolingMaterials } from "./materials";
import { PassiveChassis } from "./PassiveChassis";
import { TowerCooler } from "./TowerCooler";

export interface CoolingSceneProps {
  type: CoolingType;
  /** Rotor speed in rad/s (`rotorSpeed`): 0 keeps every fan still. */
  speed: number;
}

/** Pose of each model in the stage: centred on the origin, a three-quarter view. */
const POSE: Record<CoolingType, { position: [number, number, number]; rotation: [number, number, number]; scale: number }> = {
  fan: { position: [0, 0, 0], rotation: [-0.12, -0.5, 0], scale: 2 },
  tower: { position: [0, -1.05, 0], rotation: [0, -0.62, 0], scale: 1.05 },
  aio: { position: [0, -0.55, 0], rotation: [0.1, -0.5, 0], scale: 1 },
  passive: { position: [0, -0.95, 0], rotation: [0, -0.55, 0], scale: 1.05 },
};

/**
 * The cooling models in the shared `ObjectStage` (studio lighting of the Overview). One model
 * per canvas; the materials are made once and freed with the canvas.
 */
export default function CoolingScene({ type, speed, theme, reducedMotion }: CoolingSceneProps & StageSceneProps) {
  const materials = useMemo(() => createCoolingMaterials(theme), [theme]);
  useEffect(() => () => materials.dispose(), [materials]);
  const invalidate = useThree((s) => s.invalidate);
  useEffect(() => invalidate(), [invalidate, type, speed, materials]);
  const pose = POSE[type];
  const spin = reducedMotion ? 0 : speed;
  return (
    <CoolingMaterialsContext.Provider value={materials}>
      <group position={pose.position} rotation={pose.rotation} scale={pose.scale}>
        {type === "fan" && <AxialFan size={1.2} speed={spin} />}
        {type === "tower" && <TowerCooler speed={spin} />}
        {type === "aio" && <AioRadiator speed={spin} />}
        {type === "passive" && <PassiveChassis theme={theme} reducedMotion={reducedMotion} />}
      </group>
    </CoolingMaterialsContext.Provider>
  );
}
