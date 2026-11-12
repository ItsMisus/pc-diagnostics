import { useFrame, useThree } from "@react-three/fiber";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Group, PerspectiveCamera } from "three";
import { AMBIENT_FRAME_MS, SwapAnimator, ambientPose, damp, motionPolicy } from "../motion";
import { keyAction, OrbitController } from "../orbit";
import { cameraPosition, frameFor, PRESETS } from "../presets";
import { QualityMonitor } from "../quality";
import type { DeviceKind, StageTheme } from "../types";
import { Laptop } from "./Laptop";
import { MiniPc } from "./MiniPc";
import { FloorShadow } from "./Studio";
import { Tower } from "./Tower";

export interface RigProps {
  kind: DeviceKind;
  theme: StageTheme;
  active: boolean;
  ambient: boolean;
  reducedMotion: boolean;
  extraPadX: number;
  eventTarget: HTMLElement | null;
  onReady: () => void;
}

/**
 * Drives everything that moves, on demand: the turntable (drag, keys, return to the hero
 * angle), the ambient float, the model swap and the camera framing. It renders a frame only
 * while something changes; when only the ambient float plays it asks for 30 frames per
 * second; at rest it renders nothing (and wakes itself for the return to the hero angle).
 */
export function Rig({ kind, theme, active, ambient, reducedMotion, extraPadX, eventTarget, onReady }: RigProps) {
  const invalidate = useThree((s) => s.invalidate);
  const camera = useThree((s) => s.camera) as PerspectiveCamera;
  const gl = useThree((s) => s.gl);
  const setDpr = useThree((s) => s.setDpr);
  const viewport = useThree((s) => s.size);
  const [shown, setShown] = useState<DeviceKind>(kind);
  const preset = PRESETS[shown];
  const frame = useMemo(
    () => frameFor(shown, viewport.width / Math.max(viewport.height, 1), { extraPadX }),
    [shown, viewport.width, viewport.height, extraPadX],
  );

  const orbit = useMemo(
    () => new OrbitController({ limits: { yaw: PRESETS[kind].yawLimit, elevation: PRESETS[kind].elevationRange }, reducedMotion }),
    // Created once; limits and reduced motion are updated below.
    [],
  );
  const swap = useMemo(() => new SwapAnimator(), []);
  const quality = useMemo(() => new QualityMonitor(gl.getPixelRatio()), [gl]);
  const policy = motionPolicy({ reducedMotion, ambient });

  const turntable = useRef<Group>(null);
  const lift = useRef<Group>(null);
  const live = useRef({ kind, policy, ambientT: 0, ambientW: 0, ready: false, wasContinuous: false, timer: 0, timerAt: 0, opacity: -1 });
  live.current.kind = kind;
  live.current.policy = policy;

  useEffect(() => {
    orbit.setLimits({ yaw: preset.yawLimit, elevation: preset.elevationRange });
  }, [orbit, preset]);
  useEffect(() => orbit.setReducedMotion(reducedMotion), [orbit, reducedMotion]);

  // First appearance: fade (and settle) in.
  useEffect(() => {
    swap.enter(performance.now(), live.current.policy.swap);
    invalidate();
  }, [swap, invalidate]);

  // A new kind while paused swaps at once (nothing renders, nothing to animate).
  useEffect(() => {
    if (kind === shown) return;
    if (!active) {
      setShown(kind);
      orbit.home();
      swap.finish();
    }
    invalidate();
  }, [kind, shown, active, orbit, swap, invalidate]);

  // Resume, theme or framing change: draw once.
  useEffect(() => {
    if (active) invalidate();
  }, [active, theme, frame, invalidate]);

  // Timers that wake the on-demand loop: cleared when paused or unmounted.
  const schedule = (ms: number) => {
    const l = live.current;
    const at = performance.now() + Math.max(ms, 0);
    if (l.timer && l.timerAt <= at) return;
    if (l.timer) window.clearTimeout(l.timer);
    l.timerAt = at;
    l.timer = window.setTimeout(() => {
      l.timer = 0;
      invalidate();
    }, Math.max(ms, 0));
  };
  useEffect(() => {
    const l = live.current;
    if (!active && l.timer) {
      window.clearTimeout(l.timer);
      l.timer = 0;
    }
    return () => {
      if (l.timer) window.clearTimeout(l.timer);
      l.timer = 0;
    };
  }, [active]);

  // Pointer and keyboard input on the stage element.
  useEffect(() => {
    const el = eventTarget;
    if (!el) return;
    let last: { x: number; y: number; id: number } | null = null;
    const down = (e: PointerEvent) => {
      if (e.button !== 0 || !e.isPrimary) return;
      last = { x: e.clientX, y: e.clientY, id: e.pointerId };
      try {
        el.setPointerCapture(e.pointerId);
      } catch {
        // Capture is a convenience (drag outside the stage); without it the drag still works.
      }
      el.dataset.dragging = "true";
      orbit.dragStart(performance.now());
      invalidate();
    };
    const move = (e: PointerEvent) => {
      if (!last || e.pointerId !== last.id) return;
      const rect = el.getBoundingClientRect();
      orbit.dragBy(e.clientX - last.x, e.clientY - last.y, rect.width, rect.height, performance.now());
      last = { x: e.clientX, y: e.clientY, id: e.pointerId };
      invalidate();
    };
    const up = (e: PointerEvent) => {
      if (!last || e.pointerId !== last.id) return;
      last = null;
      delete el.dataset.dragging;
      orbit.dragEnd(performance.now());
      invalidate();
    };
    const key = (e: KeyboardEvent) => {
      if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
      const action = keyAction(e.key);
      if (!action) return;
      e.preventDefault();
      const now = performance.now();
      if (action.kind === "reset") orbit.reset(now);
      else orbit.nudge(action.yaw, action.elevation, now);
      invalidate();
    };
    el.addEventListener("pointerdown", down);
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
    el.addEventListener("lostpointercapture", up);
    el.addEventListener("keydown", key);
    return () => {
      el.removeEventListener("pointerdown", down);
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      el.removeEventListener("lostpointercapture", up);
      el.removeEventListener("keydown", key);
      delete el.dataset.dragging;
    };
  }, [eventTarget, orbit, invalidate]);

  useFrame((_, delta) => {
    const l = live.current;
    const now = performance.now();
    const dt = Math.min(delta, 0.05);

    if (l.kind !== shown && !swap.busy) swap.swap(now, l.policy.swap);
    const s = swap.sample(now);
    if (s.switchNow && l.kind !== shown) {
      setShown(l.kind);
      orbit.home();
    }

    orbit.step(dt, now);
    const ambientOn = l.policy.ambient && orbit.atHero() && !swap.busy;
    l.ambientW = l.policy.ambient ? damp(l.ambientW, ambientOn ? 1 : 0, 2.5, dt) : 0;
    if (l.ambientW > 1e-3) l.ambientT += dt;
    const pose = ambientPose(l.ambientT, preset.floatAmplitude);

    const t = turntable.current;
    const g = lift.current;
    if (t && g) {
      t.rotation.y = preset.heroYaw + orbit.yaw + s.yaw + pose.yaw * l.ambientW;
      g.position.y = pose.y * l.ambientW;
      g.scale.setScalar(s.scale);
    }

    if (camera.fov !== frame.fov) {
      camera.fov = frame.fov;
      camera.updateProjectionMatrix();
    }
    const [x, y, z] = cameraPosition(frame.target, frame.distance, preset.heroElevation + orbit.elevation);
    camera.position.set(x, y, z);
    camera.lookAt(frame.target[0], frame.target[1], frame.target[2]);
    const near = Math.max(frame.distance * 0.2, 0.1);
    if (Math.abs(camera.near - near) > 1e-3) {
      camera.near = near;
      camera.far = frame.distance * 4;
      camera.updateProjectionMatrix();
    }

    const opacity = Math.round(s.opacity * 1000) / 1000;
    if (opacity !== l.opacity) {
      gl.domElement.style.opacity = String(opacity);
      l.opacity = opacity;
    }

    // Next frame: continuous while something moves, 30 fps for the ambient float, else idle.
    const continuous = orbit.needsFrame() || swap.busy || l.kind !== shown;
    if (continuous) {
      if (l.wasContinuous) {
        const next = quality.push(delta * 1000);
        if (next !== null) setDpr(next);
      }
      invalidate();
    } else if (l.ambientW > 1e-3) {
      quality.reset();
      schedule(AMBIENT_FRAME_MS);
    } else {
      quality.reset();
      const wake = orbit.wakeAt();
      if (wake !== null) schedule(wake - now);
    }
    l.wasContinuous = continuous;

    if (!l.ready) {
      l.ready = true;
      onReady();
    }
  });

  return (
    <group ref={turntable}>
      <FloorShadow preset={preset} theme={theme} />
      <group ref={lift}>
        {shown === "tower" ? <Tower key="tower" /> : shown === "laptop" ? <Laptop key="laptop" /> : <MiniPc key="mini" />}
      </group>
    </group>
  );
}
