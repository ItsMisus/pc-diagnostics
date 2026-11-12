import { afterEach, describe, expect, it } from "vitest";
import { DEFAULT_SORT } from "./rows";
import { row } from "./testing/fixtures";
import { useProcessView } from "./viewStore";

const initial = useProcessView.getState();

describe("process view store", () => {
  afterEach(() => useProcessView.setState(initial, true));

  it("clears the search and the category together, keeping mode, sort and selection", () => {
    const s = useProcessView.getState();
    const picked = row("chrome");
    s.setMode("window");
    s.sortBy("cpu");
    s.setQuery("chr");
    s.setCategory("system");
    s.select(picked);
    useProcessView.getState().clearFilters();
    const after = useProcessView.getState();
    expect(after.query).toBe("");
    expect(after.category).toBe("all");
    expect(after.mode).toBe("window");
    expect(after.sort).toEqual({ column: "cpu", direction: "desc" });
    expect(after.selected).toBe(picked);
  });

  it("starts unfiltered, ranked by impact, with no sheet open", () => {
    expect(initial.category).toBe("all");
    expect(initial.query).toBe("");
    expect(initial.sort).toEqual(DEFAULT_SORT);
    expect(initial.selected).toBeNull();
  });
});
