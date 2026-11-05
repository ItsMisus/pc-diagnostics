import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "@/i18n";
import {
  categoryLabel,
  DEFAULT_SORT,
  filterRows,
  ioTotal,
  nextSort,
  sameRowDisplay,
  sortRows,
  weightShare,
  zeroBasedDomain,
} from "./rows";
import { denied, ok, row } from "./testing/fixtures";

const names = (rows: readonly { name: string }[]) => rows.map((r) => r.name);

describe("sortRows", () => {
  const a = row("alpha", { impact: 30, cpuPercent: ok(5) });
  const b = row("Beta", { impact: 50, cpuPercent: ok(5) });
  const c = row("gamma", { impact: 10, cpuPercent: ok(20) });
  const locked = row("locked", { accessible: false, impact: 99, cpuPercent: denied, workingSetBytes: denied });

  it("sorts numerically in both directions", () => {
    expect(names(sortRows([a, b, c], { column: "impact", direction: "desc" }))).toEqual(["Beta", "alpha", "gamma"]);
    expect(names(sortRows([a, b, c], { column: "impact", direction: "asc" }))).toEqual(["gamma", "alpha", "Beta"]);
  });

  it("is stable: ties keep the incoming (ranking) order in both directions", () => {
    expect(names(sortRows([b, a, c], { column: "cpu", direction: "desc" }))).toEqual(["gamma", "Beta", "alpha"]);
    expect(names(sortRows([b, a, c], { column: "cpu", direction: "asc" }))).toEqual(["Beta", "alpha", "gamma"]);
  });

  it("puts unavailable values last in both directions", () => {
    for (const direction of ["asc", "desc"] as const) {
      expect(sortRows([locked, a, b], { column: "cpu", direction }).at(-1)?.name).toBe("locked");
    }
  });

  it("ranks protected processes by their real score (metrics come from the kernel)", () => {
    const prot = row("protected", { accessible: false, impact: 99 });
    expect(sortRows([a, prot, b], { column: "impact", direction: "desc" })[0]?.name).toBe("protected");
  });

  it("sorts names case-insensitively", () => {
    expect(names(sortRows([c, b, a], { column: "name", direction: "asc" }))).toEqual(["alpha", "Beta", "gamma"]);
  });

  it("sorts I/O by read + write", () => {
    const r1 = row("r1", { ioReadBytesPerSec: ok(10, "bytesPerSecond"), ioWriteBytesPerSec: ok(100, "bytesPerSecond") });
    const r2 = row("r2", { ioReadBytesPerSec: ok(50, "bytesPerSecond"), ioWriteBytesPerSec: ok(0, "bytesPerSecond") });
    const r3 = row("r3", { ioReadBytesPerSec: denied });
    expect(names(sortRows([r2, r3, r1], { column: "io", direction: "desc" }))).toEqual(["r1", "r2", "r3"]);
  });

  it("does not mutate the input", () => {
    const input = [a, b, c];
    sortRows(input, DEFAULT_SORT);
    expect(names(input)).toEqual(["alpha", "Beta", "gamma"]);
  });
});

describe("nextSort", () => {
  it("toggles the active column and picks a sensible first direction", () => {
    expect(nextSort(DEFAULT_SORT, "impact")).toEqual({ column: "impact", direction: "asc" });
    expect(nextSort(DEFAULT_SORT, "name")).toEqual({ column: "name", direction: "asc" });
    expect(nextSort(DEFAULT_SORT, "ram")).toEqual({ column: "ram", direction: "desc" });
  });
});

describe("filterRows", () => {
  const rows = [row("chrome.exe"), row("Code.exe"), row("svchost.exe")];

  it("matches substrings case-insensitively and trims the query", () => {
    expect(names(filterRows(rows, "  CHRO "))).toEqual(["chrome.exe"]);
    expect(names(filterRows(rows, "co"))).toEqual(["Code.exe"]);
    expect(names(filterRows(rows, ".EXE"))).toHaveLength(3);
    expect(filterRows(rows, "zzz")).toEqual([]);
  });

  it("returns the same array for an empty query", () => {
    expect(filterRows(rows, "   ")).toBe(rows);
  });
});

describe("ioTotal", () => {
  it("sums both directions only when both are available", () => {
    const sum = ioTotal(row("x", { ioReadBytesPerSec: ok(3, "bytesPerSecond"), ioWriteBytesPerSec: ok(4, "bytesPerSecond") }));
    expect(sum.status === "available" && sum.value).toBe(7);
    expect(ioTotal(row("y", { ioWriteBytesPerSec: denied })).status).toBe("unavailable");
  });
});

describe("sameRowDisplay", () => {
  it("ignores new object identity but detects changed values", () => {
    expect(sameRowDisplay(row("a"), row("a"))).toBe(true);
    expect(sameRowDisplay(row("a"), row("a", { cpuPercent: ok(2) }))).toBe(false);
    expect(sameRowDisplay(row("a"), row("a", { impact: 11 }))).toBe(false);
    expect(sameRowDisplay(row("a"), row("a", { cpuPercent: denied }))).toBe(false);
  });
});

describe("chart and explanation helpers", () => {
  it("builds a zero-based domain with a nice ceiling and a floor", () => {
    expect(zeroBasedDomain([null, 30, 70], 10)).toEqual([0, 100]);
    expect(zeroBasedDomain([], 1024)).toEqual([0, 1024]);
    expect(zeroBasedDomain([null], 1)).toEqual([0, 1]);
  });

  it("expresses weights as shares of the total", () => {
    expect(weightShare(0.45, [0.45, 0.25, 0.3])).toBeCloseTo(45);
    expect(weightShare(1, [0, 0])).toBe(0);
  });

  it("labels categories in Italian", () => {
    expect(categoryLabel("service")).toBe("Servizio");
    expect(categoryLabel("unknown")).toBe("Non classificato");
  });
});

describe("English", () => {
  afterEach(() => setLang("it"));

  it("labels categories and still searches and sorts names case-insensitively", () => {
    setLang("en");
    expect(categoryLabel("service")).toBe("Service");
    expect(categoryLabel("unknown")).toBe("Unclassified");
    const rows = [row("chrome.exe"), row("Code.exe"), row("svchost.exe")];
    expect(names(filterRows(rows, "  CHRO "))).toEqual(["chrome.exe"]);
    expect(names(sortRows([row("beta"), row("Alpha")], { column: "name", direction: "asc" }))).toEqual(["Alpha", "beta"]);
  });
});
