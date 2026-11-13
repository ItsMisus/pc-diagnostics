import { describe, expect, it } from "vitest";
import type { GameEntry } from "@/types/generated/GameEntry";
import type { GamingState } from "@/types/generated/GamingState";
import { coverBackground, coverMonogram, coverPalette, hashTitle } from "./cover";
import { DEFAULT_QUERY, filterLibrary, foldText, hiddenCount, matchesSearch, sourceOptions } from "./library";
import {
  canPlayWithProfile,
  domainChoices,
  emptyReason,
  featuredGame,
  linkCount,
  profileFor,
  profileIndicator,
  sessionSummary,
  withLink,
} from "./model";

function entry(id: string, title: string, over: Partial<GameEntry> = {}): GameEntry {
  return {
    id,
    title,
    source: "steam",
    installPath: null,
    sizeBytes: null,
    lastPlayed: null,
    hasCover: false,
    hasHero: false,
    hasLogo: false,
    hasIcon: false,
    hidden: false,
    launch: { state: "supported", reason: null },
    sourceId: id,
    ...over,
  };
}

const library: GameEntry[] = [
  entry("steam:1", "Geometry Dash", { lastPlayed: 1000, hasHero: true }),
  entry("macApp:mc", "Minecraft Launcher", { source: "macApp" }),
  entry("macApp:rb", "Roblox", { source: "macApp", lastPlayed: 5000 }),
  entry("steam:2", "Pokémon Arena", { hidden: true }),
  entry("steam:3", "game 10"),
  entry("steam:4", "game 9"),
];

function state(over: Partial<GamingState> = {}): GamingState {
  return {
    library,
    sources: [
      { source: "steam", support: { state: "supported", reason: null }, found: true, count: 3, location: null },
      { source: "epic", support: { state: "supported", reason: null }, found: false, count: 0, location: null },
      { source: "macApp", support: { state: "supported", reason: null }, found: true, count: 2, location: null },
      { source: "manual", support: { state: "supported", reason: null }, found: false, count: 0, location: null },
    ],
    profiles: [{ gameId: "steam:1", enabled: true, performance: "perf.max", cooling: null, lighting: "gone" }],
    linkable: [
      { domain: "performance", id: "perf.max", name: "Massime", builtin: true, applicable: true },
      { domain: "performance", id: "perf.na", name: "N/A", builtin: true, applicable: false },
      { domain: "lighting", id: "rgb", name: "RGB", builtin: false, applicable: true },
    ],
    domains: ["performance", "lighting"],
    gameMode: { enabled: false, session: null },
    scannedAt: 0,
    ...over,
  };
}

describe("library search, filter and sort", () => {
  it("folds case and diacritics, every word must match", () => {
    expect(foldText("  Pokémon ")).toBe("pokemon");
    expect(matchesSearch("Pokémon Arena", "arena POKE")).toBe(true);
    expect(matchesSearch("Geometry Dash", "dash x")).toBe(false);
    expect(matchesSearch("anything", "   ")).toBe(true);
  });

  it("hides hidden entries unless asked, filters by source", () => {
    const ids = (q = DEFAULT_QUERY) => filterLibrary(library, q, "it-IT").map((e) => e.id);
    expect(ids()).not.toContain("steam:2");
    expect(ids({ ...DEFAULT_QUERY, showHidden: true })).toContain("steam:2");
    expect(ids({ ...DEFAULT_QUERY, source: "macApp" })).toEqual(["macApp:mc", "macApp:rb"]);
    expect(ids({ ...DEFAULT_QUERY, search: "pokemon", showHidden: true })).toEqual(["steam:2"]);
  });

  it("sorts by name (numeric) and by last played with unknown last", () => {
    expect(filterLibrary(library, DEFAULT_QUERY, "en-US").map((e) => e.title)).toEqual([
      "game 9",
      "game 10",
      "Geometry Dash",
      "Minecraft Launcher",
      "Roblox",
    ]);
    expect(filterLibrary(library, { ...DEFAULT_QUERY, sort: "lastPlayed" }, "en-US").map((e) => e.id)).toEqual([
      "macApp:rb",
      "steam:1",
      "steam:4",
      "steam:3",
      "macApp:mc",
    ]);
  });

  it("offers only sources that have entries", () => {
    expect(sourceOptions(state().sources, library)).toEqual(["all", "steam", "macApp"]);
    expect(hiddenCount(library)).toBe(1);
  });
});

describe("generated cover", () => {
  it("is stable per title and differs between titles", () => {
    expect(hashTitle("Roblox")).toBe(hashTitle("Roblox"));
    expect(coverPalette("Roblox")).toEqual(coverPalette("  roblox "));
    expect(coverPalette("Roblox").hue).not.toBe(coverPalette("Geometry Dash").hue);
    expect(coverBackground("Roblox")).toMatch(/^linear-gradient\(\d+deg, oklch\(.+\), oklch\(.+\)\)$/);
  });

  it("monograms take the first letters of two words", () => {
    expect(coverMonogram("Geometry Dash")).toBe("GD");
    expect(coverMonogram("roblox")).toBe("R");
    expect(coverMonogram("Ōkami: HD")).toBe("ŌH");
    expect(coverMonogram("!!!")).toBe("?");
  });
});

describe("state mapping", () => {
  it("finds or defaults a game profile and its indicator", () => {
    const s = state();
    const p = profileFor(s, "steam:1");
    expect(linkCount(p)).toBe(2);
    expect(profileIndicator(p)).toBe("active");
    expect(canPlayWithProfile(p)).toBe(true);
    expect(profileIndicator({ ...p, enabled: false })).toBe("off");
    const none = profileFor(s, "steam:3");
    expect(none).toEqual({ gameId: "steam:3", enabled: false, performance: null, cooling: null, lighting: null });
    expect(profileIndicator(none)).toBe("none");
    expect(withLink(none, "cooling", "quiet").cooling).toBe("quiet");
  });

  it("builds one choice per center with applicable options and stale links", () => {
    const s = state();
    const [perf, cool, light] = domainChoices(s, profileFor(s, "steam:1"));
    expect(perf?.options.map((o) => o.id)).toEqual(["perf.max"]);
    expect(perf?.stale).toBe(false);
    expect(cool?.connected).toBe(false);
    expect(light?.selected).toBe("gone");
    expect(light?.stale).toBe(true);
  });

  it("features the session game, else the last played", () => {
    expect(featuredGame(state())?.id).toBe("macApp:rb");
    const playing = state({
      gameMode: {
        enabled: true,
        session: {
          gameId: "steam:1",
          title: "Geometry Dash",
          since: 1,
          trigger: "detected",
          detected: true,
          domains: [
            { domain: "performance", profileId: "perf.max", profileName: "Massime", applied: true, support: { state: "supported", reason: null } },
            { domain: "lighting", profileId: "gone", profileName: null, applied: false, support: { state: "unsupported", reason: "x" } },
          ],
        },
      },
    });
    expect(featuredGame(playing)?.id).toBe("steam:1");
    expect(sessionSummary(playing)).toEqual({ applied: 1, failed: 1, total: 2 });
    expect(featuredGame(state({ library: [] }))).toBeNull();
  });

  it("explains an empty grid", () => {
    const s = state();
    expect(emptyReason(s, "epic", 0, false)).toEqual({ kind: "sourceMissing", source: "epic" });
    expect(emptyReason(s, "all", 0, true)).toEqual({ kind: "noMatches" });
    expect(emptyReason(state({ library: [] }), "all", 0, false)).toEqual({ kind: "noGames" });
    expect(emptyReason(s, "all", 3, false)).toBeNull();
  });
});
