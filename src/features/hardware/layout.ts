import type { IconName } from "@/components/common/Icon";
import type { FormFactor } from "@/types/generated/FormFactor";
import type { Metric } from "@/types/generated/Metric";

/** A component panel on the Hardware page and whether its content fits half the width. */
export interface PanelSlot<K extends string = string> {
  key: K;
  /** True when the panel reads well at half the width (short spec list, no wide table). */
  half: boolean;
}

/**
 * Places the component panels in a two-column grid without holes: the half-width panels
 * come first in their given order, paired two by two; the full-width ones follow. With an
 * odd number of half-width panels the last one takes the whole row, so no panel sits next
 * to an empty cell. Reading order equals visual order (no `grid-flow-dense`).
 */
export function arrangePanels<K extends string>(slots: readonly PanelSlot<K>[]): { key: K; span: "half" | "full" }[] {
  const halves = slots.filter((s) => s.half);
  const fulls = slots.filter((s) => !s.half);
  const odd = halves.length % 2 === 1;
  return [
    ...halves.map((s, i) => ({ key: s.key, span: odd && i === halves.length - 1 ? ("full" as const) : ("half" as const) })),
    ...fulls.map((s) => ({ key: s.key, span: "full" as const })),
  ];
}

/** Glyph of a form factor; the generic hardware glyph when it is not known. */
export function formFactorIcon(formFactor: Metric<FormFactor>): IconName {
  if (formFactor.status === "unavailable") return "hardware";
  switch (formFactor.value) {
    case "desktop":
      return "desktop";
    case "laptop":
      return "laptop";
    case "mini":
      return "miniPc";
    case "allInOne":
      return "display";
    case "other":
      return "hardware";
  }
}
