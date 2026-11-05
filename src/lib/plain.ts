import type { ErrorCode } from "@/types/generated/ErrorCode";
import { translatorFor } from "@/i18n";
import { unavailabilityText } from "@/lib/format";
import type { Unavailability } from "@/types/generated/Unavailability";

/**
 * Plain-language copy for the "Semplice" view. Pure functions only, so every text is
 * unit-testable. The technical wording (lib/format.ts) stays available behind a
 * disclosure or tooltip: nothing is hidden, it is just not the first thing shown.
 * Texts live in the "errors" namespace and follow the current language at call time.
 */

/** Technical tokens that must never appear in simple-mode copy. */
export const JARGON_TOKENS = ["CM_PROB", "PDH", "NVML", "WDDM", "LUID", "HRESULT", "NTSTATUS"] as const;

/** True when `text` contains one of `JARGON_TOKENS` (case-sensitive: they are acronyms). */
export function containsJargon(text: string): boolean {
  return JARGON_TOKENS.some((t) => text.includes(t));
}

/** Why a value is missing, in everyday words. The technical reason goes in a tooltip. */
export function unavailabilitySimpleText(reason: Unavailability): string {
  return translatorFor("errors")(`simpleReason_${reason.kind}`);
}

/**
 * Visible text and tooltip for a missing value. "Avanzata": the technical reason, as before.
 * "Semplice": the plain reason, with the technical one kept in the tooltip.
 */
export function reasonTexts(reason: Unavailability, simple: boolean): { text: string; tooltip: string } {
  const technical = unavailabilityText(reason);
  if (!simple) return { text: technical, tooltip: technical };
  const plain = unavailabilitySimpleText(reason);
  return { text: plain, tooltip: translatorFor("errors")("reasonTooltip", { plain, technical }) };
}

export interface SimpleError {
  /** Short friendly title, used when the caller does not give a context-specific one. */
  title: string;
  /** One sentence: what happened, without codes. */
  explanation: string;
  /** "Cosa puoi fare": a safe, concrete next step. */
  action: string;
}

export function simpleError(code: ErrorCode): SimpleError {
  const t = translatorFor("errors");
  return {
    title: t(`simpleTitle_${code}`),
    explanation: t(`simpleExplanation_${code}`),
    action: t(`simpleAction_${code}`),
  };
}
