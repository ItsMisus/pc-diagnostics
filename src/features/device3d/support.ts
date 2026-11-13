import type { StageMode } from "./types";

/** The part of `document` the WebGL probe needs (injectable for tests). */
export interface ProbeDocument {
  createElement(tag: "canvas"): {
    getContext(id: string, options?: unknown): unknown;
  };
}

interface LoseContext {
  loseContext(): void;
}

interface ProbeContext {
  getExtension?(name: string): unknown;
}

let cached: boolean | undefined;

/**
 * Whether this WebView can create a WebGL context (WebGL 2, else 1). The probe context is
 * released at once so it does not count against the browser's context limit. jsdom, the
 * Node test environment and WebViews with WebGL disabled answer false.
 */
export function detectWebGL(doc: ProbeDocument | undefined = typeof document !== "undefined" ? document : undefined): boolean {
  if (!doc) return false;
  try {
    const canvas = doc.createElement("canvas");
    const options = { failIfMajorPerformanceCaveat: true };
    const ctx = (canvas.getContext("webgl2", options) ?? canvas.getContext("webgl", options)) as ProbeContext | null | undefined;
    if (!ctx) return false;
    const lose = ctx.getExtension?.("WEBGL_lose_context") as LoseContext | null | undefined;
    lose?.loseContext();
    return true;
  } catch {
    return false;
  }
}

/** `detectWebGL()` for the real document, probed once per session. */
export function webglAvailable(): boolean {
  cached ??= detectWebGL();
  return cached;
}

/** Test hook. */
export function resetWebGLCache(): void {
  cached = undefined;
}

export interface StageModeInput {
  /** The WebGL probe answered yes. */
  webgl: boolean;
  /** The scene's context was lost, or the 3D bundle failed to load or render. */
  failed: boolean;
  /** The scene rendered its first frame. */
  ready: boolean;
}

/**
 * What the stage shows. Without WebGL, or after a failure, the static illustration (same
 * framing, all information still there). Otherwise a placeholder until the first frame.
 */
export function selectStageMode({ webgl, failed, ready }: StageModeInput): StageMode {
  if (!webgl || failed) return "fallback";
  return ready ? "3d" : "loading";
}
