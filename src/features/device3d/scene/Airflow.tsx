import { useFrame } from "@react-three/fiber";
import { useLayoutEffect, useMemo, useRef } from "react";
import { CapsuleGeometry, type InstancedMesh, MeshBasicMaterial, Object3D, Quaternion, Vector3 } from "three";
import { type AirflowMode, type AirStream, airStreams, particleAt, type StreamKind } from "../airflow";
import type { DeviceKind, StageTheme } from "../types";
import { useDisposable } from "./parts";

/** Cool air in, warm air out; heat leaving a fanless case is warm too. Brighter on dark. */
const COLORS: Record<StageTheme, Record<StreamKind, number>> = {
  dark: { intake: 0x5cc4ff, exhaust: 0xff9a4d, heat: 0xff8a3d },
  light: { intake: 0x1683e0, exhaust: 0xf0621f, heat: 0xf07a2a },
};

const UP = new Vector3(0, 1, 0);

/**
 * One stream as an instanced mesh of small capsules pointing along the flow. `time` is shared
 * by the parent so every stream stays in step; with reduced motion the particles hold still
 * where they are (the picture still shows which way the air goes).
 */
function Stream({ stream, index, material, time }: { stream: AirStream; index: number; material: MeshBasicMaterial; time: { current: number } }) {
  const mesh = useRef<InstancedMesh>(null);
  const geometry = useDisposable(() => new CapsuleGeometry(stream.size, stream.size * 3.2, 3, 8), [stream.size]);
  const turn = useMemo(() => {
    const dir = new Vector3(...stream.travel).normalize();
    return new Quaternion().setFromUnitVectors(UP, dir.lengthSq() > 0 ? dir : UP);
  }, [stream.travel]);
  const dummy = useMemo(() => new Object3D(), []);

  const place = (t: number) => {
    const m = mesh.current;
    if (!m) return;
    for (let i = 0; i < stream.count; i++) {
      const p = particleAt(stream, index, i, t);
      dummy.position.set(...p.position);
      dummy.quaternion.copy(turn);
      dummy.scale.setScalar(Math.max(p.scale, 1e-4));
      dummy.updateMatrix();
      m.setMatrixAt(i, dummy.matrix);
    }
    m.instanceMatrix.needsUpdate = true;
  };

  useLayoutEffect(() => place(time.current));
  useFrame(() => place(time.current));

  return <instancedMesh ref={mesh} args={[geometry, material, stream.count]} frustumCulled={false} renderOrder={2} />;
}

/**
 * The air around the device (`airflow.ts`): cool intake in blue, warm exhaust in orange, or
 * for a fanless computer the heat rising off the case. Lives inside the turntable, so the
 * streams turn with the model. `moving` false freezes it (reduced motion, stage paused).
 */
export function Airflow({ kind, mode, theme, moving }: { kind: DeviceKind; mode: AirflowMode; theme: StageTheme; moving: boolean }) {
  const streams = airStreams(kind, mode);
  const materials = useDisposable(() => {
    const make = (k: StreamKind) =>
      new MeshBasicMaterial({ color: COLORS[theme][k], transparent: true, opacity: k === "heat" ? 0.55 : 0.8, depthWrite: false, toneMapped: false });
    const m = { intake: make("intake"), exhaust: make("exhaust"), heat: make("heat") };
    return { ...m, dispose: () => Object.values(m).forEach((x) => x.dispose()) };
  }, [theme]);
  const time = useRef(0);
  useFrame((_, delta) => {
    if (moving) time.current += Math.min(delta, 0.1);
  });
  return (
    <group>
      {streams.map((s, i) => (
        <Stream key={`${kind}-${mode}-${i}`} stream={s} index={i} material={materials[s.kind]} time={time} />
      ))}
    </group>
  );
}
