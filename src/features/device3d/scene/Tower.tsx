import { RoundedBox } from "@react-three/drei";
import { useMemo } from "react";
import { BackSide, BoxGeometry, CatmullRomCurve3, Vector3 } from "three";
import { PRESETS } from "../presets";
import { useMaterials } from "./materials";
import { Fan, Instanced, Port, useDisposable, type InstanceTransform } from "./parts";

const [W, H, D] = PRESETS.tower.size;
/** Feet height, front fascia depth. */
const FOOT = 0.1;
const F = 0.16;
const BODY_H = H - FOOT;
const CY = FOOT + BODY_H / 2;
/** Inner face of the motherboard tray (components stand on it, towards -x). */
const BOARD_X = W / 2 - 0.08;
const FRONT = D / 2;

/**
 * Mid-tower desktop, 21 × 46 × 43 cm: dark anodized panels, a perforated front intake, an
 * I/O band with a softly lit power button, top exhaust, feet, and a tempered-glass left
 * panel that shows an abstract interior (board, air cooler, memory, graphics card, PSU
 * shroud, fans, two cables). No brand or real product is depicted.
 */
export function Tower() {
  const m = useMaterials();

  const slat = useDisposable(() => new BoxGeometry(1.46, 0.012, 0.05), []);
  const slats = useMemo<InstanceTransform[]>(() => Array.from({ length: 22 }, (_, i) => ({ position: [0, H + 0.007, -1.72 + i * 0.118] })), []);

  const fin = useDisposable(() => new BoxGeometry(0.68, 0.95, 0.01), []);
  const fins = useMemo<InstanceTransform[]>(() => Array.from({ length: 26 }, (_, i) => ({ position: [BOARD_X - 0.5, 3.35, -0.83 + i * 0.022] })), []);

  const cables = useMemo(
    () => [
      new CatmullRomCurve3([new Vector3(0.62, 0.96, 1.15), new Vector3(0.55, 1.6, 1.2), new Vector3(0.78, 2.3, 1.02), new Vector3(BOARD_X - 0.02, 2.55, 0.95)]),
      new CatmullRomCurve3([new Vector3(0.2, 0.96, -0.2), new Vector3(0.1, 1.45, -0.4), new Vector3(0.45, 1.8, -0.7), new Vector3(BOARD_X - 0.02, 1.85, -0.9)]),
    ],
    [],
  );

  return (
    <group>
      {/* Feet */}
      {[-1, 1].flatMap((sx) =>
        [-1, 1].map((sz) => (
          <mesh key={`${sx}${sz}`} material={m.rubber} position={[sx * 0.78, FOOT / 2, sz * 1.85]}>
            <cylinderGeometry args={[0.12, 0.13, FOOT, 32]} />
          </mesh>
        )),
      )}

      {/* Shell: top and bottom plates, the other panels fit between them */}
      <RoundedBox args={[W, 0.08, D]} radius={0.035} smoothness={4} material={m.graphite} position={[0, H - 0.04, 0]} />
      <RoundedBox args={[W, 0.08, D]} radius={0.035} smoothness={4} material={m.graphite} position={[0, FOOT + 0.04, 0]} />
      <RoundedBox args={[0.05, BODY_H - 0.16, D - F - 0.02]} radius={0.02} smoothness={3} material={m.graphite} position={[W / 2 - 0.025, CY, -F / 2]} />
      <RoundedBox args={[W - 0.02, BODY_H - 0.16, 0.05]} radius={0.02} smoothness={3} material={m.graphiteDeep} position={[0, CY, -D / 2 + 0.025]} />
      <RoundedBox args={[W, BODY_H - 0.16, F]} radius={0.045} smoothness={5} material={m.graphite} position={[0, CY, FRONT - F / 2]} />

      {/* Front: recessed perforated intake, separator groove, I/O band */}
      <mesh material={m.hole} position={[0, 2.06, FRONT + 0.0008]}>
        <planeGeometry args={[W - 0.28, 3.6]} />
      </mesh>
      <mesh material={m.grille} position={[0, 2.06, FRONT + 0.0016]}>
        <planeGeometry args={[W - 0.32, 3.56]} />
      </mesh>
      <mesh material={m.hole} position={[0, 3.98, FRONT + 0.0008]}>
        <planeGeometry args={[W - 0.2, 0.012]} />
      </mesh>
      <group position={[0, 4.29, FRONT]}>
        <mesh material={m.led} position={[0, 0, 0.0012]}>
          <ringGeometry args={[0.08, 0.092, 48]} />
        </mesh>
        <mesh material={m.hole} position={[0, 0, 0.0008]}>
          <circleGeometry args={[0.08, 48]} />
        </mesh>
        <mesh material={m.aluminium} position={[0, 0, 0.012]} rotation={[Math.PI / 2, 0, 0]}>
          <cylinderGeometry args={[0.07, 0.07, 0.022, 48]} />
        </mesh>
        <Port kind="usbA" position={[-0.66, 0, 0.0008]} vertical />
        <Port kind="usbA" position={[-0.5, 0, 0.0008]} vertical />
        <Port kind="usbC" position={[0.42, 0, 0.0008]} vertical />
        <Port kind="audio" position={[0.62, 0, 0.0008]} />
      </group>

      {/* Top exhaust: dark inset under fine slats */}
      <mesh material={m.hole} position={[0, H + 0.0008, -0.45]} rotation={[-Math.PI / 2, 0, 0]}>
        <planeGeometry args={[1.56, 2.66]} />
      </mesh>
      <Instanced geometry={slat} material={m.graphite} items={slats} />

      {/* Tempered glass with a black frit border and four thumbscrews */}
      <mesh material={m.glass} position={[-W / 2 + 0.02, CY, -F / 2]}>
        <boxGeometry args={[0.024, BODY_H - 0.16, D - F - 0.06]} />
      </mesh>
      <group position={[-W / 2 + 0.006, CY, -F / 2]} rotation={[0, -Math.PI / 2, 0]}>
        {(
          [
            [0, (BODY_H - 0.16) / 2 - 0.035, D - F - 0.06, 0.07],
            [0, -(BODY_H - 0.16) / 2 + 0.035, D - F - 0.06, 0.07],
          ] as const
        ).map(([x, y, w, h]) => (
          <mesh key={y} material={m.frit} position={[x, y, 0]}>
            <planeGeometry args={[w, h]} />
          </mesh>
        ))}
        {[-1, 1].map((s) => (
          <mesh key={s} material={m.frit} position={[s * ((D - F - 0.06) / 2 - 0.035), 0, 0]}>
            <planeGeometry args={[0.07, BODY_H - 0.16]} />
          </mesh>
        ))}
      </group>
      {[-1, 1].flatMap((sy) =>
        [-1, 1].map((sz) => (
          <mesh key={`${sy}${sz}`} material={m.aluminium} position={[-W / 2 - 0.004, CY + sy * 2.0, -F / 2 + sz * 1.95]} rotation={[0, 0, Math.PI / 2]}>
            <cylinderGeometry args={[0.04, 0.04, 0.026, 24]} />
          </mesh>
        )),
      )}

      {/* Interior */}
      <mesh position={[0, CY, -F / 2]}>
        <boxGeometry args={[W - 0.12, BODY_H - 0.18, D - F - 0.1]} />
        <meshStandardMaterial color={0x111215} metalness={0.2} roughness={0.85} side={BackSide} />
      </mesh>
      <mesh material={m.pcb} position={[BOARD_X + 0.01, 2.85, -0.35]}>
        <boxGeometry args={[0.02, 3.3, 3.1]} />
      </mesh>
      <RoundedBox args={[0.12, 0.22, 1.2]} radius={0.02} smoothness={2} material={m.graphite} position={[BOARD_X - 0.06, 4.15, -0.75]} />
      <RoundedBox args={[0.12, 1.0, 0.22]} radius={0.02} smoothness={2} material={m.graphite} position={[BOARD_X - 0.06, 3.45, -1.55]} />

      {/* Air cooler: base, fins, heat pipes, fan */}
      <mesh material={m.aluminium} position={[BOARD_X - 0.08, 3.35, -0.55]}>
        <boxGeometry args={[0.16, 0.42, 0.42]} />
      </mesh>
      <Instanced geometry={fin} material={m.fins} items={fins} />
      {[-0.18, -0.06, 0.06, 0.18].map((dz) => (
        <mesh key={dz} material={m.fins} position={[BOARD_X - 0.5, 3.86, -0.55 + dz]}>
          <cylinderGeometry args={[0.022, 0.022, 0.08, 16]} />
        </mesh>
      ))}
      <Fan size={0.9} position={[BOARD_X - 0.5, 3.35, -0.2]} />

      {/* Memory */}
      {[0, 1, 2, 3].map((i) => (
        <group key={i} position={[BOARD_X - 0.16, 3.35, 0.22 + i * 0.1]}>
          <mesh material={m.graphite}>
            <boxGeometry args={[0.3, 1.3, 0.045]} />
          </mesh>
          <mesh material={m.aluminium} position={[-0.005, 0.66, 0]}>
            <boxGeometry args={[0.29, 0.025, 0.05]} />
          </mesh>
        </group>
      ))}

      {/* Graphics card: shroud, backplate, a faint edge light */}
      <RoundedBox args={[1.15, 0.5, 2.9]} radius={0.04} smoothness={3} material={m.graphite} position={[BOARD_X - 0.575, 2.05, -0.62]} />
      <mesh material={m.aluminium} position={[BOARD_X - 0.575, 2.31, -0.62]}>
        <boxGeometry args={[1.1, 0.016, 2.84]} />
      </mesh>
      <mesh material={m.strip} position={[BOARD_X - 1.152, 2.12, -0.4]}>
        <boxGeometry args={[0.004, 0.022, 1.5]} />
      </mesh>

      {/* PSU shroud */}
      <RoundedBox args={[W - 0.12, 0.78, D - F - 0.1]} radius={0.03} smoothness={3} material={m.graphiteDeep} position={[0, FOOT + 0.08 + 0.39, -F / 2]} />

      {/* Fans: two intakes behind the front, one rear exhaust */}
      <Fan size={1.15} position={[0.05, 1.78, FRONT - F - 0.14]} />
      <Fan size={1.15} position={[0.05, 3.0, FRONT - F - 0.14]} />
      <Fan size={1.0} position={[0.35, 3.75, -D / 2 + 0.16]} />

      {/* Cables */}
      {cables.map((c, i) => (
        <mesh key={i} material={m.rubber}>
          <tubeGeometry args={[c, 32, 0.034, 10, false]} />
        </mesh>
      ))}

      {/* Interior light: a soft strip along the top edge, and its light */}
      <mesh material={m.strip} position={[-W / 2 + 0.07, H - 0.105, -F / 2]}>
        <boxGeometry args={[0.012, 0.012, D - F - 0.3]} />
      </mesh>
      <pointLight position={[-0.3, 4.1, 0.2]} intensity={1.6} distance={5} decay={2} color={0xe6edff} />
    </group>
  );
}
