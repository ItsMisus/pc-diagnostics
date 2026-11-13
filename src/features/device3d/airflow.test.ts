import { describe, expect, it } from "vitest";
import { airflowBounds, airStreams, particleAt } from "./airflow";
import { frameFor, PRESETS } from "./presets";
import { DEVICE_KINDS } from "./types";

describe("airflow", () => {
  it("draws intake and exhaust with fans, only rising heat without", () => {
    for (const kind of DEVICE_KINDS) {
      const active = airStreams(kind, "active").map((s) => s.kind);
      expect(active).toContain("intake");
      expect(active).toContain("exhaust");
      const passive = airStreams(kind, "passive");
      expect(passive.every((s) => s.kind === "heat" && s.travel[1] > 0)).toBe(true);
    }
  });

  it("keeps every particle inside the bounds, growing in and fading out", () => {
    for (const kind of DEVICE_KINDS) {
      for (const mode of ["active", "passive"] as const) {
        const b = airflowBounds(kind, mode, PRESETS[kind].size);
        airStreams(kind, mode).forEach((s, si) => {
          for (let i = 0; i < s.count; i++) {
            for (const t of [0, 0.37, 1.9, 12.4]) {
              const p = particleAt(s, si, i, t);
              expect(p.scale).toBeGreaterThanOrEqual(0);
              expect(p.scale).toBeLessThanOrEqual(1);
              p.position.forEach((v, a) => {
                expect(v).toBeGreaterThanOrEqual((b[a] ?? 0) - 1e-9);
                expect(v).toBeLessThanOrEqual((b[a + 3] ?? 0) + 1e-9);
              });
            }
          }
        });
      }
    }
  });

  it("frames a full turn with the airflow no closer than the drag arc", () => {
    for (const kind of DEVICE_KINDS) {
      const arc = frameFor(kind, 4 / 3).distance;
      const turn = frameFor(kind, 4 / 3, { fullTurn: true, bounds: airflowBounds(kind, "active", PRESETS[kind].size) }).distance;
      expect(turn).toBeGreaterThanOrEqual(arc);
    }
  });
});
