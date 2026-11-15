import { RoundedBox } from "@react-three/drei";
import { useFrame, useThree } from "@react-three/fiber";
import { useEffect, useLayoutEffect, useMemo, useRef, type RefObject } from "react";
import {
  BoxGeometry,
  CircleGeometry,
  Color,
  DoubleSide,
  ExtrudeGeometry,
  type Group,
  type InstancedMesh,
  MeshBasicMaterial,
  MeshPhysicalMaterial,
  MeshStandardMaterial,
  Object3D,
  PlaneGeometry,
  type PointLight,
  Shape,
  SRGBColorSpace,
} from "three";
import { RoundedBoxGeometry } from "three/examples/jsm/geometries/RoundedBoxGeometry.js";
import type { StageSceneProps } from "@/components/stage3d/ObjectStage";
import type { LightingEffect } from "@/types/generated/LightingEffect";
import { averageColor } from "../colorMath";
import { stillTime } from "../preview";
import { lampColors, modelLayout, type ModelLayout, type StageModel } from "./layouts";

export interface LightingSceneProps {
  model: StageModel;
  effect: LightingEffect;
  brightness: number;
  /** Animate the effect (forced off with reduced motion by the stage). */
  playing: boolean;
}

/** Materials of the lighting stage, created once per scene and freed on unmount. */
function useStageMaterials(theme: StageSceneProps["theme"]) {
  const mats = useMemo(
    () => ({
      body: new MeshPhysicalMaterial({ color: 0x1d1f23, metalness: 0.55, roughness: 0.42, clearcoat: 0.4, clearcoatRoughness: 0.3 }),
      deck: new MeshStandardMaterial({ color: 0x0b0c0e, metalness: 0.2, roughness: 0.75 }),
      cap: new MeshPhysicalMaterial({ color: 0x15161a, metalness: 0.05, roughness: 0.5, transmission: 0, clearcoat: 0.2 }),
      aluminium: new MeshPhysicalMaterial({ color: 0xc9ccd1, metalness: 1, roughness: 0.32, clearcoat: 0.08 }),
      shell: new MeshPhysicalMaterial({ color: 0x17181b, metalness: 0.3, roughness: 0.28, clearcoat: 0.8, clearcoatRoughness: 0.12 }),
      blade: new MeshPhysicalMaterial({ color: 0xe9ecf2, metalness: 0, roughness: 0.25, transmission: 0.6, thickness: 0.05, transparent: true, opacity: 0.85, side: DoubleSide }),
      pcb: new MeshStandardMaterial({ color: 0x111215, metalness: 0.4, roughness: 0.6 }),
      // Light: unlit, outside tone mapping, so colors stay as the user chose them.
      lamp: new MeshBasicMaterial({ color: 0xffffff, toneMapped: false }),
      halo: new MeshBasicMaterial({ color: 0xffffff, toneMapped: false, transparent: true, opacity: 0.5, depthWrite: false }),
    }),
    [],
  );
  useEffect(() => {
    mats.body.color.set(theme === "dark" ? 0x1d1f23 : 0x2a2d33);
  }, [mats, theme]);
  useEffect(() => () => Object.values(mats).forEach((m) => m.dispose()), [mats]);
  return mats;
}

const tmp = new Color();

/**
 * Lamps of a model as instanced meshes whose per-instance color comes from the shared effect
 * math each frame (only while playing; otherwise once per change). `halo` adds a larger, soft
 * plane behind each lamp so light spills into the gaps.
 */
function Lamps({
  model,
  layout,
  effect,
  brightness,
  playing,
  geometry,
  haloGeometry,
  material,
  haloMaterial,
  rotateX = -Math.PI / 2,
  glow,
}: {
  model: StageModel;
  layout: ModelLayout;
  effect: LightingEffect;
  brightness: number;
  playing: boolean;
  geometry: PlaneGeometry | CircleGeometry | BoxGeometry;
  haloGeometry?: PlaneGeometry | CircleGeometry;
  material: MeshBasicMaterial;
  haloMaterial?: MeshBasicMaterial;
  rotateX?: number;
  glow: RefObject<PointLight | null>;
}) {
  const lamps = useRef<InstancedMesh>(null);
  const halos = useRef<InstancedMesh>(null);
  const invalidate = useThree((s) => s.invalidate);
  const start = useRef(0);
  const n = layout.lamps.length;

  useLayoutEffect(() => {
    const o = new Object3D();
    const place = (mesh: InstancedMesh | null, grow: number, dy: number) => {
      if (!mesh) return;
      layout.lamps.forEach((l, i) => {
        o.position.set(l.x, l.y + dy, l.z);
        o.rotation.set(rotateX, 0, 0);
        o.scale.set(l.w * grow, l.d * grow, 1);
        o.updateMatrix();
        mesh.setMatrixAt(i, o.matrix);
      });
      mesh.instanceMatrix.needsUpdate = true;
      mesh.computeBoundingSphere();
    };
    place(lamps.current, 1, 0);
    place(halos.current, 1.9, rotateX === 0 ? -0.004 : -0.002);
  }, [layout, rotateX]);

  useEffect(() => {
    start.current = performance.now();
    invalidate();
  }, [effect, brightness, playing, model, invalidate]);

  useFrame(() => {
    const t = playing ? performance.now() - start.current : stillTime(effect);
    const colors = lampColors(model, layout, effect, brightness, t);
    for (const mesh of [lamps.current, halos.current]) {
      if (!mesh) continue;
      colors.forEach((c, i) => mesh.setColorAt(i, tmp.setRGB(c.r / 255, c.g / 255, c.b / 255, SRGBColorSpace)));
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    }
    const light = glow.current;
    if (light) {
      const avg = averageColor(colors);
      light.color.setRGB(avg.r / 255, avg.g / 255, avg.b / 255, SRGBColorSpace);
      light.intensity = ((avg.r + avg.g + avg.b) / 765) * 6;
    }
  });

  return (
    <>
      {haloGeometry && haloMaterial && <instancedMesh ref={halos} args={[haloGeometry, haloMaterial, n]} />}
      <instancedMesh ref={lamps} args={[geometry, material, n]} />
    </>
  );
}

function useGeometry<T extends { dispose(): void }>(make: () => T, deps: readonly unknown[]): T {
  const g = useMemo(make, deps);
  useEffect(() => () => g.dispose(), [g]);
  return g;
}

function Keycaps({ layout, height, material }: { layout: ModelLayout; height: number; material: MeshPhysicalMaterial }) {
  const ref = useRef<InstancedMesh>(null);
  const geometry = useGeometry(() => new RoundedBoxGeometry(1, 1, 1, 2, 0.12), []);
  useLayoutEffect(() => {
    const mesh = ref.current;
    if (!mesh) return;
    const o = new Object3D();
    layout.lamps.forEach((l, i) => {
      o.position.set(l.x, l.y + height / 2, l.z);
      o.scale.set(l.w * 0.92, height, l.d * 0.92);
      o.updateMatrix();
      mesh.setMatrixAt(i, o.matrix);
    });
    mesh.instanceMatrix.needsUpdate = true;
    mesh.computeBoundingSphere();
  }, [layout, height]);
  return <instancedMesh ref={ref} args={[geometry, material, layout.lamps.length]} />;
}

/** RGB keyboard: dark aluminium case, a deck lit through the gaps, per-key light. */
function Keyboard(p: LightingSceneProps & { glow: RefObject<PointLight | null>; mats: ReturnType<typeof useStageMaterials> }) {
  const layout = useMemo(() => modelLayout("keyboard"), []);
  const plane = useGeometry(() => new PlaneGeometry(1, 1), []);
  const legend = useGeometry(() => new PlaneGeometry(0.28, 0.12), []);
  return (
    <group position={[0, -0.12, 0.05]}>
      <RoundedBox args={[layout.width, 0.18, layout.depth]} radius={0.05} smoothness={5} material={p.mats.body} position={[0, 0.09, 0]} />
      <mesh material={p.mats.deck} position={[0, 0.1805, 0]} rotation={[-Math.PI / 2, 0, 0]}>
        <planeGeometry args={[layout.width - 0.2, layout.depth - 0.2]} />
      </mesh>
      <Lamps {...p} layout={layout} geometry={plane} haloGeometry={plane} material={p.mats.lamp} haloMaterial={p.mats.halo} />
      <Keycaps layout={layout} height={0.09} material={p.mats.cap} />
      {/* Key legends: thin lit marks on top of the caps. */}
      <group position={[0, 0.0915, 0]}>
        <Lamps {...p} layout={layout} geometry={legend} material={p.mats.lamp} />
      </group>
    </group>
  );
}

/** MacBook-style deck: aluminium, dark caps, white backlight through the caps. */
function Backlight(p: LightingSceneProps & { glow: RefObject<PointLight | null>; mats: ReturnType<typeof useStageMaterials> }) {
  const layout = useMemo(() => modelLayout("backlight"), []);
  const plane = useGeometry(() => new PlaneGeometry(1, 1), []);
  const legend = useGeometry(() => new PlaneGeometry(0.24, 0.1), []);
  const caps = useMemo(() => ({ ...layout, lamps: layout.lamps.map((l) => ({ ...l, y: 0.157 })) }), [layout]);
  const legends = useMemo(() => ({ ...layout, lamps: layout.lamps.map((l) => ({ ...l, y: 0.172 })) }), [layout]);
  return (
    <group position={[0, -0.08, 0.25]}>
      <RoundedBox args={[3.04, 0.155, 2.15]} radius={0.045} smoothness={6} material={p.mats.aluminium} position={[0, 0.0775, 0]} />
      <mesh material={p.mats.deck} position={[0, 0.1562, -0.415]} rotation={[-Math.PI / 2, 0, 0]}>
        <planeGeometry args={[2.72, 1.04]} />
      </mesh>
      <Lamps {...p} model="backlight" layout={layout} geometry={plane} haloGeometry={plane} material={p.mats.lamp} haloMaterial={p.mats.halo} />
      <Keycaps layout={caps} height={0.014} material={p.mats.cap} />
      <group position={[0, 0.0012, 0]}>
        <Lamps {...p} model="backlight" layout={legends} geometry={legend} material={p.mats.lamp} />
      </group>
      <mesh material={p.mats.aluminium} position={[0, 0.1561, 0.62]} rotation={[-Math.PI / 2, 0, 0]}>
        <planeGeometry args={[1.22, 0.74]} />
      </mesh>
    </group>
  );
}

/** Gaming mouse: glossy shell, a light strip around the rim and a lit logo. */
function Mouse(p: LightingSceneProps & { glow: RefObject<PointLight | null>; mats: ReturnType<typeof useStageMaterials> }) {
  const layout = useMemo(() => modelLayout("mouse"), []);
  const dot = useGeometry(() => new CircleGeometry(0.5, 20), []);
  return (
    <group position={[0, -0.2, 0]} scale={1.25}>
      <mesh material={p.mats.shell} position={[0, 0.25, 0]} scale={[0.62, 0.32, 1.02]}>
        <sphereGeometry args={[1, 48, 32]} />
      </mesh>
      <mesh material={p.mats.deck} position={[0, 0.03, 0]} scale={[0.64, 0.06, 1.04]}>
        <cylinderGeometry args={[1, 1, 1, 48]} />
      </mesh>
      <mesh material={p.mats.pcb} position={[0, 0.56, -0.5]} rotation={[-0.25, 0, 0]}>
        <boxGeometry args={[0.02, 0.04, 0.4]} />
      </mesh>
      <Lamps {...p} layout={layout} geometry={dot} haloGeometry={dot} material={p.mats.lamp} haloMaterial={p.mats.halo} rotateX={-Math.PI / 2} />
    </group>
  );
}

/** 120 mm RGB fan facing the viewer: frame, translucent blades, a ring of LEDs. */
function Fan(p: LightingSceneProps & { glow: RefObject<PointLight | null>; mats: ReturnType<typeof useStageMaterials>; reducedMotion: boolean }) {
  const layout = useMemo(() => modelLayout("fan"), []);
  const dot = useGeometry(() => new CircleGeometry(0.5, 16), []);
  const rotor = useRef<Group>(null);
  const frame = useGeometry(() => {
    const s = new Shape();
    const h = 1.2;
    const r = 0.12;
    s.moveTo(-h + r, -h);
    s.lineTo(h - r, -h);
    s.quadraticCurveTo(h, -h, h, -h + r);
    s.lineTo(h, h - r);
    s.quadraticCurveTo(h, h, h - r, h);
    s.lineTo(-h + r, h);
    s.quadraticCurveTo(-h, h, -h, h - r);
    s.lineTo(-h, -h + r);
    s.quadraticCurveTo(-h, -h, -h + r, -h);
    const hole = new Shape();
    hole.absarc(0, 0, 1.1, 0, Math.PI * 2, true);
    s.holes.push(hole);
    const g = new ExtrudeGeometry(s, { depth: 0.3, bevelEnabled: true, bevelThickness: 0.01, bevelSize: 0.01, bevelSegments: 2, curveSegments: 48 });
    g.translate(0, 0, -0.15);
    return g;
  }, []);
  const blade = useGeometry(() => {
    const s = new Shape();
    s.moveTo(0.22, -0.06);
    s.quadraticCurveTo(0.7, -0.32, 1.04, -0.08);
    s.quadraticCurveTo(0.8, 0.22, 0.22, 0.1);
    s.lineTo(0.22, -0.06);
    return new ExtrudeGeometry(s, { depth: 0.015, bevelEnabled: false, curveSegments: 12 });
  }, []);
  useFrame((_, dt) => {
    if (rotor.current && p.playing && !p.reducedMotion) rotor.current.rotation.z -= dt * 5;
  });
  return (
    <group rotation={[0, 0, 0]} position={[0, 0.1, 0]}>
      <mesh geometry={frame} material={p.mats.body} />
      <mesh material={p.mats.deck} position={[0, 0, -0.12]}>
        <circleGeometry args={[1.1, 48]} />
      </mesh>
      <group ref={rotor}>
        {Array.from({ length: 7 }, (_, i) => (
          <mesh key={i} geometry={blade} material={p.mats.blade} rotation={[0.25, 0, (i / 7) * Math.PI * 2]} />
        ))}
        <mesh material={p.mats.shell} rotation={[Math.PI / 2, 0, 0]}>
          <cylinderGeometry args={[0.24, 0.24, 0.16, 40]} />
        </mesh>
      </group>
      <Lamps {...p} layout={layout} geometry={dot} haloGeometry={dot} material={p.mats.lamp} haloMaterial={p.mats.halo} rotateX={0} />
    </group>
  );
}

/** Addressable LED strip on a flexible PCB with a frosted diffuser. */
function Strip(p: LightingSceneProps & { glow: RefObject<PointLight | null>; mats: ReturnType<typeof useStageMaterials> }) {
  const layout = useMemo(() => modelLayout("strip"), []);
  const chip = useGeometry(() => new PlaneGeometry(1, 1), []);
  const halo = useGeometry(() => new CircleGeometry(0.9, 20), []);
  return (
    <group position={[0, -0.05, 0.2]} rotation={[0, 0, 0]}>
      <RoundedBox args={[3.8, 0.04, 0.24]} radius={0.015} smoothness={3} material={p.mats.pcb} position={[0, 0.02, 0]} />
      <Lamps {...p} layout={layout} geometry={chip} haloGeometry={halo} material={p.mats.lamp} haloMaterial={p.mats.halo} />
    </group>
  );
}

/**
 * The lighting stage scene: one procedural device lit by the effect being edited. A colored
 * point light under the model, driven by the average lamp color, spills the light onto the
 * body and the floor of the studio.
 */
export default function LightingScene(props: LightingSceneProps & StageSceneProps) {
  const mats = useStageMaterials(props.theme);
  const glow = useRef<PointLight>(null);
  const shared = { ...props, mats, glow };
  return (
    <group>
      <pointLight ref={glow} position={[0, props.model === "fan" ? 0 : -0.3, props.model === "fan" ? 0.8 : 0.2]} intensity={0} distance={5} decay={1.6} />
      {props.model === "keyboard" && <Keyboard {...shared} />}
      {props.model === "backlight" && <Backlight {...shared} />}
      {props.model === "mouse" && <Mouse {...shared} />}
      {props.model === "fan" && <Fan {...shared} />}
      {props.model === "strip" && <Strip {...shared} />}
    </group>
  );
}
