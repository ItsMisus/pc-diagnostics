import { RoundedBox } from "@react-three/drei";
import { useMemo } from "react";
import { RoundedBoxGeometry } from "three/examples/jsm/geometries/RoundedBoxGeometry.js";
import { groupBySize, keyboardLayout } from "../keyboard";
import { useMaterials } from "./materials";
import { Instanced, Plate, useDisposable, type InstanceTransform } from "./parts";

/** Base 30.4 × 1.55 × 21.5 cm, display lid 2.08 tall, opened to 110°. */
const W = 3.04;
const D = 2.15;
const T = 0.155;
const LID_H = 2.08;
const LID_T = 0.05;
const OPEN = (110 * Math.PI) / 180;
/** Hinge axis: the barrel shows a little behind the base. */
const HINGE_Y = 0.1;
const HINGE_Z = -D / 2 + 0.005;
/** The model is built around the base; this shift centres the whole bounding box on z. */
const SHIFT_Z = 0.35;
const KEY_UNIT = 0.175;
const KEY_GAP = 0.03;
const KEY_H = 0.014;
const KEYS_Z = -0.415;

function KeyGroup({ w, d, items }: { w: number; d: number; items: InstanceTransform[] }) {
  const m = useMaterials();
  const geometry = useDisposable(() => new RoundedBoxGeometry(w, KEY_H, d, 2, 0.006), [w, d]);
  return <Instanced geometry={geometry} material={m.keycap} items={items} />;
}

/**
 * Notebook, open: satin aluminium base with a dark keyboard deck and individual keycaps
 * (one instanced mesh per cap size), a glass-like trackpad, a hinge barrel, and a thin lid
 * whose glossy black bezel frames an emissive wallpaper under a clear coat (so the studio
 * lights reflect on it).
 */
export function Laptop() {
  const m = useMaterials();
  const groups = useMemo(() => {
    const out: { id: string; w: number; d: number; items: InstanceTransform[] }[] = [];
    for (const [id, keys] of groupBySize(keyboardLayout(KEY_UNIT, KEY_GAP))) {
      const first = keys[0];
      if (!first) continue;
      out.push({ id, w: first.w, d: first.d, items: keys.map((k) => ({ position: [k.x, T + 0.0012 + KEY_H / 2, KEYS_Z + k.z] })) });
    }
    return out;
  }, []);

  return (
    <group position={[0, 0, SHIFT_Z]}>
      {/* Base */}
      <RoundedBox args={[W, T, D]} radius={0.045} smoothness={6} material={m.aluminium} position={[0, T / 2, 0]} />
      <Plate size={[2.72, 1.04]} radius={0.035} thickness={0.002} material={m.deck} position={[0, T + 0.0002, KEYS_Z]} flat />
      {groups.map((g) => (
        <KeyGroup key={g.id} w={g.w} d={g.d} items={g.items} />
      ))}
      <Plate size={[1.22, 0.74]} radius={0.04} thickness={0.002} material={m.trackpad} position={[0, T + 0.0002, 0.62]} flat />
      {/* Hinge barrel */}
      <mesh material={m.graphite} position={[0, HINGE_Y, HINGE_Z]} rotation={[0, 0, Math.PI / 2]}>
        <cylinderGeometry args={[0.055, 0.055, 2.5, 32]} />
      </mesh>
      {/* Lid: aluminium back, bezel, display, camera */}
      <group position={[0, HINGE_Y, HINGE_Z]} rotation={[-(OPEN - Math.PI / 2), 0, 0]}>
        <RoundedBox args={[W, LID_H, LID_T]} radius={0.022} smoothness={5} material={m.aluminium} position={[0, LID_H / 2 + 0.03, -LID_T / 2]} />
        <Plate size={[W - 0.05, LID_H - 0.04]} radius={0.03} thickness={0.002} material={m.gloss} position={[0, LID_H / 2 + 0.03, 0.0012]} />
        <mesh material={m.screen} position={[0, LID_H / 2 + 0.045, 0.0024]}>
          <planeGeometry args={[W - 0.2, LID_H - 0.24]} />
        </mesh>
        <mesh material={m.tongue} position={[0, LID_H - 0.035, 0.0024]}>
          <circleGeometry args={[0.012, 20]} />
        </mesh>
      </group>
    </group>
  );
}
