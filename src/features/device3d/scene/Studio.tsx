import { useThree } from "@react-three/fiber";
import { useLayoutEffect } from "react";
import { Color, DoubleSide, Mesh, MeshBasicMaterial, PlaneGeometry, PMREMGenerator, Scene } from "three";
import type { DevicePreset } from "../presets";
import type { StageTheme } from "../types";
import { useDisposable } from "./parts";
import { shadowTexture } from "./textures";

/** Emissive cards of the reflection studio: position, size, brightness, tint. */
const CARDS: readonly { position: [number, number, number]; scale: [number, number]; intensity: number; color?: number }[] = [
  // Large overhead softbox.
  { position: [0, 7, 1], scale: [9, 6], intensity: 2.6 },
  // Tall strips: the long highlights along metal edges.
  { position: [-7, 2, 1.5], scale: [1.6, 9], intensity: 3.2 },
  { position: [7, 2.5, -1], scale: [2.4, 8], intensity: 1.7 },
  // Low back rim and a dim front card (the reflection on the laptop screen).
  { position: [0, 2.5, -9], scale: [12, 1.6], intensity: 1.4, color: 0xdbe6ff },
  { position: [0, 1.2, 9], scale: [8, 2.5], intensity: 0.7 },
];

/**
 * Reflection environment built locally: the cards above, in a dark (or, for the light theme,
 * mid-grey) room, prefiltered once with PMREM. Nothing is downloaded (no HDR file, no drei
 * preset, no loaders in the bundle); it is rebuilt only when the theme changes.
 */
function StudioEnvironment({ theme }: { theme: StageTheme }) {
  const gl = useThree((s) => s.gl);
  const scene = useThree((s) => s.scene);
  const invalidate = useThree((s) => s.invalidate);
  useLayoutEffect(() => {
    const dark = theme === "dark";
    const room = new Scene();
    room.background = new Color(dark ? 0x0d0e10 : 0x5a5e66);
    const plane = new PlaneGeometry(1, 1);
    const materials: MeshBasicMaterial[] = [];
    for (const card of CARDS) {
      const material = new MeshBasicMaterial({ color: new Color(card.color ?? 0xffffff).multiplyScalar(card.intensity), side: DoubleSide, toneMapped: false });
      materials.push(material);
      const mesh = new Mesh(plane, material);
      mesh.position.set(...card.position);
      mesh.scale.set(card.scale[0], card.scale[1], 1);
      mesh.lookAt(0, 0, 0);
      room.add(mesh);
    }
    const pmrem = new PMREMGenerator(gl);
    const target = pmrem.fromScene(room, 0.02);
    pmrem.dispose();
    plane.dispose();
    for (const m of materials) m.dispose();
    scene.environment = target.texture;
    scene.environmentIntensity = dark ? 0.95 : 1.1;
    invalidate();
    return () => {
      if (scene.environment === target.texture) scene.environment = null;
      target.dispose();
    };
  }, [gl, scene, theme, invalidate]);
  return null;
}

/**
 * Studio lighting, entirely local: a key / fill / rim trio for shape, plus the reflection
 * environment above.
 */
export function Studio({ theme }: { theme: StageTheme }) {
  const dark = theme === "dark";
  return (
    <>
      <hemisphereLight args={[dark ? 0xdde4f0 : 0xf4f6fa, dark ? 0x0c0d10 : 0x8c9098, dark ? 0.35 : 0.55]} />
      <directionalLight position={[-5, 8, 6]} intensity={dark ? 2.1 : 2.3} color={0xfffaf4} />
      <directionalLight position={[6, 3, 4]} intensity={dark ? 0.45 : 0.7} color={0xf2f5fa} />
      <directionalLight position={[2, 5, -7]} intensity={dark ? 1.7 : 1.2} color={0xdfe8ff} />
      <StudioEnvironment theme={theme} />
    </>
  );
}

/**
 * Baked soft shadow under the model: a wide ambient occlusion and a tighter contact shadow,
 * both blurred footprints on the floor (no shadow maps, no per-frame render target). It sits
 * on the turntable, so it turns with the footprint.
 */
export function FloorShadow({ preset, theme }: { preset: DevicePreset; theme: StageTheme }) {
  const { width, depth, z } = preset.shadow;
  const aspect = width / depth;
  const soft = useDisposable(() => shadowTexture(aspect, 34, 0.5), [aspect]);
  const contact = useDisposable(() => shadowTexture(aspect, 7, 0.78), [aspect]);
  const dark = theme === "dark";
  const tint = dark ? 0x000000 : 0x1a2230;
  const softMat = useDisposable(() => new MeshBasicMaterial({ map: soft, transparent: true, depthWrite: false, toneMapped: false }), [soft]);
  const contactMat = useDisposable(() => new MeshBasicMaterial({ map: contact, transparent: true, depthWrite: false, toneMapped: false }), [contact]);
  useLayoutEffect(() => {
    softMat.color.set(tint);
    softMat.opacity = dark ? 0.62 : 0.32;
    contactMat.color.set(tint);
    contactMat.opacity = dark ? 0.7 : 0.42;
  }, [softMat, contactMat, tint, dark]);
  return (
    <group position={[0, 0.001, z]} rotation={[-Math.PI / 2, 0, 0]}>
      <mesh material={softMat} scale={[width / 0.5, depth / 0.5, 1]} renderOrder={-2}>
        <planeGeometry args={[1, 1]} />
      </mesh>
      <mesh material={contactMat} scale={[width / 0.78, depth / 0.78, 1]} position={[0, 0, 0.001]} renderOrder={-1}>
        <planeGeometry args={[1, 1]} />
      </mesh>
    </group>
  );
}
