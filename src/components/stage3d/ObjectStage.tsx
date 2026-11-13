import { Component, lazy, Suspense, useId, useMemo, useRef, useState, type ComponentType, type ErrorInfo, type ReactNode } from "react";
import { useDocumentHidden, useDocumentTheme, useInView, usePrefersReducedMotion } from "@/features/device3d/hooks";
import { isStageActive } from "@/features/device3d/motion";
import { webglAvailable } from "@/features/device3d/support";
import type { StageTheme } from "@/features/device3d/types";
import type { ObjectCanvasProps } from "./ObjectCanvas";

/** What every scene rendered in an `ObjectStage` receives, plus its own `sceneProps`. */
export interface StageSceneProps {
  theme: StageTheme;
  /** The OS "reduce motion" setting: no idle or continuous motion, state changes jump. */
  reducedMotion: boolean;
}

export type SceneModule<P> = { default: ComponentType<P & StageSceneProps> };

let canvasPromise: Promise<typeof import("./ObjectCanvas")> | undefined;
const loadCanvas = () => (canvasPromise ??= import("./ObjectCanvas"));
const ObjectCanvas = lazy(loadCanvas) as unknown as ComponentType<ObjectCanvasProps<object>>;

export interface ObjectStageProps<P extends object> {
  /** Accessible name of the picture, e.g. "Ventola da 120 mm che gira a 1.200 giri/min". */
  label: string;
  /** Longer description for screen readers (optional). */
  description?: string;
  /**
   * The scene module (lazy: three.js stays out of the page chunk until the stage shows).
   * Pass a stable function (module scope), e.g. `() => import("./scene/FanScene")`.
   */
  loadScene: () => Promise<SceneModule<P>>;
  /** Props forwarded to the scene, next to `theme` and `reducedMotion`. */
  sceneProps: P;
  /**
   * The scene animates continuously (spinning fans, light effects). Without it the canvas
   * renders only on change. Forced off with reduced motion, while hidden or off screen.
   */
  animate?: boolean;
  /** Stop rendering entirely (e.g. while a benchmark runs). */
  paused?: boolean;
  /** Drag to orbit around the object (default true), within a limited arc. */
  orbit?: boolean;
  /**
   * The object turns slowly on itself (a turntable, one turn in about 24 s). Drag still turns
   * it by hand. Off with reduced motion, and stops while hidden or off screen like the rest.
   */
  autoRotate?: boolean;
  /** Camera distance and height (scene units); the camera looks at the origin. */
  camera?: { distance?: number; height?: number; fov?: number };
  /** Shown without WebGL, after a context loss, or if the 3D bundle fails. */
  fallback: ReactNode;
  /** CSS aspect ratio (default "4 / 3"); null fills the parent's height. */
  aspectRatio?: string | null;
  className?: string;
  /** Overlays on top of the stage (labels, badges). Pointer events pass through the gaps. */
  children?: ReactNode;
}

class Boundary extends Component<{ onError: () => void; children: ReactNode }, { failed: boolean }> {
  override state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  override componentDidCatch(_error: unknown, _info: ErrorInfo) {
    this.props.onError();
  }
  override render() {
    return this.state.failed ? null : this.props.children;
  }
}

/**
 * A small 3D stage for one object (a fan, a heatsink, a keyboard, a game card), shared by the
 * control centers so their 3D looks and behaves like the Overview device: the same studio
 * lighting and reflections, a transparent canvas over the panel, on-demand rendering, a
 * limited orbit by drag, paused while hidden or off screen, no continuous motion with reduced
 * motion, and a static `fallback` without WebGL.
 */
export function ObjectStage<P extends object>({
  label,
  description,
  loadScene,
  sceneProps,
  animate = false,
  paused = false,
  orbit = true,
  autoRotate = false,
  camera,
  fallback,
  aspectRatio = "4 / 3",
  className = "",
  children,
}: ObjectStageProps<P>) {
  const [webgl] = useState(webglAvailable);
  const [failed, setFailed] = useState(false);
  const [ready, setReady] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const theme = useDocumentTheme();
  const reducedMotion = usePrefersReducedMotion();
  const hidden = useDocumentHidden();
  const inView = useInView(rootRef);
  const active = isStageActive({ paused, documentHidden: hidden, inView });
  const descId = useId();
  const Scene = useMemo(() => lazy(loadScene) as unknown as ComponentType<P & StageSceneProps>, [loadScene]);
  const show3d = webgl && !failed;

  return (
    <div ref={rootRef} className={`relative isolate ${className}`} style={aspectRatio ? { aspectRatio } : undefined} data-stage-mode={show3d ? (ready ? "3d" : "loading") : "fallback"}>
      <div
        role="img"
        aria-label={label}
        aria-describedby={description ? descId : undefined}
        className={`absolute inset-0 select-none rounded-[inherit] ${show3d && orbit ? "cursor-grab touch-pan-y active:cursor-grabbing" : ""}`}
      >
        {!show3d && fallback}
        {show3d && !ready && <div className="absolute inset-0 grid place-items-center opacity-60">{fallback}</div>}
        {show3d && (
          <Boundary onError={() => setFailed(true)}>
            <Suspense fallback={null}>
              <ObjectCanvas
                Scene={Scene as ComponentType<object & StageSceneProps>}
                sceneProps={sceneProps}
                theme={theme}
                reducedMotion={reducedMotion}
                active={active}
                continuous={(animate || autoRotate) && !reducedMotion}
                orbit={orbit}
                autoRotate={autoRotate && !reducedMotion}
                camera={camera}
                onReady={() => setReady(true)}
                onContextLost={() => setFailed(true)}
              />
            </Suspense>
          </Boundary>
        )}
      </div>
      {description && (
        <span id={descId} className="sr-only">
          {description}
        </span>
      )}
      {children && <div className="pointer-events-none absolute inset-0 [&>*]:pointer-events-auto">{children}</div>}
    </div>
  );
}
