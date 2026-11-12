import { PRESETS } from "./presets";
import type { DeviceKind } from "./types";

/**
 * The static illustration shown without WebGL: the same models reduced to boxes, drawn in
 * the same hero pose (orthographic), so the stage keeps its footprint and framing. Pure:
 * returns polygons; `StageFallback` renders them as SVG with theme tokens.
 */

type V3 = readonly [number, number, number];
type P2 = readonly [number, number];

export type Tone = "top" | "front" | "side" | "dark" | "glass" | "screen" | "line" | "light";

export interface Shape2D {
  tone: Tone;
  points: P2[];
  depth: number;
}

interface Face {
  name: "front" | "back" | "left" | "right" | "top";
  /** Model-space corners: (u0,v0) (u1,v0) (u1,v1) (u0,v1). */
  corners: [V3, V3, V3, V3];
  normal: V3;
}

export class Projector {
  private readonly cy: number;
  private readonly sy: number;
  private readonly ce: number;
  private readonly se: number;

  constructor(yaw: number, elevation: number) {
    this.cy = Math.cos(yaw);
    this.sy = Math.sin(yaw);
    this.ce = Math.cos(elevation);
    this.se = Math.sin(elevation);
  }

  /** Screen point (SVG: y down) of a model-space point. */
  point([x, y, z]: V3): P2 {
    const rz = -x * this.sy + z * this.cy;
    return [x * this.cy + z * this.sy, -(y * this.ce - rz * this.se)];
  }

  /** Distance towards the camera (larger = nearer). */
  depth([x, y, z]: V3): number {
    const rz = -x * this.sy + z * this.cy;
    return y * this.se + rz * this.ce;
  }

  /** A direction faces the camera. */
  facing(n: V3): boolean {
    return this.depth(n) > 1e-6;
  }
}

/** Box faces (no bottom), optionally tilted about the x axis through `pivot`. */
export function boxFaces(min: V3, max: V3, tilt = 0, pivot: V3 = [0, 0, 0]): Face[] {
  const [x0, y0, z0] = min;
  const [x1, y1, z1] = max;
  const c = Math.cos(tilt);
  const s = Math.sin(tilt);
  const rot = ([x, y, z]: V3): V3 => {
    const py = y - pivot[1];
    const pz = z - pivot[2];
    return [x, pivot[1] + py * c - pz * s, pivot[2] + py * s + pz * c];
  };
  const dir = ([x, y, z]: V3): V3 => [x, y * c - z * s, y * s + z * c];
  const face = (name: Face["name"], corners: [V3, V3, V3, V3], normal: V3): Face => ({
    name,
    corners: corners.map(rot) as [V3, V3, V3, V3],
    normal: dir(normal),
  });
  return [
    face("front", [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], [0, 0, 1]),
    face("back", [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]], [0, 0, -1]),
    face("right", [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]], [1, 0, 0]),
    face("left", [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]], [-1, 0, 0]),
    face("top", [[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]], [0, 1, 0]),
  ];
}

/** A point on a face at (u, v) ∈ [0, 1]² (bilinear; exact for planar quads). */
function onFace(f: Face, u: number, v: number): V3 {
  const [a, b, c, d] = f.corners;
  const lerp = (p: V3, q: V3, t: number): V3 => [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t];
  return lerp(lerp(a, b, u), lerp(d, c, u), v);
}

const center = (pts: readonly V3[]): V3 => {
  const n = pts.length;
  return [pts.reduce((s, p) => s + p[0], 0) / n, pts.reduce((s, p) => s + p[1], 0) / n, pts.reduce((s, p) => s + p[2], 0) / n];
};

class Drawing {
  readonly shapes: Shape2D[] = [];
  constructor(readonly proj: Projector) {}

  /** Visible faces of a box, toned by orientation. */
  box(faces: Face[], tones: Partial<Record<Face["name"], Tone>> = {}): Record<string, Face> {
    const byName: Record<string, Face> = {};
    for (const f of faces) {
      byName[f.name] = f;
      if (!this.proj.facing(f.normal)) continue;
      const tone = tones[f.name] ?? (f.name === "top" ? "top" : f.name === "front" ? "front" : "side");
      this.polygon(tone, f.corners);
    }
    return byName;
  }

  polygon(tone: Tone, pts: readonly V3[], bias = 0): void {
    this.shapes.push({ tone, points: pts.map((p) => this.proj.point(p)), depth: this.proj.depth(center(pts)) + bias });
  }

  /** A rectangle inset on a face, if the face is visible. */
  rect(f: Face | undefined, tone: Tone, u0: number, v0: number, u1: number, v1: number, bias = 0.001): void {
    if (!f || !this.proj.facing(f.normal)) return;
    this.polygon(tone, [onFace(f, u0, v0), onFace(f, u1, v0), onFace(f, u1, v1), onFace(f, u0, v1)], bias);
  }

  /** An ellipse on a face (radii in face units). */
  dot(f: Face | undefined, tone: Tone, u: number, v: number, ru: number, rv: number, bias = 0.002): void {
    if (!f || !this.proj.facing(f.normal)) return;
    const pts: V3[] = [];
    for (let i = 0; i < 16; i++) {
      const a = (i / 16) * Math.PI * 2;
      pts.push(onFace(f, u + Math.cos(a) * ru, v + Math.sin(a) * rv));
    }
    this.polygon(tone, pts, bias);
  }
}

export interface Illustration {
  viewBox: [number, number, number, number];
  shapes: Shape2D[];
  /** Floor shadow: centre and radii in viewBox units. */
  shadow: { cx: number; cy: number; rx: number; ry: number };
}

function drawTower(d: Drawing): void {
  const [w, h, dep] = PRESETS.tower.size;
  const f = d.box(boxFaces([-w / 2, 0.1, -dep / 2], [w / 2, h, dep / 2]), { left: "glass" });
  // Through the glass: board, GPU, PSU shroud, cooler.
  d.rect(f.left, "side", 0.02, 0.02, 0.96, 0.2);
  d.rect(f.left, "dark", 0.1, 0.42, 0.78, 0.53);
  d.rect(f.left, "line", 0.3, 0.64, 0.5, 0.84);
  d.rect(f.left, "line", 0.56, 0.62, 0.6, 0.86);
  d.rect(f.left, "line", 0.62, 0.62, 0.66, 0.86);
  // Front: intake grille with slats, I/O band, power button.
  d.rect(f.front, "dark", 0.1, 0.06, 0.9, 0.84);
  for (let i = 0; i < 14; i++) {
    const v = 0.1 + (i * 0.7) / 13;
    d.rect(f.front, "line", 0.12, v, 0.88, v + 0.008, 0.002);
  }
  d.dot(f.front, "light", 0.5, 0.92, 0.07, 0.032);
  d.rect(f.front, "dark", 0.18, 0.905, 0.3, 0.935);
  d.rect(f.front, "dark", 0.7, 0.905, 0.8, 0.935);
  // Top exhaust.
  d.rect(f.top, "dark", 0.12, 0.18, 0.88, 0.8);
}

function drawLaptop(d: Drawing): void {
  const [w] = PRESETS.laptop.size;
  const base = 2.15;
  const t = 0.155;
  const shift = 0.35;
  const b = d.box(boxFaces([-w / 2, 0, -base / 2 + shift], [w / 2, t, base / 2 + shift]));
  d.rect(b.top, "dark", 0.07, 0.52, 0.93, 0.95);
  d.rect(b.top, "side", 0.3, 0.06, 0.7, 0.42);
  const pivotZ = -base / 2 + shift + 0.05;
  const lid = d.box(boxFaces([-w / 2, t * 0.55, pivotZ - 0.055], [w / 2, t * 0.55 + 2.08, pivotZ], (-20 * Math.PI) / 180, [0, t * 0.55, pivotZ]), {
    front: "dark",
  });
  d.rect(lid.front, "screen", 0.025, 0.05, 0.975, 0.955);
}

function drawMini(d: Drawing): void {
  const [w, h, dep] = PRESETS.mini.size;
  const f = d.box(boxFaces([-w / 2, 0.02, -dep / 2], [w / 2, h, dep / 2]));
  d.rect(f.top, "dark", 0.08, 0.08, 0.92, 0.92);
  d.rect(f.front, "dark", 0.12, 0.42, 0.2, 0.56);
  d.rect(f.front, "dark", 0.24, 0.42, 0.32, 0.56);
  d.rect(f.front, "dark", 0.38, 0.4, 0.5, 0.58);
  d.dot(f.front, "light", 0.86, 0.5, 0.018, 0.05);
}

/** Polygons for a kind, already sorted back to front, and a viewBox with the preset padding. */
export function illustrationFor(kind: DeviceKind): Illustration {
  const p = PRESETS[kind];
  const d = new Drawing(new Projector(p.heroYaw, p.heroElevation));
  if (kind === "tower") drawTower(d);
  else if (kind === "laptop") drawLaptop(d);
  else drawMini(d);
  const shapes = d.shapes.sort((a, b) => a.depth - b.depth);
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const s of shapes) {
    for (const [x, y] of s.points) {
      minX = Math.min(minX, x);
      maxX = Math.max(maxX, x);
      minY = Math.min(minY, y);
      maxY = Math.max(maxY, y);
    }
  }
  // Shadow: the footprint's projected extent.
  const { width, depth, z } = p.shadow;
  const foot = [
    [-width / 2, 0, z - depth / 2],
    [width / 2, 0, z - depth / 2],
    [width / 2, 0, z + depth / 2],
    [-width / 2, 0, z + depth / 2],
  ].map((q) => d.proj.point(q as unknown as V3));
  const fx = foot.map((q) => q[0]);
  const fy = foot.map((q) => q[1]);
  const shadow = {
    cx: (Math.min(...fx) + Math.max(...fx)) / 2,
    cy: (Math.min(...fy) + Math.max(...fy)) / 2,
    rx: ((Math.max(...fx) - Math.min(...fx)) / 2) * 1.08,
    ry: ((Math.max(...fy) - Math.min(...fy)) / 2) * 1.15,
  };
  maxY = Math.max(maxY, shadow.cy + shadow.ry * 0.6);
  const spanX = maxX - minX;
  const spanY = maxY - minY;
  const padX = (spanX * p.padding) / (1 - p.padding);
  const padY = (spanY * p.padding) / (1 - p.padding);
  return {
    viewBox: [minX - padX, minY - padY, spanX + 2 * padX, spanY + 2 * padY],
    shapes,
    shadow,
  };
}
