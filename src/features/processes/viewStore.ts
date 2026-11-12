import { create } from "zustand";
import type { ProcessRow } from "@/types/generated/ProcessRow";
import { type CategoryFilter, DEFAULT_SORT, nextSort, type SortColumn, type SortState } from "./rows";

export type RankingMode = "instant" | "window";

/** UI-only state of the Processes page. Survives navigation away and back. */
interface ProcessViewState {
  mode: RankingMode;
  sort: SortState;
  query: string;
  category: CategoryFilter;
  /** Selected process instance (`ProcessRow.key`) and its last-seen row; opens the detail sheet. */
  selected: ProcessRow | null;
  setMode: (mode: RankingMode) => void;
  sortBy: (column: SortColumn) => void;
  setQuery: (query: string) => void;
  setCategory: (category: CategoryFilter) => void;
  /** Empties the search and shows every category again (the "no match" state's action). */
  clearFilters: () => void;
  select: (row: ProcessRow | null) => void;
}

export const useProcessView = create<ProcessViewState>((set, get) => ({
  mode: "instant",
  sort: DEFAULT_SORT,
  query: "",
  category: "all",
  selected: null,
  setMode: (mode) => set({ mode }),
  sortBy: (column) => set({ sort: nextSort(get().sort, column) }),
  setQuery: (query) => set({ query }),
  setCategory: (category) => set({ category }),
  clearFilters: () => set({ query: "", category: "all" }),
  select: (row) => set({ selected: row }),
}));

export const rankingKey = (mode: RankingMode) => (mode === "instant" ? "rankingInstant" : "rankingWindow");
