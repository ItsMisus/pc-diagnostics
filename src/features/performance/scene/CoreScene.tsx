import { useFrame, useThree } from "@react-three/fiber";
import { useEffect, useLayoutEffect, useMemo, useRef } from "react";
import {
  CanvasTexture,
  Color,
  type Group,
  type InstancedMesh,
  MeshBasicMaterial,
  MeshPhysicalMaterial,
  MeshStandardMaterial,
  NormalBlending,
  Object3D,
  type PointLight,
  SRGBColorSpace,
} from "three";
import type { StageSceneProps } from "@/components/stage3d/ObjectStage";
import { useDisposable, plateGeometry } from "@/features/device3d/scene/parts";
import { readColorToken } from "@/lib/theme";
import type { CoreLook } from "../model";

export interface CoreSceneProps {
  /** Target look of the core; the scene eases toward it (jumps with reduced motion). */
  look: CoreLook;
}

/** Warm end of the color temperature (amber white); the cool end is the theme's accent. */
const WARM = 0xffb36b;
/** Used when the accent token cannot be read (tests, very old WebViews). */
const COOL_FALLBACK = 0x6fb6ff;

const SUBSTRATE = 3.2;
const RINGS = [
  { half: 1.32, count: 52 },
  { half: 1.12, count: 44 },
] as const;
const PADS_PER_SIDE = 15;

/** Points evenly spaced along the perimeter of a square of half-size `half` (u in 0..1). */
function squarePoint(half: number, u: number): { x: number; z: number; alongX: boolean } {
  const side = Math.floor(u * 4) % 4;
  const t = u * 4 - Math.floor(u * 4);
  const p = -half + 2 * half * t;
  switch (side) {
    case 0:
      return { x: p, z: half, alongX: true };
    case 1:
      return { x: half, z: -p, alongX: false };
    case 2:
      return { x: -p, z: -half, alongX: true };
    default:
      return { x: -half, z: p, alongX: false };
  }
}

/** Soft round glow: white centre fading to transparent (tinted by the material color). */
function haloTexture(): CanvasTexture {
  const size = 128;
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (ctx) {
    const g = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
    g.addColorStop(0, "rgba(255,255,255,1)");
    g.addColorStop(0.35, "rgba(255,255,255,0.45)");
    g.addColorStop(1, "rgba(255,255,255,0)");
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, size, size);
  }
  const t = new CanvasTexture(canvas);
  t.colorSpace = SRGBColorSpace;
  return t;
}

function useCoreMaterials() {
  const m = useMemo(() => {
    const halo = haloTexture();
    return {
      halo,
      substrate: new MeshStandardMaterial({ color: 0x1a1d22, metalness: 0.35, roughness: 0.62 }),
      frame: new MeshPhysicalMaterial({ color: 0x34373d, metalness: 0.9, roughness: 0.38, clearcoat: 0.3, clearcoatRoughness: 0.3 }),
      pads: new MeshStandardMaterial({ color: 0xc9a86a, metalness: 1, roughness: 0.3 }),
      core: new MeshPhysicalMaterial({
        color: 0x0b0c0e,
        emissive: 0xffffff,
        emissiveIntensity: 0.5,
        metalness: 0.2,
        roughness: 0.15,
        clearcoat: 1,
        clearcoatRoughness: 0.06,
      }),
      trace: new MeshBasicMaterial({ color: 0xffffff, toneMapped: false }),
      glow: new MeshBasicMaterial({ map: halo, transparent: true, depthWrite: false, blending: NormalBlending, toneMapped: false, opacity: 0.4 }),
    };
  }, []);
  useEffect(
    () => () => {
      for (const mat of [m.substrate, m.frame, m.pads, m.core, m.trace, m.glow]) mat.dispose();
      m.halo.dispose();
    },
    [m],
  );
  return m;
}

/**
 * A processor package seen from above at an angle: dark substrate with gold contacts, a
 * machined frame and a glass-like die that glows. Two rings of traces carry pulses of energy
 * around the die. The selected mode sets the glow, the pulse speed and the color temperature
 * (cool and calm for Silent, warm and vivid for High performance); changes ease in over about
 * a second. Reduced motion: no pulses, the look jumps.
 */
export default function CoreScene({ look, theme, reducedMotion }: CoreSceneProps & StageSceneProps) {
  const m = useCoreMaterials();
  const invalidate = useThree((s) => s.invalidate);
  const group = useRef<Group>(null);
  const light = useRef<PointLight>(null);
  const traces = useRef<InstancedMesh>(null);
  const pads = useRef<InstancedMesh>(null);
  const current = useRef<CoreLook>({ ...look });
  const phase = useRef(0);

  const cool = useMemo(() => {
    const c = readColorToken("--accent");
    return c ? new Color(`rgb(${c.r}, ${c.g}, ${c.b})`) : new Color(COOL_FALLBACK);
  }, [theme]);
  const warm = useMemo(() => new Color(WARM), []);
  const tint = useMemo(() => new Color(), []);
  const scratch = useMemo(() => new Color(), []);

  const substrateGeo = useDisposable(() => plateGeometry(SUBSTRATE, SUBSTRATE, 0.2, 0.12), []);
  const frameGeo = useDisposable(() => plateGeometry(1.7, 1.7, 0.14, 0.09), []);
  const coreGeo = useDisposable(() => plateGeometry(1.22, 1.22, 0.1, 0.06), []);

  const traceSlots = useMemo(
    () => RINGS.flatMap((ring, r) => Array.from({ length: ring.count }, (_, i) => ({ ring: r, u: i / ring.count, ...squarePoint(ring.half, i / ring.count) }))),
    [],
  );

  // Static transforms of the instanced traces and pads.
  useLayoutEffect(() => {
    const o = new Object3D();
    const t = traces.current;
    if (t) {
      traceSlots.forEach((s, i) => {
        o.position.set(s.x, 0.066, s.z);
        o.rotation.set(0, s.alongX ? 0 : Math.PI / 2, 0);
        o.scale.set(1, 1, 1);
        o.updateMatrix();
        t.setMatrixAt(i, o.matrix);
        t.setColorAt(i, scratch.setRGB(0.2, 0.2, 0.2));
      });
      t.instanceMatrix.needsUpdate = true;
      if (t.instanceColor) t.instanceColor.needsUpdate = true;
    }
    const p = pads.current;
    if (p) {
      const edge = SUBSTRATE / 2 - 0.11;
      let k = 0;
      for (let side = 0; side < 4; side++) {
        for (let i = 0; i < PADS_PER_SIDE; i++) {
          const along = -1.25 + (2.5 * i) / (PADS_PER_SIDE - 1);
          const x = side === 0 ? along : side === 1 ? edge : side === 2 ? -along : -edge;
          const z = side === 0 ? edge : side === 1 ? -along : side === 2 ? -edge : along;
          o.position.set(x, 0.064, z);
          o.rotation.set(0, side % 2 === 0 ? 0 : Math.PI / 2, 0);
          o.updateMatrix();
          p.setMatrixAt(k++, o.matrix);
        }
      }
      p.instanceMatrix.needsUpdate = true;
    }
    invalidate();
  }, [traceSlots, scratch, invalidate]);

  /** Writes the look into materials, the light and the trace colors. */
  const paint = (l: CoreLook, t: number) => {
    tint.copy(cool).lerp(warm, l.warmth);
    m.core.emissive.copy(tint);
    m.core.emissiveIntensity = 0.25 + 2.1 * l.glow;
    m.glow.color.copy(tint);
    m.glow.opacity = (theme === "dark" ? 0.18 : 0.12) + (theme === "dark" ? 0.55 : 0.4) * l.glow;
    if (light.current) {
      light.current.color.copy(tint);
      light.current.intensity = 0.4 + 5 * l.glow;
    }
    const mesh = traces.current;
    if (!mesh) return;
    traceSlots.forEach((s, i) => {
      // Three packets per ring; the inner ring runs the other way.
      const dir = s.ring === 0 ? 1 : -1;
      const wave = Math.cos(2 * Math.PI * (3 * s.u - dir * t));
      const pulse = Math.pow(Math.max(0, wave), 6);
      const b = 0.06 + 0.22 * l.glow + 1.4 * l.glow * pulse;
      mesh.setColorAt(i, scratch.copy(tint).multiplyScalar(b));
    });
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  };

  // Reduced motion (or a stage rendering on demand): jump to the target and render once.
  useEffect(() => {
    if (!reducedMotion) return;
    current.current = { ...look };
    paint(current.current, 0.12);
    invalidate();
    // `paint` reads refs and memoized objects only.
  }, [reducedMotion, look, theme, cool]);

  useFrame((_, delta) => {
    if (reducedMotion) return;
    const dt = Math.min(delta, 0.1);
    const k = 1 - Math.exp(-dt * 3.2);
    const c = current.current;
    c.glow += (look.glow - c.glow) * k;
    c.speed += (look.speed - c.speed) * k;
    c.warmth += (look.warmth - c.warmth) * k;
    phase.current = (phase.current + dt * c.speed * 0.45) % 1;
    paint(c, phase.current);
  });

  return (
    <group ref={group} rotation={[0.42, -0.5, 0]} position={[0, -0.15, 0]}>
      <mesh geometry={substrateGeo} material={m.substrate} rotation={[-Math.PI / 2, 0, 0]} />
      <instancedMesh ref={pads} args={[undefined, undefined, PADS_PER_SIDE * 4]} material={m.pads}>
        <boxGeometry args={[0.07, 0.012, 0.16]} />
      </instancedMesh>
      <instancedMesh ref={traces} args={[undefined, undefined, traceSlots.length]} material={m.trace}>
        <boxGeometry args={[0.11, 0.008, 0.028]} />
      </instancedMesh>
      <mesh geometry={frameGeo} material={m.frame} rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.1, 0]} />
      <mesh geometry={coreGeo} material={m.core} rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.17, 0]} />
      <mesh material={m.glow} rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.21, 0]} scale={[3.4, 3.4, 1]} renderOrder={2}>
        <planeGeometry args={[1, 1]} />
      </mesh>
      <pointLight ref={light} position={[0, 0.7, 0]} distance={4.5} decay={2} />
    </group>
  );
}
