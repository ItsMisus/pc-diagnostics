import type { Tone } from "@/components/common/StatusBadge";
import type { ComponentKind } from "@/types/generated/ComponentKind";
import type { Confidence } from "@/types/generated/Confidence";
import type { CoverageItem } from "@/types/generated/CoverageItem";
import type { CoverageStatus } from "@/types/generated/CoverageStatus";
import type { Finding } from "@/types/generated/Finding";
import type { Platform } from "@/types/generated/Platform";
import type { FindingKind } from "@/types/generated/FindingKind";
import type { Severity } from "@/types/generated/Severity";
import { translatorFor } from "@/i18n";

/**
 * Labels are looked up in the "diagnostics" namespace at call time, in the current language.
 * Components that render them also call `useT("diagnostics")`, so they re-render on a
 * language change.
 */

export const SEVERITY_ORDER: readonly Severity[] = ["critical", "warning", "info"];

const dt = () => translatorFor("diagnostics");

export function severityLabel(s: Severity): string {
  return dt()(`severity_${s}`);
}

export function severityGroupLabel(s: Severity): string {
  return dt()(`severityGroup_${s}`);
}

export function severityTone(s: Severity): Tone {
  switch (s) {
    case "critical":
      return "bad";
    case "warning":
      return "warn";
    case "info":
      return "neutral";
  }
}

/** Epistemic status: each kind has its own wording, never interchangeable. */
export function findingKindLabel(k: FindingKind): string {
  return dt()(`kind_${k}`);
}

export function findingKindHint(k: FindingKind): string {
  return dt()(`kindHint_${k}`);
}

export function confidenceLabel(c: Confidence): string {
  return dt()(`confidence_${c}`);
}

export function componentKindLabel(c: ComponentKind): string {
  return dt()(`component_${c}`);
}

export function coverageLabel(s: CoverageStatus): string {
  return dt()(`coverage_${s}`);
}

export function coverageTone(s: CoverageStatus): Tone {
  // Coverage is about completeness, not health: never green.
  return s === "evaluated" ? "neutral" : "warn";
}

const SEVERITY_RANK: Record<Severity, number> = { critical: 0, warning: 1, info: 2 };
const KIND_RANK: Record<FindingKind, number> = { confirmed: 0, hypothesis: 1, observation: 2 };
const CONFIDENCE_RANK: Record<Confidence, number> = { high: 0, medium: 1, low: 2 };

/** Most severe first; then confirmed before hypotheses before observations; then confidence. Stable. */
export function sortFindings(findings: readonly Finding[]): Finding[] {
  return findings
    .map((f, i) => ({ f, i }))
    .sort(
      (a, b) =>
        SEVERITY_RANK[a.f.severity] - SEVERITY_RANK[b.f.severity] ||
        KIND_RANK[a.f.kind] - KIND_RANK[b.f.kind] ||
        CONFIDENCE_RANK[a.f.confidence] - CONFIDENCE_RANK[b.f.confidence] ||
        a.i - b.i,
    )
    .map((x) => x.f);
}

export interface FindingGroup {
  severity: Severity;
  findings: Finding[];
}

/** Non-empty groups in severity order. */
export function groupFindings(findings: readonly Finding[]): FindingGroup[] {
  const sorted = sortFindings(findings);
  return SEVERITY_ORDER.map((severity) => ({ severity, findings: sorted.filter((f) => f.severity === severity) })).filter(
    (g) => g.findings.length > 0,
  );
}

export function severityCounts(findings: readonly Finding[]): Record<Severity, number> {
  const counts: Record<Severity, number> = { critical: 0, warning: 0, info: 0 };
  for (const f of findings) counts[f.severity] += 1;
  return counts;
}

export function topFindings(findings: readonly Finding[], n = 3): Finding[] {
  return sortFindings(findings).slice(0, n);
}

export interface CoverageSummary {
  evaluated: number;
  partial: number;
  notEvaluated: number;
  total: number;
  /** Every component fully evaluated. */
  complete: boolean;
  /** Partial and not-evaluated items, not-evaluated first. */
  gaps: CoverageItem[];
}

/**
 * The coverage items that apply on `platform`. The device and driver inventory exists only on
 * Windows; elsewhere the backend reports drivers as "not evaluated" (no inventory to read),
 * which is a property of the platform, not a gap of this check, so the item is left out like
 * the Drivers page itself. Every other item, and drivers in any other status, is kept.
 */
export function platformCoverage(coverage: readonly CoverageItem[], platform: Platform): readonly CoverageItem[] {
  if (platform === "windows") return coverage;
  return coverage.filter((c) => !(c.component === "drivers" && c.status === "notEvaluated"));
}

export function coverageSummary(coverage: readonly CoverageItem[]): CoverageSummary {
  const evaluated = coverage.filter((c) => c.status === "evaluated").length;
  const partial = coverage.filter((c) => c.status === "partial").length;
  const notEvaluated = coverage.filter((c) => c.status === "notEvaluated").length;
  const gaps = [
    ...coverage.filter((c) => c.status === "notEvaluated"),
    ...coverage.filter((c) => c.status === "partial"),
  ];
  return { evaluated, partial, notEvaluated, total: coverage.length, complete: coverage.length > 0 && gaps.length === 0, gaps };
}

/** One sentence that never claims "all good". */
export function coverageSentence(s: CoverageSummary): string {
  const t = dt();
  if (s.total === 0) return t("coverageNone");
  const parts = [t.plural("coverageEvaluated", s.total, { evaluated: s.evaluated })];
  if (s.partial > 0) parts.push(t("coveragePartialCount", { count: s.partial }));
  if (s.notEvaluated > 0) parts.push(t("coverageNotEvaluatedCount", { count: s.notEvaluated }));
  return `${parts.join(", ")}.`;
}

/** "Nessun problema rilevato dalle regole disponibili", in the current language. */
export function noFindingsCopy(): string {
  return dt()("noFindings");
}
