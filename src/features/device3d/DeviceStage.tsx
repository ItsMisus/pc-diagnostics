import { Component, Fragment, lazy, Suspense, useEffect, useId, useMemo, useRef, useState, type ErrorInfo, type ReactNode } from "react";
import { useT } from "@/i18n";
import { useDocumentHidden, useDocumentTheme, useInView, usePrefersReducedMotion } from "./hooks";
import { stageLabels, type FallbackReason } from "./labels";
import { isStageActive } from "./motion";
import { extraPaddingFor } from "./presets";
import { StageFallback } from "./StageFallback";
import { selectStageMode, webglAvailable } from "./support";
import { INDICATOR_SLOTS, type DeviceIndicator, type DeviceKind, type IndicatorSlot, type StageMode, type StageTheme } from "./types";

let scenePromise: Promise<typeof import("./scene/DeviceScene")> | undefined;
/** The 3D bundle (three.js, fiber, drei, models), loaded once on first use. */
const loadScene = () => (scenePromise ??= import("./scene/DeviceScene"));
const DeviceScene = lazy(loadScene);

/**
 * Start downloading the 3D bundle ahead of time (e.g. when the app is idle), so the stage
 * appears at once. Does nothing without WebGL. Safe to call repeatedly.
 */
export function preloadDeviceStage(): void {
  if (webglAvailable()) {
    loadScene().catch(() => {
      // A failed preload is retried (and reported through the fallback) on first render.
      scenePromise = undefined;
    });
  }
}

export interface DeviceStageProps {
  /** The device family to show. Changing it plays a short swap. */
  kind: DeviceKind;
  /** Theme of the surface behind the stage; by default it follows the app (setting + OS). */
  theme?: StageTheme;
  /**
   * Stop all rendering: pass true while a benchmark runs or when the stage is not shown.
   * The stage also pauses by itself while the window is hidden or the stage is off screen.
   */
  paused?: boolean;
  /** Slow ambient float while idle (default true). Always off with reduced motion. */
  ambient?: boolean;
  /** Small indicators placed around the model (see `IndicatorSlot`). */
  indicators?: readonly DeviceIndicator[];
  /** CSS aspect ratio of the stage (default "4 / 3"); null fills the parent's height. */
  aspectRatio?: string | null;
  className?: string;
  /** Loading → 3d, or → fallback (no WebGL, context lost, bundle failed). */
  onModeChange?: (mode: StageMode) => void;
}

const SLOT_CLASS: Record<IndicatorSlot, string> = {
  "top-start": "top-4 start-4 items-start",
  "top-end": "top-4 end-4 items-end",
  start: "top-1/2 start-4 -translate-y-1/2 items-start",
  end: "top-1/2 end-4 -translate-y-1/2 items-end",
  "bottom-start": "bottom-4 start-4 items-start",
  "bottom-end": "bottom-4 end-4 items-end",
};

class SceneBoundary extends Component<{ onError: () => void; children: ReactNode }, { failed: boolean }> {
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
 * The signature element of the Overview: a procedural 3D model of the user's kind of device
 * (tower, laptop, mini PC) on a soft studio stage. The 3D code is a separate lazy chunk;
 * until its first frame a soft silhouette holds the same footprint. Without WebGL (or after
 * a context loss) a static illustration in the same framing is shown instead.
 *
 * Interaction: drag to turn (within ±50°, a small vertical range), arrow keys when focused,
 * Home for the starting view; it settles back to the hero angle on its own.
 */
export function DeviceStage({
  kind,
  theme,
  paused = false,
  ambient = true,
  indicators,
  aspectRatio = "4 / 3",
  className = "",
  onModeChange,
}: DeviceStageProps) {
  const t = useT("device3d");
  const [webgl] = useState(webglAvailable);
  const [failure, setFailure] = useState<FallbackReason | null>(null);
  const [ready, setReady] = useState(false);
  const [placeholderGone, setPlaceholderGone] = useState(false);
  const [target, setTarget] = useState<HTMLDivElement | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  const appTheme = useDocumentTheme();
  const reducedMotion = usePrefersReducedMotion();
  const hidden = useDocumentHidden();
  const inView = useInView(rootRef);
  const active = isStageActive({ paused, documentHidden: hidden, inView });

  const mode = selectStageMode({ webgl, failed: failure !== null, ready });
  const labels = stageLabels(t, kind, mode, failure ?? "noWebgl");
  const descId = useId();

  const usedSlots = useMemo(() => [...new Set((indicators ?? []).map((i) => i.slot))], [indicators]);
  const extraPadX = extraPaddingFor(usedSlots);

  const modeRef = useRef(onModeChange);
  modeRef.current = onModeChange;
  useEffect(() => {
    modeRef.current?.(mode);
  }, [mode]);

  // The silhouette fades out under the canvas fade-in, then leaves the DOM (a timer, not
  // transitionend: with reduced motion the transition has no duration and never ends).
  useEffect(() => {
    if (!ready) return;
    const id = window.setTimeout(() => setPlaceholderGone(true), 400);
    return () => window.clearTimeout(id);
  }, [ready]);

  const showScene = webgl && failure === null;
  const showSilhouette = showScene && !placeholderGone;

  return (
    <div ref={rootRef} className={`relative isolate ${className}`} style={aspectRatio ? { aspectRatio } : undefined} data-stage-mode={mode}>
      <div
        ref={setTarget}
        role="img"
        aria-label={labels.name}
        aria-describedby={descId}
        aria-busy={mode === "loading" || undefined}
        tabIndex={mode === "3d" ? 0 : undefined}
        className={`absolute inset-0 touch-pan-y select-none rounded-[inherit] ${mode === "3d" ? "cursor-grab data-[dragging]:cursor-grabbing" : ""}`}
      >
        {mode === "fallback" && <StageFallback kind={kind} variant="illustration" />}
        {showSilhouette && (
          <div
            className="absolute inset-0 transition-opacity duration-300 ease-out"
            style={{ opacity: ready ? 0 : 1 }}
          >
            <StageFallback kind={kind} variant="silhouette" />
          </div>
        )}
        {showScene && (
          <SceneBoundary onError={() => setFailure("contextLost")}>
            <Suspense fallback={null}>
              <DeviceScene
                kind={kind}
                theme={theme ?? appTheme}
                active={active}
                ambient={ambient}
                reducedMotion={reducedMotion}
                extraPadX={extraPadX}
                eventTarget={target}
                onReady={() => setReady(true)}
                onContextLost={() => setFailure("contextLost")}
              />
            </Suspense>
          </SceneBoundary>
        )}
      </div>
      <span id={descId} className="sr-only">
        {labels.description}
      </span>
      {indicators && indicators.length > 0 && (
        <div className="pointer-events-none absolute inset-0">
          {INDICATOR_SLOTS.map((slot) => {
            const items = indicators.filter((i) => i.slot === slot);
            if (items.length === 0) return null;
            return (
              <div key={slot} className={`pointer-events-auto absolute flex flex-col gap-2 ${SLOT_CLASS[slot]}`}>
                {items.map((i) => (
                  <Fragment key={i.id}>{i.content}</Fragment>
                ))}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
