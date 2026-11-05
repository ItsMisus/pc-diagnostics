import type { ComponentKind } from "@/types/generated/ComponentKind";
import type { CoverageItem } from "@/types/generated/CoverageItem";
import type { Finding } from "@/types/generated/Finding";
import type { FindingKind } from "@/types/generated/FindingKind";
import type { Severity } from "@/types/generated/Severity";
import { translatorFor } from "@/i18n";
import { severityCounts } from "./model";

/**
 * Plain-language helpers for the "Semplice" view of Diagnostics. Pure, unit-tested. Texts
 * come from the "diagnostics" namespace, looked up at call time in the current language.
 */

const dt = () => translatorFor("diagnostics");

/** Severity in everyday words. Colour semantics stay those of `severityTone`. */
export function simpleSeverityLabel(s: Severity): string {
  return dt()(`simpleSeverity_${s}`);
}

/**
 * Visible hint for the epistemic status. A hypothesis is never presented as certain;
 * observations and confirmed findings need no extra wording in the simple view.
 */
export function simpleKindHint(k: FindingKind): string | null {
  return k === "hypothesis" ? dt()("simpleHypothesis") : null;
}

export interface SimpleFindingText {
  headline: string;
  meaning: string;
  /** Empty when neither the simple text nor the recommendation say anything to do. */
  action: string;
  /** True when at least one field came from the technical text (older reports). */
  fallback: boolean;
}

/** The plain explanation, falling back field by field to title/description/recommendation. */
export function simpleFindingText(f: Finding): SimpleFindingText {
  const s = f.simple as Partial<Finding["simple"]> | undefined;
  const headline = s?.headline?.trim() ?? "";
  const meaning = s?.meaning?.trim() ?? "";
  const action = s?.action?.trim() ?? "";
  return {
    headline: headline || f.title,
    meaning: meaning || f.description,
    action: action || f.recommendation.trim(),
    fallback: !headline || !meaning || !action,
  };
}

/** Component names a non-technical user recognises, with the acronym kept in brackets. */
export function simpleComponentName(c: ComponentKind): string {
  return dt()(`simpleComponent_${c}`);
}

/**
 * One plain sentence per component, e.g.
 * "Processore (CPU): controllato solo in parte (temperatura non leggibile su questo PC)".
 * The note comes from the backend, already in the effective language.
 */
export function simpleCoverageSentence(item: CoverageItem): string {
  const t = dt();
  const c = item.component;
  const name = simpleComponentName(c);
  const note = item.note.trim();
  let line: string;
  let fallbackNote = "";
  switch (item.status) {
    case "evaluated":
      line = t("simpleCoverageEvaluated", { name, checked: t(`simpleChecked_${c}`) });
      break;
    case "partial":
      line = t("simpleCoveragePartial", { name, checked: t(`simpleChecked_${c}`) });
      fallbackNote = t("simplePartialDefaultNote");
      break;
    case "notEvaluated":
      line = t("simpleCoverageNotEvaluated", { name, notChecked: t(`simpleNotChecked_${c}`) });
      fallbackNote = t("simpleNotEvaluatedDefaultNote");
      break;
  }
  const shown = note || fallbackNote;
  return shown ? t("simpleCoverageWithNote", { line, note: shown }) : line;
}

/** "Non abbiamo trovato problemi." */
export function simpleNothingFound(): string {
  return dt()("simpleNothingFound");
}

/** "Nessun problema rilevato non significa che sia tutto perfetto." */
export function simpleNotAGuarantee(): string {
  return dt()("simpleNotAGuarantee");
}

/**
 * Short plain summary at the top of the simple view. Never says "all good": with nothing
 * found it states explicitly that this is not a guarantee, and mentions unchecked parts.
 */
export function simpleSummary(findings: readonly Finding[], coverage: readonly CoverageItem[]): string[] {
  const t = dt();
  const counts = severityCounts(findings);
  const gaps = coverage.filter((c) => c.status !== "evaluated").length;
  const lines: string[] = [];

  if (counts.critical === 0 && counts.warning === 0) {
    lines.push(counts.info > 0 ? t("simpleNothingFoundOnlyInfo") : t("simpleNothingFound"));
    lines.push(t("simpleNotAGuarantee"));
  } else {
    const important = counts.critical > 0 ? t.plural("simpleImportant", counts.critical) : "";
    const toCheck = counts.warning > 0 ? t.plural("simpleToCheck", counts.warning) : "";
    lines.push(
      important && toCheck ? t("simpleFoundBoth", { important, toCheck }) : t("simpleFound", { items: important || toCheck }),
    );
    if (counts.info > 0) lines.push(t.plural("simpleAlsoInfo", counts.info));
  }
  if (gaps > 0) lines.push(t.plural("simpleGaps", gaps));
  return lines;
}

/** Plain wording of the short-window warning. */
export function simpleShortWindowNote(): string {
  return dt()("simpleShortWindow");
}
