import { type Translator, tr, translatorFor } from "@/i18n";
import { formatSignedPercent } from "@/lib/format";
import type { BenchmarkComparison } from "@/types/generated/BenchmarkComparison";
import type { MetricDelta } from "@/types/generated/MetricDelta";

/**
 * Run-to-run differences below this are treated as measurement noise: shown, but never
 * coloured as an improvement or a regression.
 */
export const MEANINGFUL_CHANGE_PCT = 3;

export type DeltaDirection = "better" | "worse" | "noise" | "unknown";

export interface DeltaView {
  text: string;
  direction: DeltaDirection;
  tone: "ok" | "bad" | "neutral";
  /** Spoken / tooltip explanation. */
  description: string;
}

export function deltaView(
  delta: MetricDelta,
  comparable: boolean,
  t: Translator<"benchmarks"> = translatorFor("benchmarks"),
): DeltaView {
  const pct = delta.changePercent;
  if (pct === null || !Number.isFinite(pct)) {
    return { text: tr("common", "notAvailable"), direction: "unknown", tone: "neutral", description: t("deltaUnknown") };
  }
  const text = formatSignedPercent(pct);
  if (!comparable) {
    return { text, direction: "unknown", tone: "neutral", description: t("deltaNotComparable") };
  }
  if (Math.abs(pct) < MEANINGFUL_CHANGE_PCT) {
    return { text, direction: "noise", tone: "neutral", description: t("deltaNoise", { pct: MEANINGFUL_CHANGE_PCT }) };
  }
  const better = delta.higherIsBetter ? pct > 0 : pct < 0;
  return better
    ? { text, direction: "better", tone: "ok", description: t("deltaBetter") }
    : { text, direction: "worse", tone: "bad", description: t("deltaWorse") };
}

export type ComparisonSummary =
  | { kind: "comparable"; caveats: string[] }
  | { kind: "notComparable"; caveats: string[] };

export function comparisonSummary(c: BenchmarkComparison): ComparisonSummary {
  return c.comparable && c.deltas.length > 0
    ? { kind: "comparable", caveats: c.caveats }
    : { kind: "notComparable", caveats: c.caveats };
}
