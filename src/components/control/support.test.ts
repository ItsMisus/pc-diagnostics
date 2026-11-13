import { describe, expect, it } from "vitest";
import { bySupport } from "./support";

describe("bySupport", () => {
  it("puts available first, then unsupported, then read-only, keeping order within a group", () => {
    const items = [
      { id: "a", state: "readOnly" },
      { id: "b", state: "supported" },
      { id: "c", state: "unsupported" },
      { id: "d", state: "supported" },
      { id: "e", state: "readOnly" },
    ] as const;
    const out = bySupport(items, (x) => ({ state: x.state, reason: null }));
    expect(out.map((x) => x.id)).toEqual(["b", "d", "c", "a", "e"]);
  });
});
