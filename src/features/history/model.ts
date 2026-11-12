import type { BenchmarkKind } from "@/types/generated/BenchmarkKind";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";

export type KindFilter = BenchmarkKind | "all";

/** Outcome filter: every run, only completed ones, or the ones that stopped early (cancelled or failed). */
export type OutcomeFilter = "all" | "completed" | "stopped";

export function matchesOutcome(result: Pick<BenchmarkResult, "state">, filter: OutcomeFilter): boolean {
  if (filter === "all") return true;
  return filter === "completed" ? result.state === "completed" : result.state !== "completed";
}

/** Results of the selected kind (and outcome), newest first (stable for equal timestamps). */
export function filterHistory(
  results: readonly BenchmarkResult[],
  filter: KindFilter,
  outcome: OutcomeFilter = "all",
): BenchmarkResult[] {
  return results
    .filter((r) => (filter === "all" || r.kind === filter) && matchesOutcome(r, outcome))
    .map((r, i) => ({ r, i }))
    .sort((a, b) => b.r.startedAt - a.r.startedAt || a.i - b.i)
    .map((x) => x.r);
}

/** Kinds present in the history, in first-seen order: the filter only offers these. */
export function kindsInHistory(results: readonly BenchmarkResult[]): BenchmarkKind[] {
  const seen: BenchmarkKind[] = [];
  for (const r of results) if (!seen.includes(r.kind)) seen.push(r.kind);
  return seen;
}

export type HistorySortKey = "date" | "duration";
export interface HistorySort {
  key: HistorySortKey;
  direction: "asc" | "desc";
}

/** The order the backend returns and `filterHistory` produces: newest first. */
export const DEFAULT_SORT: HistorySort = { key: "date", direction: "desc" };

/** Clicking the active column flips it; another column starts from its most useful end. */
export function nextSort(current: HistorySort, key: HistorySortKey): HistorySort {
  if (current.key === key) return { key, direction: current.direction === "asc" ? "desc" : "asc" };
  return { key, direction: "desc" };
}

/**
 * Display order of the table. Stable: ties keep the incoming (newest-first) order, so the
 * default sort returns the rows unchanged.
 */
export function sortHistory(rows: readonly BenchmarkResult[], sort: HistorySort): BenchmarkResult[] {
  const sign = sort.direction === "asc" ? 1 : -1;
  const value = (r: BenchmarkResult) => (sort.key === "date" ? r.startedAt : r.durationMs);
  return rows
    .map((r, i) => ({ r, i }))
    .sort((a, b) => sign * (value(a.r) - value(b.r)) || a.i - b.i)
    .map((x) => x.r);
}

/** Other completed runs of the same kind that `current` can be compared with, newest first. */
export function compareCandidates(results: readonly BenchmarkResult[], current: BenchmarkResult): BenchmarkResult[] {
  return filterHistory(results, current.kind).filter((r) => r.runId !== current.runId && r.state === "completed");
}
