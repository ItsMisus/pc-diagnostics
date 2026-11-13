import { useFrame } from "@react-three/fiber";
import { useEffect, useMemo, useRef } from "react";
import { ExtrudeGeometry, type Group, RingGeometry, Shape } from "three";
import { Instanced, type InstanceTransform, roundedRect, useDisposable } from "@/features/device3d/scene/parts";
import { useCoolingMaterials } from "./materials";

type V3 = [number, number, number];

const BLADES = 9;

/**
 * An axial case fan facing +z: a square frame with rounded corners and a round throat, four
 * corner mounting holes, nine swept and pitched blades on a hub with a cap. `speed` is the
 * rotor's angular speed in rad/s (0: still). While it turns, a faint disc fills the throat
 * like the blur a camera sees on a spinning fan, denser the faster it goes. `size` is the
 * frame's side (12 cm fan = 1.2).
 */
export function AxialFan({ size = 1.2, speed = 0, position, rotation }: { size?: number; speed?: number; position?: V3; rotation?: V3 }) {
  const m = useCoolingMaterials();
  const rotor = useRef<Group>(null);
  const depth = size * 0.21;
  const throat = size * 0.47;

  const frame = useDisposable(() => {
    const outer = roundedRect(size, size, size * 0.09);
    const hole = new Shape();
    hole.absarc(0, 0, throat, 0, Math.PI * 2, true);
    outer.holes.push(hole);
    const c = size * 0.42;
    for (const [x, y] of [
      [c, c],
      [-c, c],
      [c, -c],
      [-c, -c],
    ] as const) {
      const screw = new Shape();
      screw.absarc(x, y, size * 0.022, 0, Math.PI * 2, true);
      outer.holes.push(screw);
    }
    const g = new ExtrudeGeometry(outer, { depth, bevelEnabled: true, bevelThickness: 0.008, bevelSize: 0.008, bevelSegments: 3, curveSegments: 40 });
    g.translate(0, 0, -depth / 2);
    return g;
  }, [size, depth, throat]);

  const blade = useDisposable(() => {
    const r0 = size * 0.13;
    const r1 = size * 0.455;
    const s = new Shape();
    // A swept blade: narrow at the hub, wide and curved toward the tip.
    s.moveTo(r0, -size * 0.035);
    s.bezierCurveTo(r0 + (r1 - r0) * 0.35, -size * 0.09, r1 * 0.88, -size * 0.07, r1, size * 0.01);
    s.bezierCurveTo(r1 * 0.98, size * 0.12, r1 * 0.7, size * 0.17, (r0 + r1) * 0.48, size * 0.13);
    s.bezierCurveTo(r0 * 1.3, size * 0.09, r0 * 1.05, size * 0.06, r0, size * 0.035);
    s.closePath();
    return new ExtrudeGeometry(s, { depth: 0.008, bevelEnabled: true, bevelThickness: 0.003, bevelSize: 0.003, bevelSegments: 2, curveSegments: 14 });
  }, [size]);

  const blades = useMemo<InstanceTransform[]>(
    () => Array.from({ length: BLADES }, (_, i) => ({ position: [0, 0, -0.004] as V3, rotation: [0.38, 0, (i / BLADES) * Math.PI * 2] as V3 })),
    [],
  );

  const disc = useDisposable(() => new RingGeometry(size * 0.14, size * 0.455, 64), [size]);

  useEffect(() => {
    // Denser with speed, never opaque: the blades stay visible through it.
    m.blur.opacity = speed > 0 ? Math.min(0.32, 0.06 + speed / 120) : 0;
  }, [m, speed]);

  useFrame((_, delta) => {
    if (speed > 0 && rotor.current) rotor.current.rotation.z -= speed * Math.min(delta, 0.1);
  });

  return (
    <group position={position ?? [0, 0, 0]} rotation={rotation ?? [0, 0, 0]}>
      <mesh geometry={frame} material={m.frame} />
      {/* Four struts holding the motor at the back. */}
      {[0, 1, 2, 3].map((i) => (
        <mesh key={i} material={m.frame} position={[0, 0, -depth * 0.42]} rotation={[0, 0, (i * Math.PI) / 2 + Math.PI / 4]}>
          <boxGeometry args={[size * 0.62, size * 0.025, depth * 0.12]} />
        </mesh>
      ))}
      <group ref={rotor}>
        <Instanced geometry={blade} material={m.blade} items={blades} />
        <mesh material={m.hub} rotation={[Math.PI / 2, 0, 0]}>
          <cylinderGeometry args={[size * 0.13, size * 0.13, depth * 0.7, 40]} />
        </mesh>
        <mesh material={m.aluminium} position={[0, 0, depth * 0.351]}>
          <circleGeometry args={[size * 0.085, 40]} />
        </mesh>
      </group>
      {speed > 0 && <mesh geometry={disc} material={m.blur} position={[0, 0, depth * 0.2]} />}
    </group>
  );
}
