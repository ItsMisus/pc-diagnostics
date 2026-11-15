import { useMemo } from "react";
import { BoxGeometry, CatmullRomCurve3, TubeGeometry, Vector3 } from "three";
import { Instanced, type InstanceTransform, useDisposable } from "@/features/device3d/scene/parts";
import { AxialFan } from "./AxialFan";
import { useCoolingMaterials } from "./materials";

type V3 = [number, number, number];

const CORE_L = 2.44;
const CORE_W = 1.2;
const CORE_H = 0.27;
const SLATS = 60;
const Y = 0.9;

/**
 * An all-in-one liquid cooler: a 240 mm radiator (fin core between two end tanks) with two
 * 12 cm fans on top, and two sleeved hoses down to the pump block that sits on the
 * processor in front of it.
 */
export function AioRadiator({ speed = 0 }: { speed?: number }) {
  const m = useCoolingMaterials();
  const slat = useDisposable(() => new BoxGeometry(0.012, CORE_H * 0.9, CORE_W), []);
  const slats = useMemo<InstanceTransform[]>(
    () => Array.from({ length: SLATS }, (_, i) => ({ position: [-CORE_L / 2 + 0.03 + (i / (SLATS - 1)) * (CORE_L - 0.06), Y, 0] as V3 })),
    [],
  );
  const hoses = useDisposable(() => {
    const ends = [0.25, -0.25].map((z, i) => {
      const curve = new CatmullRomCurve3([
        new Vector3(CORE_L / 2 + 0.16, Y, z),
        new Vector3(CORE_L / 2 + 0.45, Y - 0.1, z * 1.1),
        new Vector3(CORE_L / 2 + 0.35, 0.45, 0.7 + z * 0.4),
        new Vector3(0.75 + i * 0.08, 0.22, 1.25 + z * 0.3),
        new Vector3(0.38, 0.2, 1.45 + z * 0.25),
      ]);
      return new TubeGeometry(curve, 80, 0.055, 14, false);
    });
    return {
      ends,
      dispose: () => {
        for (const g of ends) g.dispose();
      },
    };
  }, []);
  return (
    <group position={[-0.25, 0, -0.35]}>
      {/* Radiator: frame rails, fin core, end tanks. */}
      <Instanced geometry={slat} material={m.fins} items={slats} />
      {[-1, 1].map((s) => (
        <mesh key={`rail${s}`} material={m.frame} position={[0, Y, (s * (CORE_W + 0.04)) / 2]}>
          <boxGeometry args={[CORE_L, CORE_H, 0.04]} />
        </mesh>
      ))}
      {[-1, 1].map((s) => (
        <mesh key={`tank${s}`} material={m.frame} position={[(s * (CORE_L + 0.24)) / 2, Y, 0]}>
          <boxGeometry args={[0.24, CORE_H + 0.06, CORE_W + 0.08]} />
        </mesh>
      ))}
      {[-0.61, 0.61].map((x) => (
        <AxialFan key={x} size={1.2} speed={speed} position={[x, Y + CORE_H / 2 + 0.13, 0]} rotation={[-Math.PI / 2, 0, 0]} />
      ))}
      {hoses.ends.map((g, i) => (
        <mesh key={i} geometry={g} material={m.hose} />
      ))}
      {/* Pump block on the processor. */}
      <group position={[0.25, 0, 1.45]}>
        <mesh material={m.copper} position={[0, 0.03, 0]}>
          <boxGeometry args={[0.5, 0.06, 0.5]} />
        </mesh>
        <mesh material={m.frame} position={[0, 0.17, 0]}>
          <cylinderGeometry args={[0.3, 0.32, 0.22, 48]} />
        </mesh>
        <mesh material={m.aluminium} position={[0, 0.285, 0]} rotation={[-Math.PI / 2, 0, 0]}>
          <circleGeometry args={[0.26, 48]} />
        </mesh>
      </group>
    </group>
  );
}
