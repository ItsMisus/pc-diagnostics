import { Canvas, useThree } from "@react-three/fiber";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ACESFilmicToneMapping, SRGBColorSpace } from "three";
import { initialDpr } from "../quality";
import type { AirflowMode } from "../airflow";
import type { DeviceKind, StageTheme } from "../types";
import { createMaterials, MaterialsContext } from "./materials";
import { Rig } from "./Rig";
import { Studio } from "./Studio";

export interface DeviceSceneProps {
  kind: DeviceKind;
  theme: StageTheme;
  /** False: render nothing at all (frameloop "never"). */
  active: boolean;
  ambient: boolean;
  reducedMotion: boolean;
  extraPadX: number;
  spin: boolean;
  airflow: AirflowMode | null;
  /** The focusable stage element that receives pointer and keyboard input. */
  eventTarget: HTMLElement | null;
  /** The first frame is on screen. */
  onReady: () => void;
  /** The WebGL context was lost: the stage switches to the illustration. */
  onContextLost: () => void;
}

function SceneContent(props: Omit<DeviceSceneProps, "onContextLost">) {
  const materials = useMemo(() => createMaterials(props.theme), []);
  useEffect(() => () => materials.dispose(), [materials]);
  const firstTheme = useRef(props.theme);
  useEffect(() => {
    if (props.theme === firstTheme.current) return;
    firstTheme.current = props.theme;
    materials.setTheme(props.theme);
  }, [materials, props.theme]);
  return (
    <MaterialsContext.Provider value={materials}>
      <Studio theme={props.theme} />
      <Rig {...props} />
    </MaterialsContext.Provider>
  );
}

/**
 * The WebGL part of the stage (lazy chunk: three.js, fiber, drei). Transparent canvas over the
 * stage surface, on-demand rendering, DPR within [1, 1.75], low-power GPU preference (it must
 * not compete with a benchmark), ACES tone mapping, sRGB output.
 */
export default function DeviceScene({ onContextLost, ...props }: DeviceSceneProps) {
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
  const exposure = props.theme === "dark" ? 1 : 1.06;

  return (
    <Canvas
      aria-hidden="true"
      frameloop={props.active ? "demand" : "never"}
      dpr={dpr}
      gl={{ antialias: true, alpha: true, powerPreference: "low-power", stencil: false, preserveDrawingBuffer: false }}
      camera={{ fov: 26, near: 1, far: 100, position: [0, 2, 16] }}
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
      <Exposure value={exposure} />
      <SceneContent {...props} />
    </Canvas>
  );
}

/** Keeps the exposure in step with the theme after creation. */
function Exposure({ value }: { value: number }) {
  const gl = useThree((s) => s.gl);
  const invalidate = useThree((s) => s.invalidate);
  useEffect(() => {
    gl.toneMappingExposure = value;
    invalidate();
  }, [gl, value, invalidate]);
  return null;
}
