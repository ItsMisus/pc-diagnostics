import { describe, expect, it } from "vitest";
import type { FormFactor } from "@/types/generated/FormFactor";
import type { Metric } from "@/types/generated/Metric";
import { arrangePanels, formFactorIcon } from "./layout";

describe("arrangePanels", () => {
  it("pairs two half-width panels and keeps the full one after them", () => {
    expect(
      arrangePanels([
        { key: "cpu", half: true },
        { key: "gpu", half: true },
        { key: "memory", half: false },
      ]),
    ).toEqual([
      { key: "cpu", span: "half" },
      { key: "gpu", span: "half" },
      { key: "memory", span: "full" },
    ]);
  });

  it("moves a full-width panel after the halves so no cell stays empty", () => {
    expect(
      arrangePanels([
        { key: "cpu", half: true },
        { key: "gpu", half: false },
        { key: "memory", half: true },
      ]),
    ).toEqual([
      { key: "cpu", span: "half" },
      { key: "memory", span: "half" },
      { key: "gpu", span: "full" },
    ]);
  });

  it("widens the last half-width panel when their number is odd", () => {
    expect(
      arrangePanels([
        { key: "cpu", half: true },
        { key: "gpu", half: false },
        { key: "memory", half: false },
      ]),
    ).toEqual([
      { key: "cpu", span: "full" },
      { key: "gpu", span: "full" },
      { key: "memory", span: "full" },
    ]);
    expect(arrangePanels([{ key: "a", half: true }, { key: "b", half: true }, { key: "c", half: true }]).map((s) => s.span)).toEqual([
      "half",
      "half",
      "full",
    ]);
  });

  it("handles an empty list", () => {
    expect(arrangePanels([])).toEqual([]);
  });
});

describe("formFactorIcon", () => {
  const available = (value: FormFactor): Metric<FormFactor> => ({ status: "available", value, unit: "none", source: "ioKit", at: 0 });

  it("maps each form factor to its glyph", () => {
    expect(formFactorIcon(available("desktop"))).toBe("desktop");
    expect(formFactorIcon(available("laptop"))).toBe("laptop");
    expect(formFactorIcon(available("mini"))).toBe("miniPc");
    expect(formFactorIcon(available("allInOne"))).toBe("display");
    expect(formFactorIcon(available("other"))).toBe("hardware");
  });

  it("uses the generic glyph when the form factor is unknown", () => {
    expect(formFactorIcon({ status: "unavailable", reason: { kind: "inconclusive", detail: "x" } })).toBe("hardware");
  });
});
