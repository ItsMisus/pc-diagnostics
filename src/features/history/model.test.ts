import { describe, expect, it } from "vitest";
import { result } from "@/features/benchmarks/testing/fixtures";
import { compareCandidates, DEFAULT_SORT, filterHistory, kindsInHistory, matchesOutcome, nextSort, sortHistory } from "./model";

const runs = [
  result("a", { startedAt: 100 }),
  result("b", { startedAt: 300, kind: "memoryLatency" }),
  result("c", { startedAt: 200 }),
  result("d", { startedAt: 250, state: "cancelled", metrics: [] }),
];

describe("history", () => {
  it("filters by kind and sorts newest first", () => {
    expect(filterHistory(runs, "all").map((r) => r.runId)).toEqual(["b", "d", "c", "a"]);
    expect(filterHistory(runs, "cpuSingleCore").map((r) => r.runId)).toEqual(["d", "c", "a"]);
    expect(filterHistory(runs, "storageRandom")).toEqual([]);
    expect(filterHistory([], "all")).toEqual([]);
  });

  it("filters by outcome, combined with the kind", () => {
    const mixed = [...runs, result("e", { startedAt: 400, state: "failed", metrics: [] })];
    expect(filterHistory(mixed, "all", "completed").map((r) => r.runId)).toEqual(["b", "c", "a"]);
    expect(filterHistory(mixed, "all", "stopped").map((r) => r.runId)).toEqual(["e", "d"]);
    expect(filterHistory(mixed, "memoryLatency", "stopped")).toEqual([]);
    expect(matchesOutcome({ state: "cancelled" }, "all")).toBe(true);
    expect(matchesOutcome({ state: "completed" }, "stopped")).toBe(false);
  });

  it("lists the kinds present", () => {
    expect(kindsInHistory(runs)).toEqual(["cpuSingleCore", "memoryLatency"]);
  });

  it("offers only other completed runs of the same kind for comparison", () => {
    const current = runs[2]!;
    expect(compareCandidates(runs, current).map((r) => r.runId)).toEqual(["a"]);
  });
});

describe("history sort", () => {
  const rows = filterHistory(
    [
      result("a", { startedAt: 100, durationMs: 5000 }),
      result("b", { startedAt: 300, durationMs: 1000 }),
      result("c", { startedAt: 200, durationMs: 5000 }),
    ],
    "all",
  );
  const ids = (list: readonly { runId: string }[]) => list.map((r) => r.runId);

  it("keeps the newest-first order by default", () => {
    expect(ids(sortHistory(rows, DEFAULT_SORT))).toEqual(["b", "c", "a"]);
    expect(ids(sortHistory(rows, { key: "date", direction: "asc" }))).toEqual(["a", "c", "b"]);
  });

  it("sorts by duration, ties stay newest first", () => {
    expect(ids(sortHistory(rows, { key: "duration", direction: "desc" }))).toEqual(["c", "a", "b"]);
    expect(ids(sortHistory(rows, { key: "duration", direction: "asc" }))).toEqual(["b", "c", "a"]);
  });

  it("flips the active column and starts a new one descending", () => {
    expect(nextSort(DEFAULT_SORT, "date")).toEqual({ key: "date", direction: "asc" });
    expect(nextSort({ key: "date", direction: "asc" }, "date")).toEqual({ key: "date", direction: "desc" });
    expect(nextSort(DEFAULT_SORT, "duration")).toEqual({ key: "duration", direction: "desc" });
  });
});
