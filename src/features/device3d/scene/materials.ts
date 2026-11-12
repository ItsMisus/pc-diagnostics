import { createContext, useContext } from "react";
import { DoubleSide, MeshBasicMaterial, MeshPhysicalMaterial, MeshStandardMaterial, type Material, type Texture } from "three";
import type { StageTheme } from "../types";
import { perforationTexture, wallpaperTexture } from "./textures";

/**
 * The scene's physically based materials, created once per scene and shared by every model
 * (so a model swap allocates no new shader programs). `dispose()` frees them and their
 * textures when the scene unmounts.
 */
export interface Materials {
  /** Satin silver aluminium (laptop, mini PC). */
  aluminium: MeshPhysicalMaterial;
  /** Dark anodized aluminium (tower panels, accents). */
  graphite: MeshPhysicalMaterial;
  /** Deeper anodized finish (tower back, PSU shroud). */
  graphiteDeep: MeshPhysicalMaterial;
  /** Cooler fins, bright machined metal. */
  fins: MeshStandardMaterial;
  /** Matte interior parts. */
  interior: MeshStandardMaterial;
  /** Motherboard. */
  pcb: MeshStandardMaterial;
  rubber: MeshStandardMaterial;
  /** Port openings. */
  hole: MeshStandardMaterial;
  /** Port tongues and pins. */
  tongue: MeshStandardMaterial;
  /** Glossy black glass (bezel, camera). */
  gloss: MeshPhysicalMaterial;
  /** Laptop display: emissive wallpaper under a clear coat. */
  screen: MeshPhysicalMaterial;
  /** Laptop keycaps. */
  keycap: MeshStandardMaterial;
  /** Keyboard deck behind the keys. */
  deck: MeshStandardMaterial;
  trackpad: MeshPhysicalMaterial;
  /** Tempered glass side panel (the only transmissive material). */
  glass: MeshPhysicalMaterial;
  /** Black ceramic frit along the glass edge. */
  frit: MeshStandardMaterial;
  /** Front intake: perforated sheet. */
  grille: MeshStandardMaterial;
  /** Power button / status light: soft, not a glow. */
  led: MeshBasicMaterial;
  /** Interior light strip. */
  strip: MeshBasicMaterial;
  setTheme(theme: StageTheme): void;
  dispose(): void;
}

export function createMaterials(theme: StageTheme): Materials {
  const textures: Texture[] = [];
  const grilleMap = perforationTexture(26, 30);
  textures.push(grilleMap);
  let wallpaper = wallpaperTexture(theme);

  const m = {
    aluminium: new MeshPhysicalMaterial({ color: 0xc9ccd1, metalness: 1, roughness: 0.32, clearcoat: 0.08, clearcoatRoughness: 0.4 }),
    graphite: new MeshPhysicalMaterial({ color: 0x34373d, metalness: 0.9, roughness: 0.4, clearcoat: 0.3, clearcoatRoughness: 0.32 }),
    graphiteDeep: new MeshPhysicalMaterial({ color: 0x202226, metalness: 0.75, roughness: 0.5, clearcoat: 0.15, clearcoatRoughness: 0.45 }),
    fins: new MeshStandardMaterial({ color: 0xb9bcc2, metalness: 1, roughness: 0.3 }),
    interior: new MeshStandardMaterial({ color: 0x15161a, metalness: 0.3, roughness: 0.72 }),
    pcb: new MeshStandardMaterial({ color: 0x1a1d22, metalness: 0.4, roughness: 0.58 }),
    rubber: new MeshStandardMaterial({ color: 0x0c0c0d, metalness: 0, roughness: 0.95 }),
    hole: new MeshStandardMaterial({ color: 0x050506, metalness: 0, roughness: 0.8 }),
    tongue: new MeshStandardMaterial({ color: 0x2e3137, metalness: 0.4, roughness: 0.5 }),
    gloss: new MeshPhysicalMaterial({ color: 0x060607, metalness: 0.1, roughness: 0.12, clearcoat: 1, clearcoatRoughness: 0.04 }),
    screen: new MeshPhysicalMaterial({
      color: 0x000000,
      emissive: 0xffffff,
      emissiveMap: wallpaper,
      emissiveIntensity: theme === "dark" ? 0.85 : 1,
      metalness: 0,
      roughness: 0.06,
      clearcoat: 1,
      clearcoatRoughness: 0.03,
    }),
    keycap: new MeshStandardMaterial({ color: 0x17181b, metalness: 0.05, roughness: 0.58 }),
    deck: new MeshStandardMaterial({ color: 0x0e0f11, metalness: 0.2, roughness: 0.7 }),
    trackpad: new MeshPhysicalMaterial({ color: 0xbec1c6, metalness: 0.65, roughness: 0.24, clearcoat: 0.6, clearcoatRoughness: 0.12 }),
    glass: new MeshPhysicalMaterial({
      color: 0xd6dbe2,
      metalness: 0,
      roughness: 0.04,
      transmission: 1,
      thickness: 0.04,
      ior: 1.5,
      attenuationColor: 0x6b7480,
      attenuationDistance: 0.5,
      specularIntensity: 1,
    }),
    frit: new MeshStandardMaterial({ color: 0x08090a, metalness: 0, roughness: 0.35, side: DoubleSide }),
    grille: new MeshStandardMaterial({ color: 0x4a4e55, map: grilleMap, metalness: 0.7, roughness: 0.45 }),
    led: new MeshBasicMaterial({ color: 0xcfe0ff, toneMapped: false }),
    strip: new MeshBasicMaterial({ color: 0xe7eeff, toneMapped: false }),
  };
  const all: Material[] = Object.values(m);

  return {
    ...m,
    setTheme(next) {
      const old = wallpaper;
      wallpaper = wallpaperTexture(next);
      m.screen.emissiveMap = wallpaper;
      m.screen.emissiveIntensity = next === "dark" ? 0.85 : 1;
      m.screen.needsUpdate = true;
      old.dispose();
    },
    dispose() {
      for (const mat of all) mat.dispose();
      for (const t of textures) t.dispose();
      wallpaper.dispose();
    },
  };
}

export const MaterialsContext = createContext<Materials | null>(null);

export function useMaterials(): Materials {
  const m = useContext(MaterialsContext);
  if (!m) throw new Error("useMaterials outside the device scene");
  return m;
}
