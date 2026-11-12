import { CanvasTexture, ClampToEdgeWrapping, LinearMipmapLinearFilter, RepeatWrapping, SRGBColorSpace } from "three";
import type { StageTheme } from "../types";

/**
 * Procedural textures drawn on 2D canvases at runtime: nothing is fetched or bundled.
 * The caller owns each texture and disposes it.
 */

function canvas(w: number, h: number): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const ctx = c.getContext("2d");
  if (!ctx) throw new Error("2D canvas unavailable");
  return [c, ctx];
}

/**
 * One tile of a hexagonal perforated sheet: light metal, dark holes. Used as a colour map
 * (not an alpha test), so mipmaps average it to the right mid-tone at a distance instead of
 * shimmering.
 */
export function perforationTexture(repeatX: number, repeatY: number): CanvasTexture {
  const w = 64;
  const h = Math.round(64 * Math.sqrt(3));
  const [c, ctx] = canvas(w, h);
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, w, h);
  ctx.fillStyle = "#0b0b0c";
  const r = w * 0.3;
  const hole = (x: number, y: number) => {
    ctx.beginPath();
    ctx.arc(x, y, r, 0, Math.PI * 2);
    ctx.fill();
  };
  hole(w / 2, h / 4);
  hole(0, (3 * h) / 4);
  hole(w, (3 * h) / 4);
  const t = new CanvasTexture(c);
  t.wrapS = RepeatWrapping;
  t.wrapT = RepeatWrapping;
  t.repeat.set(repeatX, repeatY);
  t.colorSpace = SRGBColorSpace;
  t.minFilter = LinearMipmapLinearFilter;
  t.anisotropy = 8;
  return t;
}

/** A calm abstract wallpaper for the laptop screen: deep graphite to a soft azure glow. */
export function wallpaperTexture(theme: StageTheme): CanvasTexture {
  const [c, ctx] = canvas(512, 320);
  const base = ctx.createLinearGradient(0, 0, 512, 320);
  if (theme === "dark") {
    base.addColorStop(0, "#0d1220");
    base.addColorStop(0.55, "#16233d");
    base.addColorStop(1, "#0a0e18");
  } else {
    base.addColorStop(0, "#c9d6ea");
    base.addColorStop(0.55, "#8fb0dd");
    base.addColorStop(1, "#5c7fb8");
  }
  ctx.fillStyle = base;
  ctx.fillRect(0, 0, 512, 320);
  // Two soft light blooms and one broad wave, like a lock-screen gradient.
  const bloom = (x: number, y: number, r: number, color: string) => {
    const g = ctx.createRadialGradient(x, y, 0, x, y, r);
    g.addColorStop(0, color);
    g.addColorStop(1, "rgba(0,0,0,0)");
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, 512, 320);
  };
  bloom(360, 120, 260, theme === "dark" ? "rgba(64,132,255,0.45)" : "rgba(255,255,255,0.55)");
  bloom(110, 280, 220, theme === "dark" ? "rgba(120,170,255,0.22)" : "rgba(70,120,210,0.35)");
  ctx.globalAlpha = theme === "dark" ? 0.22 : 0.3;
  ctx.fillStyle = theme === "dark" ? "#7fb2ff" : "#ffffff";
  ctx.beginPath();
  ctx.moveTo(0, 230);
  ctx.bezierCurveTo(140, 150, 300, 300, 512, 170);
  ctx.lineTo(512, 205);
  ctx.bezierCurveTo(300, 330, 150, 190, 0, 262);
  ctx.closePath();
  ctx.fill();
  ctx.globalAlpha = 1;
  const t = new CanvasTexture(c);
  t.colorSpace = SRGBColorSpace;
  t.wrapS = ClampToEdgeWrapping;
  t.wrapT = ClampToEdgeWrapping;
  t.anisotropy = 4;
  return t;
}

/**
 * Soft shadow of a rounded footprint: white with alpha (the material colour tints it). The footprint fills `fill` of the
 * texture on each axis; the rest is room for the blur. Drawn with the canvas shadow (the rect
 * itself is drawn off-canvas), which every WebView supports.
 */
export function shadowTexture(aspect: number, blur: number, fill: number): CanvasTexture {
  const w = 256;
  const h = Math.max(32, Math.round(256 / Math.max(aspect, 0.1)));
  const [c, ctx] = canvas(w, h);
  const rw = w * fill;
  const rh = h * fill;
  const r = Math.min(rw, rh) * 0.12;
  const offset = w * 4;
  ctx.shadowColor = "rgba(255,255,255,1)";
  ctx.shadowBlur = blur;
  ctx.shadowOffsetX = offset;
  ctx.fillStyle = "#fff";
  ctx.beginPath();
  const x = (w - rw) / 2 - offset;
  const y = (h - rh) / 2;
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + rw, y, x + rw, y + rh, r);
  ctx.arcTo(x + rw, y + rh, x, y + rh, r);
  ctx.arcTo(x, y + rh, x, y, r);
  ctx.arcTo(x, y, x + rw, y, r);
  ctx.closePath();
  ctx.fill();
  const t = new CanvasTexture(c);
  t.wrapS = ClampToEdgeWrapping;
  t.wrapT = ClampToEdgeWrapping;
  return t;
}
