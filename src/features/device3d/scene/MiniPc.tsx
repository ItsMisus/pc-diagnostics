import { RoundedBox } from "@react-three/drei";
import { useMemo } from "react";
import { PlaneGeometry } from "three";
import { PRESETS } from "../presets";
import { useMaterials } from "./materials";
import { Instanced, Plate, Port, useDisposable, type InstanceTransform } from "./parts";

const [W, H, D] = PRESETS.mini.size;
const FOOT = 0.02;
const BODY_H = H - FOOT;
const CY = FOOT + BODY_H / 2;
const PORT_Y = CY - 0.01;

/**
 * Compact desktop, 12.7 × 4.3 × 12.7 cm: a softly rounded aluminium shell, a satin dark top,
 * front ports with a power button and a small status light, rear I/O, side vent slots and a
 * recessed rubber foot.
 */
export function MiniPc() {
  const m = useMaterials();
  const slot = useDisposable(() => new PlaneGeometry(0.018, 0.13), []);
  const slots = useMemo<InstanceTransform[]>(() => {
    const out: InstanceTransform[] = [];
    for (const side of [-1, 1]) {
      for (let i = 0; i < 13; i++) {
        out.push({ position: [side * (W / 2 + 0.0006), CY, -0.33 + i * 0.055], rotation: [0, (side * Math.PI) / 2, 0] });
      }
    }
    return out;
  }, []);

  return (
    <group>
      <Plate size={[W - 0.3, D - 0.3]} radius={0.12} thickness={FOOT + 0.01} material={m.rubber} position={[0, (FOOT + 0.01) / 2, 0]} flat />
      <RoundedBox args={[W, BODY_H, D]} radius={0.11} smoothness={8} material={m.aluminium} position={[0, CY, 0]} />
      <Plate size={[W - 0.3, D - 0.3]} radius={0.06} thickness={0.004} material={m.graphite} position={[0, H + 0.0012, 0]} flat />

      {/* Front I/O, power button and status light */}
      <group position={[0, PORT_Y, D / 2]}>
        <Port kind="usbC" position={[-0.44, 0, 0.0006]} />
        <Port kind="usbC" position={[-0.32, 0, 0.0006]} />
        <Port kind="usbA" position={[-0.15, 0, 0.0006]} />
        <Port kind="audio" position={[0.02, 0, 0.0006]} />
        <mesh material={m.hole} position={[0.4, 0, 0.0006]}>
          <circleGeometry args={[0.04, 32]} />
        </mesh>
        <mesh material={m.aluminium} position={[0.4, 0, 0.006]} rotation={[Math.PI / 2, 0, 0]}>
          <cylinderGeometry args={[0.034, 0.034, 0.01, 32]} />
        </mesh>
        <mesh material={m.led} position={[0.5, 0, 0.0008]}>
          <circleGeometry args={[0.008, 16]} />
        </mesh>
      </group>

      {/* Rear I/O */}
      <group position={[0, PORT_Y, -D / 2]} rotation={[0, Math.PI, 0]}>
        <Port kind="dc" position={[-0.44, 0, 0.0006]} />
        <Port kind="ethernet" position={[-0.24, 0, 0.0006]} />
        <Port kind="hdmi" position={[-0.03, 0.03, 0.0006]} />
        <Port kind="hdmi" position={[-0.03, -0.04, 0.0006]} />
        <Port kind="usbA" position={[0.17, 0.03, 0.0006]} />
        <Port kind="usbA" position={[0.17, -0.04, 0.0006]} />
        <Port kind="usbC" position={[0.36, 0, 0.0006]} />
      </group>

      {/* Side vents */}
      <Instanced geometry={slot} material={m.hole} items={slots} />
    </group>
  );
}
