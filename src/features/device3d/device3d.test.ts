import { afterEach, describe, expect, it } from "vitest";
import { setLang, translatorFor } from "@/i18n";
import { dictionaries } from "@/i18n/locales";
import { setPlatform } from "@/lib/platform";
import { resolveTheme } from "./hooks";
import { illustrationFor, Projector, boxFaces } from "./illustration";
import { groupBySize, keyboardLayout, KEYBOARD_UNITS } from "./keyboard";
import { stageLabels } from "./labels";
import { ambientPose, AMBIENT_FRAME_MS, isStageActive, motionPolicy, SwapAnimator, SWAP_TIMING } from "./motion";
import { clamp, keyAction, KEY_YAW_STEP, OrbitController, rubberBand, springStep, RETURN_SPRING } from "./orbit";
import { cameraPosition, extraPaddingFor, fitDistance, frameFor, PRESETS } from "./presets";
import { initialDpr, QualityMonitor } from "./quality";
import { detectWebGL, selectStageMode, webglAvailable, resetWebGLCache, type ProbeDocument } from "./support";
import { DEVICE_KINDS } from "./types";

const deg = (r: number) => (r * 180) / Math.PI;

describe("presets and framing", () => {
  it("every kind has a three-quarter hero pose within sensible limits", () => {
    for (const kind of DEVICE_KINDS) {
      const p = PRESETS[kind];
      expect(deg(p.heroYaw)).toBeGreaterThan(20);
      expect(deg(p.heroYaw)).toBeLessThan(50);
      expect(deg(p.heroElevation)).toBeGreaterThan(5);
      expect(deg(p.heroElevation)).toBeLessThan(35);
      expect(deg(p.yawLimit)).toBeLessThanOrEqual(50);
      const [lo, hi] = p.elevationRange;
      expect(lo).toBeLessThan(0);
      expect(hi).toBeGreaterThan(0);
      // The camera never goes under the floor or overhead.
      expect(deg(p.heroElevation + lo)).toBeGreaterThanOrEqual(0);
      expect(deg(p.heroElevation + hi)).toBeLessThan(45);
    }
  });

  it("the tower is tall, the laptop wide, the mini PC low", () => {
    const [tw, th] = PRESETS.tower.size;
    const [lw, lh] = PRESETS.laptop.size;
    const [mw, mh] = PRESETS.mini.size;
    expect(th / tw).toBeGreaterThan(2);
    expect(lw / lh).toBeGreaterThan(1.3);
    expect(mw / mh).toBeGreaterThan(2.5);
  });

  it("fits every corner of the box in the view frustum", () => {
    const size = [2, 4, 3] as const;
    const fov = 30;
    const aspect = 1.5;
    const yaw = 0.6;
    const elev = 0.2;
    const d = fitDistance(size, 2, yaw, elev, fov, aspect, 0, 0);
    const tanV = Math.tan((fov * Math.PI) / 360);
    const tanH = tanV * aspect;
    let touching = false;
    for (const x of [-1, 1]) for (const y of [0, 4]) for (const z of [-1.5, 1.5]) {
      const rx = x * Math.cos(yaw) + z * Math.sin(yaw);
      const rz = -x * Math.sin(yaw) + z * Math.cos(yaw);
      const ry = y - 2;
      const vy = ry * Math.cos(elev) - rz * Math.sin(elev);
      const vz = ry * Math.sin(elev) + rz * Math.cos(elev);
      const depth = d - vz;
      expect(Math.abs(rx) / depth).toBeLessThanOrEqual(tanH + 1e-9);
      expect(Math.abs(vy) / depth).toBeLessThanOrEqual(tanV + 1e-9);
      if (Math.abs(Math.abs(rx) / depth - tanH) < 1e-9 || Math.abs(Math.abs(vy) / depth - tanV) < 1e-9) touching = true;
    }
    // Tight: at least one corner is on the frustum edge.
    expect(touching).toBe(true);
  });

  it("a narrow viewport moves the camera back; padding moves it back further", () => {
    for (const kind of DEVICE_KINDS) {
      const wide = frameFor(kind, 16 / 9).distance;
      const narrow = frameFor(kind, 3 / 4).distance;
      expect(narrow).toBeGreaterThan(wide);
      // Side room costs distance only where the width is the limit (narrow viewports).
      expect(frameFor(kind, 16 / 9, { extraPadX: 0.22 }).distance).toBeGreaterThanOrEqual(wide);
      expect(frameFor(kind, 3 / 4, { extraPadX: 0.22 }).distance).toBeGreaterThan(narrow);
    }
  });

  it("frames the whole rotation range, so dragging never zooms or clips", () => {
    for (const kind of DEVICE_KINDS) {
      const p = PRESETS[kind];
      const f = frameFor(kind, 4 / 3);
      for (const yaw of [p.heroYaw - p.yawLimit, p.heroYaw + p.yawLimit]) {
        for (const e of p.elevationRange) {
          expect(fitDistance(p.size, f.target[1], yaw, p.heroElevation + e, p.fov, 4 / 3, 0, 0)).toBeLessThanOrEqual(f.distance);
        }
      }
    }
  });

  it("side indicators widen the framing; none leave it alone", () => {
    expect(extraPaddingFor([])).toBe(0);
    expect(extraPaddingFor(["start", "end"])).toBeGreaterThan(0);
  });

  it("places the camera above the aim point by the elevation", () => {
    const [x, y, z] = cameraPosition([0, 1, 0], 10, Math.PI / 6);
    expect(x).toBe(0);
    expect(y).toBeCloseTo(6);
    expect(z).toBeCloseTo(10 * Math.cos(Math.PI / 6));
  });
});

describe("rotation", () => {
  const limits = { yaw: (50 * Math.PI) / 180, elevation: [-0.1, 0.2] as const };
  const run = (c: OrbitController, from: number, ms: number) => {
    let now = from;
    for (let t = 0; t < ms; t += 16) {
      now += 16;
      c.step(0.016, now);
    }
    return now;
  };

  it("clamps and rubber-bands", () => {
    expect(clamp(5, 0, 1)).toBe(1);
    expect(rubberBand(0.5, -1, 1)).toBe(0.5);
    const over = rubberBand(3, -1, 1);
    expect(over).toBeGreaterThan(1);
    expect(over).toBeLessThan(1 + 0.18);
    expect(rubberBand(100, -1, 1)).toBeLessThan(1.18);
    expect(rubberBand(-3, -1, 1)).toBeCloseTo(-over);
  });

  it("the spring settles on its target", () => {
    let x = 1;
    let v = 0;
    for (let i = 0; i < 300; i++) [x, v] = springStep(x, v, 0, 1 / 60, RETURN_SPRING);
    expect(Math.abs(x)).toBeLessThan(1e-3);
  });

  it("a drag past the limit stretches a little, then settles inside it on release", () => {
    const c = new OrbitController({ limits });
    c.dragStart(0);
    c.dragBy(5000, 0, 400, 300, 10);
    let now = run(c, 10, 400);
    expect(c.yaw).toBeGreaterThan(limits.yaw);
    expect(c.yaw).toBeLessThan(limits.yaw * 1.2);
    c.dragEnd(now);
    now = run(c, now, 1000);
    expect(c.yaw).toBeLessThanOrEqual(limits.yaw + 1e-3);
    expect(c.yaw).toBeGreaterThan(limits.yaw * 0.9);
  });

  it("drifts back to the hero angle after the idle delay", () => {
    const c = new OrbitController({ limits, returnDelay: 1500 });
    c.nudge(0.4, 0.1, 0);
    run(c, 0, 1400);
    expect(c.yaw).toBeCloseTo(0.4, 2);
    expect(c.wakeAt()).toBe(1500);
    expect(c.atHero()).toBe(false);
    run(c, 1400, 3000);
    expect(Math.abs(c.yaw)).toBeLessThan(1e-3);
    expect(c.atHero()).toBe(true);
    expect(c.needsFrame()).toBe(false);
    expect(c.wakeAt()).toBeNull();
  });

  it("keyboard steps are clamped; Home resets", () => {
    const c = new OrbitController({ limits });
    for (let i = 0; i < 10; i++) c.nudge(KEY_YAW_STEP, 0, 0);
    run(c, 0, 1600);
    expect(c.yaw).toBeCloseTo(limits.yaw, 2);
    expect(keyAction("ArrowLeft")).toEqual({ kind: "nudge", yaw: -KEY_YAW_STEP, elevation: 0 });
    expect(keyAction("Home")).toEqual({ kind: "reset" });
    expect(keyAction("a")).toBeNull();
  });

  it("with reduced motion the model follows input directly and stays where it was left", () => {
    const c = new OrbitController({ limits, reducedMotion: true, returnDelay: 100 });
    c.dragStart(0);
    c.dragBy(5000, 0, 400, 300, 1);
    c.step(0.016, 2);
    expect(c.yaw).toBe(limits.yaw);
    c.dragEnd(3);
    c.step(0.016, 10_000);
    expect(c.yaw).toBe(limits.yaw);
    expect(c.wakeAt()).toBeNull();
    expect(c.needsFrame()).toBe(false);
  });
});

describe("motion policy", () => {
  it("reduced motion disables the ambient float and the animated swap", () => {
    expect(motionPolicy({ reducedMotion: true, ambient: true })).toEqual({ ambient: false, swap: "fade" });
    expect(motionPolicy({ reducedMotion: false, ambient: true })).toEqual({ ambient: true, swap: "animated" });
    expect(motionPolicy({ reducedMotion: false, ambient: false }).ambient).toBe(false);
  });

  it("renders only when not paused, visible and on screen", () => {
    expect(isStageActive({ paused: false, documentHidden: false, inView: true })).toBe(true);
    expect(isStageActive({ paused: true, documentHidden: false, inView: true })).toBe(false);
    expect(isStageActive({ paused: false, documentHidden: true, inView: true })).toBe(false);
    expect(isStageActive({ paused: false, documentHidden: false, inView: false })).toBe(false);
  });

  it("the ambient float starts from rest and stays subtle", () => {
    expect(ambientPose(0, 0.04)).toEqual({ y: 0, yaw: 0 });
    for (let t = 0; t < 60; t += 0.37) {
      const p = ambientPose(t, 0.04);
      expect(p.y).toBeGreaterThanOrEqual(0);
      expect(p.y).toBeLessThanOrEqual(0.04 + 1e-9);
      expect(Math.abs(deg(p.yaw))).toBeLessThan(2);
    }
    expect(AMBIENT_FRAME_MS).toBeGreaterThan(30);
  });

  it("a swap fades out, switches once, fades in, within ~600 ms", () => {
    const s = new SwapAnimator();
    s.swap(0, "animated");
    expect(s.sample(0).opacity).toBe(1);
    expect(s.sample(100).phase).toBe("out");
    const atSwitch = s.sample(SWAP_TIMING.animated.out);
    expect(atSwitch.switchNow).toBe(true);
    expect(atSwitch.opacity).toBe(0);
    expect(s.sample(SWAP_TIMING.animated.out + 10).switchNow).toBe(false);
    const end = s.sample(SWAP_TIMING.animated.out + SWAP_TIMING.animated.in + 5);
    expect(end).toEqual({ phase: "idle", opacity: 1, scale: 1, yaw: 0, switchNow: false });
    expect(s.busy).toBe(false);
    expect(SWAP_TIMING.animated.out + SWAP_TIMING.animated.in).toBeLessThanOrEqual(600);
    expect(SWAP_TIMING.animated.out + SWAP_TIMING.animated.in).toBeGreaterThanOrEqual(400);
  });

  it("the reduced-motion swap only fades", () => {
    const s = new SwapAnimator();
    s.swap(0, "fade");
    const mid = s.sample(50);
    expect(mid.scale).toBe(1);
    expect(mid.yaw).toBe(0);
    expect(mid.opacity).toBeLessThan(1);
  });
});

describe("WebGL support and fallback", () => {
  afterEach(() => resetWebGLCache());

  const fakeDoc = (ctx: unknown): ProbeDocument => ({ createElement: () => ({ getContext: () => ctx }) });

  it("the Node test environment has no WebGL: the stage degrades to the illustration", () => {
    expect(detectWebGL(undefined)).toBe(false);
    expect(webglAvailable()).toBe(false);
    expect(selectStageMode({ webgl: webglAvailable(), failed: false, ready: false })).toBe("fallback");
  });

  it("detects a context and releases the probe", () => {
    let lost = false;
    const ctx = { getExtension: (n: string) => (n === "WEBGL_lose_context" ? { loseContext: () => (lost = true) } : null) };
    expect(detectWebGL(fakeDoc(ctx))).toBe(true);
    expect(lost).toBe(true);
    expect(detectWebGL(fakeDoc(null))).toBe(false);
    expect(
      detectWebGL({
        createElement: () => ({
          getContext: () => {
            throw new Error("blocked");
          },
        }),
      }),
    ).toBe(false);
  });

  it("selects loading, 3D or the fallback", () => {
    expect(selectStageMode({ webgl: true, failed: false, ready: false })).toBe("loading");
    expect(selectStageMode({ webgl: true, failed: false, ready: true })).toBe("3d");
    expect(selectStageMode({ webgl: true, failed: true, ready: true })).toBe("fallback");
    expect(selectStageMode({ webgl: false, failed: false, ready: true })).toBe("fallback");
  });
});

describe("quality", () => {
  it("clamps the DPR and steps down on slow frames only", () => {
    expect(initialDpr(3)).toBe(1.75);
    expect(initialDpr(0.5)).toBe(1);
    expect(initialDpr(Number.NaN)).toBe(1);
    const q = new QualityMonitor(1.75);
    let changed: number | null = null;
    for (let i = 0; i < 40; i++) changed = q.push(8) ?? changed;
    expect(changed).toBeNull();
    for (let i = 0; i < 40; i++) changed = q.push(30) ?? changed;
    expect(changed).toBe(1.5);
    // Idle gaps between on-demand frames are ignored.
    expect(q.push(5000)).toBeNull();
  });
});

describe("labels", () => {
  afterEach(() => {
    setLang("it");
    setPlatform("windows");
  });

  it("names the model per kind in Italian and English", () => {
    const it = translatorFor("device3d", "it");
    expect(stageLabels(it, "tower", "3d").name).toBe("Modello 3D: PC fisso a torre");
    expect(stageLabels(it, "laptop", "loading").name).toBe("Modello 3D: portatile");
    expect(stageLabels(it, "mini", "fallback").name).toBe("Illustrazione: mini PC");
    expect(stageLabels(it, "mini", "fallback", "contextLost").description).toBe(dictionaries.it.device3d.contextLost);
    const en = translatorFor("device3d", "en");
    expect(stageLabels(en, "tower", "3d").name).toBe("3D model: tower desktop PC");
    expect(stageLabels(en, "tower", "3d").description).toContain("arrow keys");
  });

  it("uses the macOS wording for the reset key on a Mac", () => {
    const mac = translatorFor("device3d", "it", "macos");
    expect(stageLabels(mac, "laptop", "3d").description).toContain("Fn+←");
  });

  it("every kind has its labels in every language", () => {
    for (const lang of ["it", "en"] as const) {
      for (const kind of DEVICE_KINDS) {
        expect(dictionaries[lang].device3d[`model_${kind}`]).toBeTruthy();
        expect(dictionaries[lang].device3d[`illustration_${kind}`]).toBeTruthy();
      }
    }
  });
});

describe("theme", () => {
  it("an explicit setting wins over the OS", () => {
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
    expect(resolveTheme(undefined, true)).toBe("dark");
    expect(resolveTheme(undefined, false)).toBe("light");
  });
});

describe("laptop keyboard", () => {
  const unit = 0.175;
  const keys = keyboardLayout(unit, 0.03);

  it("has a full notebook layout that fits the deck without overlaps", () => {
    expect(keys.length).toBe(14 + 14 + 14 + 13 + 12 + 7 + 4);
    const half = (KEYBOARD_UNITS * unit) / 2;
    for (const k of keys) {
      expect(Math.abs(k.x) + k.w / 2).toBeLessThanOrEqual(half + 1e-9);
      expect(k.w).toBeGreaterThan(0);
      expect(k.d).toBeGreaterThan(0);
    }
    for (let i = 0; i < keys.length; i++) {
      for (let j = i + 1; j < keys.length; j++) {
        const a = keys[i]!;
        const b = keys[j]!;
        const overlap = Math.abs(a.x - b.x) < (a.w + b.w) / 2 - 1e-9 && Math.abs(a.z - b.z) < (a.d + b.d) / 2 - 1e-9;
        expect(overlap, `keys ${i} and ${j}`).toBe(false);
      }
    }
  });

  it("groups caps by size for instancing", () => {
    const groups = groupBySize(keys);
    const total = [...groups.values()].reduce((s, g) => s + g.length, 0);
    expect(total).toBe(keys.length);
    expect(groups.size).toBeLessThan(12);
  });
});

describe("fallback illustration", () => {
  it("a box in a three-quarter view shows three faces", () => {
    const p = new Projector((36 * Math.PI) / 180, (15 * Math.PI) / 180);
    const visible = boxFaces([-1, 0, -1], [1, 2, 1]).filter((f) => p.facing(f.normal)).map((f) => f.name);
    expect(visible.sort()).toEqual(["front", "left", "top"]);
  });

  it("every kind draws a framed illustration", () => {
    for (const kind of DEVICE_KINDS) {
      const art = illustrationFor(kind);
      expect(art.shapes.length).toBeGreaterThan(4);
      const [, , w, h] = art.viewBox;
      expect(w).toBeGreaterThan(0);
      expect(h).toBeGreaterThan(0);
      for (const s of art.shapes) for (const [x, y] of s.points) expect(Number.isFinite(x + y)).toBe(true);
    }
  });
});

describe("bundle boundary", () => {
  // Everything outside scene/ is in the main bundle: it must not pull three.js in.
  const sources = import.meta.glob(["./*.ts", "./*.tsx", "!./*.test.ts"], { query: "?raw", import: "default", eager: true }) as Record<string, string>;

  it("the public module imports no three.js, fiber or drei", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(5);
    for (const [file, text] of Object.entries(sources)) {
      expect(text, file).not.toMatch(/from\s+["'](three|@react-three\/[\w-]+)(\/[^"']*)?["']/);
      // The scene is reached only through a dynamic import (a separate chunk).
      expect(text.replace(/import\("\.\/scene\/DeviceScene"\)/g, "").replace(/typeof import\("\.\/scene\/DeviceScene"\)/g, ""), file).not.toMatch(/["']\.\/scene\//);
    }
  });
});
