import { useFrame } from "@react-three/fiber";
import { useEffect, useMemo, useRef } from "react";
import { CircleGeometry, type Mesh, type MeshBasicMaterial, RingGeometry } from "three";
import { Airflow } from "@/features/device3d/scene/Airflow";
import { Laptop } from "@/features/device3d/scene/Laptop";
import { createMaterials, MaterialsContext } from "@/features/device3d/scene/materials";
import { useDisposable } from "@/features/device3d/scene/parts";
import type { StageTheme } from "@/features/device3d/types";
import { useCoolingMaterials } from "./materials";

/** Where the chip sits under the keyboard deck (laptop model space, after its forward shift). */
const CHIP: [number, number] = [0.25, -0.2];
/** Just above the keycaps, so the glow reads through the deck. */
const GLOW_Y = 0.19;
const RINGS = 3;
/** Seconds for a ring to travel from the chip to the edge of the deck. */
const PERIOD = 3.6;

/**
 * Passive cooling, as people know it: the same fanless notebook as the Overview, open. The
 * chip under the keyboard glows warm, rings of heat spread from it across the aluminium,
 * and warm particles rise off the case into the air around it: no fan, no air pushed in or
 * out. With reduced motion everything holds still.
 */
export function PassiveChassis({ theme, reducedMotion }: { theme: StageTheme; reducedMotion: boolean }) {
  const m = useCoolingMaterials();
  const device = useMemo(() => createMaterials(theme), [theme]);
  useEffect(() => () => device.dispose(), [device]);

  const ring = useDisposable(() => new RingGeometry(0.2, 0.25, 72), []);
  const core = useDisposable(() => new CircleGeometry(0.34, 48), []);
  const rings = useDisposable(() => {
    const list = Array.from({ length: RINGS }, () => m.heat.clone());
    return { list, dispose: () => list.forEach((x) => x.dispose()) };
  }, [m]);
  const refs = useRef<(Mesh | null)[]>([]);
  const time = useRef(0);

  const place = (t: number) => {
    refs.current.forEach((mesh, i) => {
      if (!mesh) return;
      const phase = (t / PERIOD + i / RINGS) % 1;
      const s = 1 + phase * 4.6;
      mesh.scale.set(s, s, 1);
      (mesh.material as MeshBasicMaterial).opacity = 0.4 * (1 - phase) * Math.min(1, phase * 6);
    });
  };

  useFrame((_, delta) => {
    if (reducedMotion) return;
    time.current += Math.min(delta, 0.1);
    place(time.current);
  });

  return (
    <MaterialsContext.Provider value={device}>
      <Laptop />
      <mesh geometry={core} material={m.heat} position={[CHIP[0], GLOW_Y, CHIP[1]]} rotation={[-Math.PI / 2, 0, 0]} />
      {rings.list.map((mat, i) => (
        <mesh
          key={i}
          ref={(el) => {
            refs.current[i] = el;
            if (el) place(time.current);
          }}
          geometry={ring}
          material={mat}
          position={[CHIP[0], GLOW_Y + 0.002 + i * 0.001, CHIP[1]]}
          rotation={[-Math.PI / 2, 0, 0]}
        />
      ))}
      <Airflow kind="laptop" mode="passive" theme={theme} moving={!reducedMotion} />
    </MaterialsContext.Provider>
  );
}
