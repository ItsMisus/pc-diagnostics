import { useFrame } from "@react-three/fiber";
import { useRef } from "react";
import { CircleGeometry, type Mesh, type MeshBasicMaterial, RingGeometry } from "three";
import { Plate, useDisposable } from "@/features/device3d/scene/parts";
import { useCoolingMaterials } from "./materials";

const W = 3.0;
const D = 2.05;
const SOC: [number, number] = [0.35, -0.3];
const RINGS = 3;
/** Seconds for a ring to travel from the chip to the edge of the spreader. */
const PERIOD = 4.2;

/**
 * Passive cooling of a fanless laptop: the aluminium bottom case seen from inside, the logic
 * board, the system on a chip under a wide aluminium heat spreader on a graphite sheet. Warm
 * rings drift slowly outward from the chip across the spreader and fade, the way heat leaves
 * through the case. With reduced motion they hold still.
 */
export function PassiveChassis({ reducedMotion }: { reducedMotion: boolean }) {
  const m = useCoolingMaterials();
  const ring = useDisposable(() => new RingGeometry(0.2, 0.26, 72), []);
  const core = useDisposable(() => new CircleGeometry(0.42, 48), []);
  const rings = useDisposable(() => {
    const list = Array.from({ length: RINGS }, () => m.heat.clone());
    return {
      list,
      dispose: () => {
        for (const x of list) x.dispose();
      },
    };
  }, [m]);
  const refs = useRef<(Mesh | null)[]>([]);
  const time = useRef(0);

  const place = (t: number) => {
    refs.current.forEach((mesh, i) => {
      if (!mesh) return;
      const phase = (t / PERIOD + i / RINGS) % 1;
      const s = 1 + phase * 4.2;
      mesh.scale.set(s, s, 1);
      (mesh.material as MeshBasicMaterial).opacity = 0.32 * (1 - phase) * Math.min(1, phase * 6);
    });
  };

  useFrame((_, delta) => {
    if (reducedMotion) return;
    time.current += Math.min(delta, 0.1);
    place(time.current);
  });

  const y = 0.17;
  return (
    <group rotation={[0, -0.3, 0]}>
      {/* Bottom case: floor and the four walls of the unibody. */}
      <Plate size={[W, D]} radius={0.14} thickness={0.05} material={m.aluminium} position={[0, 0.025, 0]} flat />
      {[
        { p: [0, 0.11, D / 2 - 0.02], s: [W - 0.2, 0.17, 0.04] },
        { p: [0, 0.11, -D / 2 + 0.02], s: [W - 0.2, 0.17, 0.04] },
        { p: [W / 2 - 0.02, 0.11, 0], s: [0.04, 0.17, D - 0.2] },
        { p: [-W / 2 + 0.02, 0.11, 0], s: [0.04, 0.17, D - 0.2] },
      ].map((w, i) => (
        <mesh key={i} material={m.aluminium} position={w.p as [number, number, number]}>
          <boxGeometry args={w.s as [number, number, number]} />
        </mesh>
      ))}
      {/* Battery cells along the front, logic board at the back. */}
      {[-0.95, 0, 0.95].map((x) => (
        <Plate key={x} size={[0.85, 0.7]} radius={0.05} thickness={0.07} material={m.frame} position={[x, 0.09, 0.55]} flat />
      ))}
      <Plate size={[2.3, 0.85]} radius={0.06} thickness={0.025} material={m.graphite} position={[0.1, 0.07, -0.35]} flat />
      <mesh material={m.hub} position={[SOC[0], 0.1, SOC[1]]}>
        <boxGeometry args={[0.32, 0.03, 0.32]} />
      </mesh>
      {/* Heat spreader over the chip. */}
      <Plate size={[1.5, 0.66]} radius={0.05} thickness={0.018} material={m.fins} position={[SOC[0], 0.125, SOC[1]]} flat />
      <mesh geometry={core} material={m.heat} position={[SOC[0], 0.137, SOC[1]]} rotation={[-Math.PI / 2, 0, 0]} />
      {rings.list.map((mat, i) => (
        <mesh
          key={i}
          ref={(el) => {
            refs.current[i] = el;
            if (el) place(time.current);
          }}
          geometry={ring}
          material={mat}
          position={[SOC[0], y - 0.03 + i * 0.001, SOC[1]]}
          rotation={[-Math.PI / 2, 0, 0]}
        />
      ))}
    </group>
  );
}
