import { useEffect, useLayoutEffect, useMemo, useRef, type ReactNode } from "react";
import { BufferGeometry, ExtrudeGeometry, Object3D, Shape, type InstancedMesh, type Material } from "three";
import { useMaterials } from "./materials";

type V3 = [number, number, number];

/** A geometry (or other disposable) owned by a component: built once per deps, freed after. */
export function useDisposable<T extends { dispose(): void }>(factory: () => T, deps: readonly unknown[]): T {
  const value = useMemo(factory, deps);
  useEffect(() => () => value.dispose(), [value]);
  return value;
}

/** Rounded rectangle centred on the origin, in the xy plane. */
export function roundedRect(w: number, h: number, r: number): Shape {
  const rr = Math.min(r, w / 2 - 1e-4, h / 2 - 1e-4);
  const x = -w / 2;
  const y = -h / 2;
  const s = new Shape();
  s.moveTo(x + rr, y);
  s.lineTo(x + w - rr, y);
  s.quadraticCurveTo(x + w, y, x + w, y + rr);
  s.lineTo(x + w, y + h - rr);
  s.quadraticCurveTo(x + w, y + h, x + w - rr, y + h);
  s.lineTo(x + rr, y + h);
  s.quadraticCurveTo(x, y + h, x, y + h - rr);
  s.lineTo(x, y + rr);
  s.quadraticCurveTo(x, y, x + rr, y);
  return s;
}

/** A thin plate with rounded corners, centred, facing +z (rotate it to lie flat). */
export function plateGeometry(w: number, h: number, r: number, thickness: number): ExtrudeGeometry {
  const g = new ExtrudeGeometry(roundedRect(w, h, r), { depth: thickness, bevelEnabled: false, curveSegments: 8 });
  g.translate(0, 0, -thickness / 2);
  return g;
}

interface PlateProps {
  size: [number, number];
  radius: number;
  thickness: number;
  material: Material;
  position?: V3;
  rotation?: V3;
  /** Lie flat in the xz plane, top face up. */
  flat?: boolean;
}

export function Plate({ size, radius, thickness, material, position = [0, 0, 0], rotation, flat = false }: PlateProps) {
  const geometry = useDisposable(() => plateGeometry(size[0], size[1], radius, thickness), [size[0], size[1], radius, thickness]);
  return <mesh geometry={geometry} material={material} position={position} rotation={rotation ?? (flat ? [-Math.PI / 2, 0, 0] : [0, 0, 0])} />;
}

/** A flat rounded-rect decal (no thickness), facing +z. */
function Decal({ w, h, r, material, z = 0, x = 0, y = 0 }: { w: number; h: number; r: number; material: Material; z?: number; x?: number; y?: number }) {
  const shape = useMemo(() => roundedRect(w, h, r), [w, h, r]);
  return (
    <mesh material={material} position={[x, y, z]}>
      <shapeGeometry args={[shape, 6]} />
    </mesh>
  );
}

export type PortKind = "usbA" | "usbC" | "audio" | "hdmi" | "ethernet" | "dc";

/**
 * A generic port opening on a surface facing +z (place it 0.5 mm in front of the surface).
 * Sizes are real (1 unit = 10 cm): USB-A 12.5 × 4.5 mm, USB-C 8.4 × 2.6 mm...
 */
export function Port({ kind, position, vertical = false }: { kind: PortKind; position: V3; vertical?: boolean }) {
  const m = useMaterials();
  const rotation: V3 = [0, 0, vertical ? Math.PI / 2 : 0];
  let body: ReactNode;
  switch (kind) {
    case "usbA":
      body = (
        <>
          <Decal w={0.125} h={0.048} r={0.004} material={m.hole} />
          <Decal w={0.104} h={0.016} r={0.002} material={m.tongue} y={0.008} z={0.0006} />
        </>
      );
      break;
    case "usbC":
      body = (
        <>
          <Decal w={0.088} h={0.032} r={0.016} material={m.hole} />
          <Decal w={0.064} h={0.008} r={0.004} material={m.tongue} z={0.0006} />
        </>
      );
      break;
    case "hdmi":
      body = (
        <>
          <Decal w={0.15} h={0.052} r={0.006} material={m.hole} />
          <Decal w={0.12} h={0.014} r={0.002} material={m.tongue} y={0.006} z={0.0006} />
        </>
      );
      break;
    case "ethernet":
      body = (
        <>
          <Decal w={0.16} h={0.135} r={0.006} material={m.hole} />
          <Decal w={0.12} h={0.03} r={0.003} material={m.tongue} y={-0.035} z={0.0006} />
        </>
      );
      break;
    case "audio":
    case "dc":
      body = (
        <>
          <mesh material={m.tongue}>
            <circleGeometry args={[kind === "audio" ? 0.03 : 0.045, 24]} />
          </mesh>
          <mesh material={m.hole} position={[0, 0, 0.0006]}>
            <circleGeometry args={[kind === "audio" ? 0.018 : 0.03, 24]} />
          </mesh>
        </>
      );
      break;
  }
  return (
    <group position={position} rotation={rotation}>
      {body}
    </group>
  );
}

export interface InstanceTransform {
  position: V3;
  /** Euler angles applied x first, then y, then z (order "ZYX"). */
  rotation?: V3;
  scale?: V3;
}

/** Many copies of one geometry in a single draw call. */
export function Instanced({ geometry, material, items }: { geometry: BufferGeometry; material: Material; items: readonly InstanceTransform[] }) {
  const ref = useRef<InstancedMesh>(null);
  useLayoutEffect(() => {
    const mesh = ref.current;
    if (!mesh) return;
    const o = new Object3D();
    items.forEach((it, i) => {
      o.position.set(...it.position);
      const [rx, ry, rz] = it.rotation ?? [0, 0, 0];
      o.rotation.set(rx, ry, rz, "ZYX");
      o.scale.set(...(it.scale ?? [1, 1, 1]));
      o.updateMatrix();
      mesh.setMatrixAt(i, o.matrix);
    });
    mesh.instanceMatrix.needsUpdate = true;
    mesh.computeBoundingSphere();
  }, [items]);
  return <instancedMesh ref={ref} args={[geometry, material, items.length]} />;
}

/**
 * A case fan facing +z: square frame with rounded corners, a dark throat, seven swept blades
 * and a hub. `size` is the frame's side (12 cm fan = 1.2).
 */
export function Fan({ size, position, rotation }: { size: number; position: V3; rotation?: V3 }) {
  const m = useMaterials();
  const depth = size * 0.2;
  const frame = useDisposable(() => {
    const outer = roundedRect(size, size, size * 0.08);
    const hole = new Shape();
    hole.absarc(0, 0, size * 0.46, 0, Math.PI * 2, true);
    outer.holes.push(hole);
    const g = new ExtrudeGeometry(outer, { depth, bevelEnabled: true, bevelThickness: 0.006, bevelSize: 0.006, bevelSegments: 2, curveSegments: 32 });
    g.translate(0, 0, -depth / 2);
    return g;
  }, [size, depth]);
  const blade = useDisposable(() => {
    const s = new Shape();
    const r0 = size * 0.12;
    const r1 = size * 0.44;
    s.moveTo(r0, -size * 0.04);
    s.quadraticCurveTo((r0 + r1) / 2, -size * 0.1, r1, -size * 0.02);
    s.quadraticCurveTo(r1 * 0.98, size * 0.14, (r0 + r1) / 2, size * 0.13);
    s.quadraticCurveTo(r0 * 1.1, size * 0.08, r0, size * 0.04);
    s.closePath();
    return new ExtrudeGeometry(s, { depth: 0.006, bevelEnabled: false, curveSegments: 8 });
  }, [size]);
  const blades = useMemo<InstanceTransform[]>(
    () => Array.from({ length: 7 }, (_, i) => ({ position: [0, 0, -0.003] as V3, rotation: [0.32, 0, (i / 7) * Math.PI * 2] as V3 })),
    [],
  );
  return (
    <group position={position} rotation={rotation ?? [0, 0, 0]}>
      <mesh geometry={frame} material={m.interior} />
      <mesh material={m.hole} position={[0, 0, -depth / 2 + 0.002]}>
        <circleGeometry args={[size * 0.46, 40]} />
      </mesh>
      <Instanced geometry={blade} material={m.graphiteDeep} items={blades} />
      <mesh material={m.graphite} rotation={[Math.PI / 2, 0, 0]}>
        <cylinderGeometry args={[size * 0.12, size * 0.12, depth * 0.6, 32]} />
      </mesh>
    </group>
  );
}
