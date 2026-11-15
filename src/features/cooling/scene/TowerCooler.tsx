import { useMemo } from "react";
import { BoxGeometry, CatmullRomCurve3, TubeGeometry, Vector3 } from "three";
import { Instanced, type InstanceTransform, Plate, useDisposable } from "@/features/device3d/scene/parts";
import { AxialFan } from "./AxialFan";
import { useCoolingMaterials } from "./materials";

type V3 = [number, number, number];

const FINS = 34;
const FIN_W = 1.25;
const FIN_D = 0.5;
const STACK_BOTTOM = 0.45;
const STACK_TOP = 1.95;

/**
 * A single-tower CPU cooler: a copper base on the processor, four copper heat pipes rising in
 * U shapes through a stack of thin aluminium fins, a brushed top cover and a 12 cm fan
 * clipped to the front of the stack. Base at y = 0.
 */
export function TowerCooler({ speed = 0 }: { speed?: number }) {
  const m = useCoolingMaterials();
  const fin = useDisposable(() => new BoxGeometry(FIN_W, 0.012, FIN_D), []);
  const fins = useMemo<InstanceTransform[]>(
    () => Array.from({ length: FINS }, (_, i) => ({ position: [0, STACK_BOTTOM + (i / (FINS - 1)) * (STACK_TOP - STACK_BOTTOM), 0] as V3 })),
    [],
  );
  const pipes = useDisposable(
    () => {
      // Four U-shaped pipes: down one side of the stack, through the base, up the other.
      const geometries = [-0.3, -0.1, 0.1, 0.3].map((z, i) => {
        const x = 0.2 + i * 0.09;
        const curve = new CatmullRomCurve3([
          new Vector3(-x, STACK_TOP - 0.05, z * 0.6),
          new Vector3(-x, 0.35, z * 0.6),
          new Vector3(-x * 0.55, 0.1, z),
          new Vector3(0, 0.07, z),
          new Vector3(x * 0.55, 0.1, z),
          new Vector3(x, 0.35, z * 0.6),
          new Vector3(x, STACK_TOP - 0.05, z * 0.6),
        ]);
        return new TubeGeometry(curve, 96, 0.03, 12, false);
      });
      return {
        geometries,
        dispose: () => {
          for (const g of geometries) g.dispose();
        },
      };
    },
    [],
  );
  return (
    <group>
      {/* Copper base and its mounting bracket. */}
      <mesh material={m.copper} position={[0, 0.04, 0]}>
        <boxGeometry args={[0.44, 0.08, 0.44]} />
      </mesh>
      <mesh material={m.aluminium} position={[0, 0.12, 0]}>
        <boxGeometry args={[0.7, 0.07, 0.3]} />
      </mesh>
      {pipes.geometries.map((g, i) => (
        <mesh key={i} geometry={g} material={m.copper} />
      ))}
      <Instanced geometry={fin} material={m.fins} items={fins} />
      <Plate size={[FIN_W + 0.04, FIN_D + 0.04]} radius={0.05} thickness={0.05} material={m.aluminium} position={[0, STACK_TOP + 0.04, 0]} flat />
      <AxialFan size={1.2} speed={speed} position={[0, (STACK_BOTTOM + STACK_TOP) / 2, FIN_D / 2 + 0.14]} />
    </group>
  );
}
