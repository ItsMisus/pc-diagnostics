import { createContext, useContext } from "react";
import { AdditiveBlending, DoubleSide, MeshBasicMaterial, MeshPhysicalMaterial, MeshStandardMaterial, type Material } from "three";
import type { StageTheme } from "@/features/device3d/types";

/**
 * Physically based materials of the cooling scenes, in the same family as the Overview device
 * (satin aluminium, graphite, bright fins), created once per canvas and freed on unmount.
 */
export interface CoolingMaterials {
  aluminium: MeshPhysicalMaterial;
  /** Fan frames, radiator tanks. */
  frame: MeshStandardMaterial;
  /** Fan blades: satin dark polymer. */
  blade: MeshPhysicalMaterial;
  hub: MeshPhysicalMaterial;
  fins: MeshStandardMaterial;
  copper: MeshPhysicalMaterial;
  /** Soft sleeved hoses. */
  hose: MeshStandardMaterial;
  /** Graphite thermal sheet inside the passive chassis. */
  graphite: MeshStandardMaterial;
  /** The rotor's motion disc: a faint translucent veil that thickens with speed. */
  blur: MeshBasicMaterial;
  /** Heat-flow glow of the passive chassis. */
  heat: MeshBasicMaterial;
  dispose(): void;
}

export function createCoolingMaterials(theme: StageTheme): CoolingMaterials {
  const dark = theme === "dark";
  const m = {
    aluminium: new MeshPhysicalMaterial({ color: 0xc9ccd1, metalness: 1, roughness: 0.3, clearcoat: 0.1, clearcoatRoughness: 0.4 }),
    frame: new MeshStandardMaterial({ color: 0x1b1d21, metalness: 0.25, roughness: 0.62 }),
    blade: new MeshPhysicalMaterial({ color: 0x2a2d33, metalness: 0.2, roughness: 0.42, clearcoat: 0.4, clearcoatRoughness: 0.3, side: DoubleSide }),
    hub: new MeshPhysicalMaterial({ color: 0x34373d, metalness: 0.9, roughness: 0.34, clearcoat: 0.5 }),
    fins: new MeshStandardMaterial({ color: 0xbfc2c8, metalness: 1, roughness: 0.28 }),
    copper: new MeshPhysicalMaterial({ color: 0xc8794a, metalness: 1, roughness: 0.26, clearcoat: 0.3 }),
    hose: new MeshStandardMaterial({ color: 0x111214, metalness: 0, roughness: 0.85 }),
    graphite: new MeshStandardMaterial({ color: 0x2b2d31, metalness: 0.5, roughness: 0.5 }),
    blur: new MeshBasicMaterial({ color: dark ? 0x5a5f68 : 0x3a3e45, transparent: true, opacity: 0, depthWrite: false, side: DoubleSide }),
    heat: new MeshBasicMaterial({ color: 0xff8a3d, transparent: true, opacity: 0.3, depthWrite: false, blending: AdditiveBlending, toneMapped: false, side: DoubleSide }),
  };
  const all: Material[] = Object.values(m);
  return {
    ...m,
    dispose() {
      for (const mat of all) mat.dispose();
    },
  };
}

export const CoolingMaterialsContext = createContext<CoolingMaterials | null>(null);

export function useCoolingMaterials(): CoolingMaterials {
  const m = useContext(CoolingMaterialsContext);
  if (!m) throw new Error("useCoolingMaterials outside the cooling scene");
  return m;
}
