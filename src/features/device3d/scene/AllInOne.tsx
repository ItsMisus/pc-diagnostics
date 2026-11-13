import { RoundedBox } from "@react-three/drei";
import { PRESETS } from "../presets";
import { useMaterials } from "./materials";
import { Plate } from "./parts";

const [W, H] = PRESETS.allInOne.size;
const PANEL_T = 0.12;
const PANEL_BOTTOM = 0.75;
const PANEL_H = H - PANEL_BOTTOM;
const PANEL_Z = 0.3;
const BEZEL = 0.12;
const CHIN = 0.28;
const SCREEN_H = PANEL_H - BEZEL * 2 - CHIN;
const FOOT: readonly [number, number, number] = [1.8, 0.05, 1.4];
const ARM: readonly [number, number, number] = [1.4, 1.35, 0.07];
const ARM_TILT = 0.45;

/**
 * All-in-one computer, 54 cm wide: a thin aluminium display body with a glossy black front,
 * an emissive wallpaper, a graphite chin with a status light, and a leaning arm on a flat
 * foot. A generic shape (no brand, no product colours).
 */
export function AllInOne() {
  const m = useMaterials();
  const panelY = PANEL_BOTTOM + PANEL_H / 2;
  return (
    <group>
      {/* Stand: foot plate and leaning arm. */}
      <RoundedBox args={FOOT as [number, number, number]} radius={0.02} smoothness={4} material={m.aluminium} position={[0, FOOT[1] / 2, 0]} />
      <RoundedBox
        args={ARM as [number, number, number]}
        radius={0.03}
        smoothness={4}
        material={m.aluminium}
        position={[0, 0.7, -0.07]}
        rotation={[ARM_TILT, 0, 0]}
      />
      {/* Display body. */}
      <RoundedBox args={[W, PANEL_H, PANEL_T]} radius={0.05} smoothness={6} material={m.aluminium} position={[0, panelY, PANEL_Z]} />
      <Plate
        size={[W - 0.04, PANEL_H - 0.04]}
        radius={0.04}
        thickness={0.002}
        material={m.gloss}
        position={[0, panelY, PANEL_Z + PANEL_T / 2 + 0.0012]}
      />
      <mesh material={m.screen} position={[0, PANEL_BOTTOM + CHIN + BEZEL + SCREEN_H / 2, PANEL_Z + PANEL_T / 2 + 0.0026]}>
        <planeGeometry args={[W - BEZEL * 2, SCREEN_H]} />
      </mesh>
      {/* Chin. */}
      <Plate
        size={[W - 0.04, CHIN]}
        radius={0.03}
        thickness={0.003}
        material={m.graphite}
        position={[0, PANEL_BOTTOM + 0.02 + CHIN / 2, PANEL_Z + PANEL_T / 2 + 0.0028]}
      />
      <mesh material={m.led} position={[W / 2 - 0.3, PANEL_BOTTOM + 0.02 + CHIN / 2, PANEL_Z + PANEL_T / 2 + 0.0062]}>
        <circleGeometry args={[0.012, 16]} />
      </mesh>
      {/* Camera. */}
      <mesh material={m.tongue} position={[0, H - BEZEL / 2, PANEL_Z + PANEL_T / 2 + 0.0026]}>
        <circleGeometry args={[0.014, 20]} />
      </mesh>
    </group>
  );
}
