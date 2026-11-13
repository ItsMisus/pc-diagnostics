import type { IconName } from "@/components/common/Icon";
import type { ControlDomainId } from "@/types/generated/ControlDomainId";
import type { GameEntry } from "@/types/generated/GameEntry";
import type { GameProfile } from "@/types/generated/GameProfile";
import type { GamingState } from "@/types/generated/GamingState";
import type { ProfileRef } from "@/types/generated/ProfileRef";
import type { SourceFilter } from "./library";

/** View-model helpers over `GamingState` (pure, unit-tested). */

export const DOMAINS: readonly ControlDomainId[] = ["performance", "cooling", "lighting"];

export const DOMAIN_ICON: Record<ControlDomainId, IconName> = {
  performance: "bolt",
  cooling: "fan",
  lighting: "bulb",
};

export function profileFor(state: GamingState, gameId: string): GameProfile {
  return (
    state.profiles.find((p) => p.gameId === gameId) ?? { gameId, enabled: false, performance: null, cooling: null, lighting: null }
  );
}

export function linkedId(profile: GameProfile, domain: ControlDomainId): string | null {
  return domain === "performance" ? profile.performance : domain === "cooling" ? profile.cooling : profile.lighting;
}

export function withLink(profile: GameProfile, domain: ControlDomainId, id: string | null): GameProfile {
  return { ...profile, [domain]: id };
}

export function linkCount(profile: GameProfile): number {
  return DOMAINS.filter((d) => linkedId(profile, d) !== null).length;
}

/** "active": enabled with at least one link; "off": links kept but disabled; "none". */
export type ProfileIndicator = "active" | "off" | "none";

export function profileIndicator(profile: GameProfile): ProfileIndicator {
  if (linkCount(profile) === 0) return "none";
  return profile.enabled ? "active" : "off";
}

export interface DomainChoice {
  domain: ControlDomainId;
  /** The center is connected to Gaming in this build. */
  connected: boolean;
  /** Profiles that can be applied here. */
  options: ProfileRef[];
  /** The linked id (may point to a profile that is gone or not applicable). */
  selected: string | null;
  /** `selected` is not among `options` any more. */
  stale: boolean;
}

export function domainChoices(state: GamingState, profile: GameProfile): DomainChoice[] {
  return DOMAINS.map((domain) => {
    const options = state.linkable.filter((r) => r.domain === domain && r.applicable);
    const selected = linkedId(profile, domain);
    return {
      domain,
      connected: state.domains.includes(domain),
      options,
      selected,
      stale: selected !== null && !options.some((o) => o.id === selected),
    };
  });
}

/** Whether "Gioca con profilo" has anything to apply. */
export const canPlayWithProfile = (profile: GameProfile): boolean => profileIndicator(profile) === "active";

export function sessionEntry(state: GamingState): GameEntry | null {
  const id = state.gameMode.session?.gameId;
  return id ? (state.library.find((e) => e.id === id) ?? null) : null;
}

/** The game the hero features: the one being played, else the most recently played visible one with art. */
export function featuredGame(state: GamingState): GameEntry | null {
  const active = sessionEntry(state);
  if (active) return active;
  const visible = state.library.filter((e) => !e.hidden);
  const played = visible.filter((e) => e.lastPlayed !== null).sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0));
  return played[0] ?? visible.find((e) => e.hasHero) ?? visible[0] ?? null;
}

/** Why the grid is empty: a missing launcher, an empty library, or a search with no result. */
export type EmptyReason = { kind: "sourceMissing"; source: Exclude<SourceFilter, "all"> } | { kind: "noGames" } | { kind: "noMatches" };

export function emptyReason(state: GamingState, source: SourceFilter, filteredCount: number, searching: boolean): EmptyReason | null {
  if (filteredCount > 0) return null;
  if (source !== "all") {
    const status = state.sources.find((s) => s.source === source);
    if (status && !status.found && source !== "manual") return { kind: "sourceMissing", source };
  }
  if (searching) return { kind: "noMatches" };
  return state.library.some((e) => !e.hidden) ? { kind: "noMatches" } : { kind: "noGames" };
}

/** Results of a session: applied count and the domains that could not apply. */
export function sessionSummary(state: GamingState): { applied: number; failed: number; total: number } {
  const domains = state.gameMode.session?.domains ?? [];
  const applied = domains.filter((d) => d.applied).length;
  return { applied, failed: domains.length - applied, total: domains.length };
}
