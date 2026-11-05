import { create } from "zustand";
import type { ProcessRow } from "@/types/generated/ProcessRow";
import { DEFAULT_SORT, nextSort, type SortColumn, type SortState } from "./rows";

export type RankingMode = "instant" | "window";

/** UI-only state of the Processes page. Survives navigation away and back. */
interface ProcessViewState {
  mode: RankingMode;
  sort: SortState;
  query: string;
  /** Selected process instance (`ProcessRow.key`) and its last-seen row. */
  selected: ProcessRow | null;
  setMode: (mode: RankingMode) => void;
  sortBy: (column: SortColumn) => void;
  setQuery: (query: string) => void;
  select: (row: ProcessRow | null) => void;
}

export const useProcessView = create<ProcessViewState>((set, get) => ({
  mode: "instant",
  sort: DEFAULT_SORT,
  query: "",
  selected: null,
  setMode: (mode) => set({ mode }),
  sortBy: (column) => set({ sort: nextSort(get().sort, column) }),
  setQuery: (query) => set({ query }),
  select: (row) => set({ selected: row }),
}));

export const rankingKey = (mode: RankingMode) => (mode === "instant" ? "rankingInstant" : "rankingWindow");
