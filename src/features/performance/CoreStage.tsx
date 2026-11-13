import { useMemo, type ReactNode } from "react";
import { ObjectStage } from "@/components/stage3d/ObjectStage";
import type { PerformanceMode } from "@/types/generated/PerformanceMode";
import { coreLook } from "./model";
import type { CoreSceneProps } from "./scene/CoreScene";

/** Module scope: a stable loader keeps three.js out of the page chunk. */
const loadCoreScene = () => import("./scene/CoreScene");

/** Flat drawing of the core for stages without WebGL (and while the 3D chunk loads). */
export function CoreFallback({ mode }: { mode: PerformanceMode | null }) {
  const look = coreLook(mode);
  const pads = Array.from({ length: 11 }, (_, i) => 26 + i * 6.8);
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 120 120" preserveAspectRatio="xMidYMid meet" className="absolute inset-0 h-full w-full text-fg">
      <rect x="14" y="14" width="92" height="92" rx="8" fill="currentColor" fillOpacity="0.1" stroke="currentColor" strokeOpacity="0.2" />
      {pads.map((p) => (
        <g key={p} fill="currentColor" fillOpacity="0.28">
          <rect x={p - 1} y="16.5" width="2" height="5" rx="0.6" />
          <rect x={p - 1} y="98.5" width="2" height="5" rx="0.6" />
          <rect x="16.5" y={p - 1} width="5" height="2" rx="0.6" />
          <rect x="98.5" y={p - 1} width="5" height="2" rx="0.6" />
        </g>
      ))}
      <rect x="27" y="27" width="66" height="66" rx="5" fill="none" stroke="currentColor" strokeOpacity="0.22" strokeDasharray="3 2.4" />
      <rect x="37" y="37" width="46" height="46" rx="5" fill="currentColor" fillOpacity="0.16" />
      <g className="text-accent-text">
        <rect x="43" y="43" width="34" height="34" rx="3.5" fill="currentColor" fillOpacity={0.15 + 0.55 * look.glow} />
      </g>
    </svg>
  );
}

/**
 * The page's focal element: the power core in 3D, lit by the mode in effect. It animates
 * continuously (the energy pulses) only while visible and without reduced motion; with
 * reduced motion it is a still image that changes at once.
 */
export function CoreStage({
  mode,
  label,
  paused,
  children,
}: {
  mode: PerformanceMode | null;
  label: string;
  paused: boolean;
  children?: ReactNode;
}) {
  const look = coreLook(mode);
  // A new object only when the mode changes, so the scene is not re-rendered per page render.
  const sceneProps = useMemo<CoreSceneProps>(() => ({ look }), [look]);
  return (
    <ObjectStage
      label={label}
      loadScene={loadCoreScene}
      sceneProps={sceneProps}
      animate={mode !== null}
      autoRotate
      paused={paused}
      camera={{ distance: 8.2, height: 2.4, fov: 30 }}
      fallback={<CoreFallback mode={mode} />}
      aspectRatio={null}
      className="absolute! inset-3 rounded-xl"
    >
      {children}
    </ObjectStage>
  );
}
