import type { GameEntry } from "@/types/generated/GameEntry";
import type { GameSource } from "@/types/generated/GameSource";
import type { GameSourceStatus } from "@/types/generated/GameSourceStatus";

/** Library filtering, search and sorting (pure, unit-tested). */

export type SourceFilter = "all" | GameSource;
export type SortKey = "name" | "lastPlayed";

export interface LibraryQuery {
  search: string;
  source: SourceFilter;
  sort: SortKey;
  /** Also list the entries the user hid (they stay marked). */
  showHidden: boolean;
}

export const DEFAULT_QUERY: LibraryQuery = { search: "", source: "all", sort: "name", showHidden: false };

/** Lowercase without diacritics, so "pokemon" finds "Pokémon". */
export function foldText(s: string): string {
  return s
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .trim();
}

/** Every word of the query must appear in the title (any order). */
export function matchesSearch(title: string, search: string): boolean {
  const words = foldText(search).split(/\s+/).filter(Boolean);
  if (words.length === 0) return true;
  const hay = foldText(title);
  return words.every((w) => hay.includes(w));
}

export function filterLibrary(entries: readonly GameEntry[], q: LibraryQuery, locale: string): GameEntry[] {
  const collator = new Intl.Collator(locale, { sensitivity: "base", numeric: true });
  const byName = (a: GameEntry, b: GameEntry) => collator.compare(a.title, b.title) || a.id.localeCompare(b.id);
  const list = entries.filter(
    (e) => (q.showHidden || !e.hidden) && (q.source === "all" || e.source === q.source) && matchesSearch(e.title, q.search),
  );
  if (q.sort === "lastPlayed") {
    // Most recent first; never-played (unknown) after, by name.
    return list.sort((a, b) => {
      if (a.lastPlayed !== null && b.lastPlayed !== null) return b.lastPlayed - a.lastPlayed || byName(a, b);
      if (a.lastPlayed !== null) return -1;
      if (b.lastPlayed !== null) return 1;
      return byName(a, b);
    });
  }
  return list.sort(byName);
}

/** Source filter choices: "all" plus every source that has (visible or hidden) entries or was found. */
export function sourceOptions(sources: readonly GameSourceStatus[], entries: readonly GameEntry[]): SourceFilter[] {
  const present = new Set(entries.map((e) => e.source));
  const out: SourceFilter[] = ["all"];
  for (const s of sources) {
    if (present.has(s.source) && !out.includes(s.source)) out.push(s.source);
  }
  for (const source of present) if (!out.includes(source)) out.push(source);
  return out;
}

export const hiddenCount = (entries: readonly GameEntry[]): number => entries.filter((e) => e.hidden).length;
