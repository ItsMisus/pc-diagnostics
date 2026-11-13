import { OrbitControls } from "@react-three/drei";
import { Canvas, useThree } from "@react-three/fiber";
import { useEffect, useLayoutEffect, useRef, useState, type ComponentType } from "react";
import { ACESFilmicToneMapping, SRGBColorSpace } from "three";
import { initialDpr } from "@/features/device3d/quality";
import { Studio } from "@/features/device3d/scene/Studio";
import type { StageTheme } from "@/features/device3d/types";
import type { StageSceneProps } from "./ObjectStage";

export interface ObjectCanvasProps<P extends object> {
  Scene: ComponentType<P & StageSceneProps>;
  sceneProps: P;
  theme: StageTheme;
  reducedMotion: boolean;
  active: boolean;
  continuous: boolean;
  orbit: boolean;
  /** Turntable spin (OrbitControls' auto-rotate); needs `continuous`. */
  autoRotate?: boolean;
  camera?: { distance?: number; height?: number; fov?: number } | undefined;
  onReady: () => void;
  onContextLost: () => void;
}

/** OrbitControls units: 2 = one turn per 30 s at 60 fps; 2.5 ≈ one turn in 24 s. */
const AUTO_ROTATE_SPEED = 2.5;

/** Reports the first rendered frame. */
function FirstFrame({ onReady }: { onReady: () => void }) {
  const gl = useThree((s) => s.gl);
  const done = useRef(false);
  useEffect(() => {
    if (done.current) return;
    const id = requestAnimationFrame(() => {
      done.current = true;
      onReady();
    });
    return () => cancelAnimationFrame(id);
  }, [gl, onReady]);
  return null;
}

/**
 * WebGL part of `ObjectStage` (lazy chunk). Same renderer setup as the Overview device: DPR
 * within [1, 1.75], low-power GPU preference, ACES tone mapping, sRGB, transparent clear,
 * the local studio environment. `continuous` switches the frame loop from on-demand to every
 * frame; inactive stages render nothing.
 */
export default function ObjectCanvas<P extends object>({ Scene, sceneProps, theme, reducedMotion, active, continuous, orbit, autoRotate = false, camera, onReady, onContextLost }: ObjectCanvasProps<P>) {
  const [dpr] = useState(() => initialDpr(typeof window !== "undefined" ? window.devicePixelRatio : 1));
  const mounted = useRef(true);
  useLayoutEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const lostRef = useRef(onContextLost);
  lostRef.current = onContextLost;
  const distance = camera?.distance ?? 9;
  const height = camera?.height ?? 2.2;
  const exposure = theme === "dark" ? 1 : 1.06;

  return (
    <Canvas
      aria-hidden="true"
      frameloop={active ? (continuous ? "always" : "demand") : "never"}
      dpr={dpr}
      gl={{ antialias: true, alpha: true, powerPreference: "low-power", stencil: false, preserveDrawingBuffer: false }}
      camera={{ fov: camera?.fov ?? 30, near: 0.5, far: 100, position: [0, height, distance] }}
      resize={{ scroll: false, debounce: { scroll: 0, resize: 60 } }}
      style={{ position: "absolute", inset: 0 }}
      onCreated={({ gl }) => {
        gl.toneMapping = ACESFilmicToneMapping;
        gl.toneMappingExposure = exposure;
        gl.outputColorSpace = SRGBColorSpace;
        gl.setClearColor(0x000000, 0);
        gl.domElement.addEventListener("webglcontextlost", (e) => {
          e.preventDefault();
          if (mounted.current) lostRef.current();
        });
      }}
    >
      <Studio theme={theme} />
      <Scene {...sceneProps} theme={theme} reducedMotion={reducedMotion} />
      {orbit && (
        <OrbitControls
          makeDefault
          enablePan={false}
          enableZoom={false}
          enableDamping={!reducedMotion}
          dampingFactor={0.12}
          rotateSpeed={0.6}
          minPolarAngle={Math.PI * 0.18}
          maxPolarAngle={Math.PI * 0.55}
          autoRotate={autoRotate}
          autoRotateSpeed={AUTO_ROTATE_SPEED}
        />
      )}
      <FirstFrame onReady={onReady} />
    </Canvas>
  );
}
