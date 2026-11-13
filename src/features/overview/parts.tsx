import { useT } from "@/i18n";
import type { Level } from "./levels";

/** "12.3 GiB" → ["12.3", "GiB"], for `Readout` / `RingGauge` (number and unit drawn apart). */
export function splitUnit(text: string): [string, string | undefined] {
  const i = text.lastIndexOf(" ");
  return i > 0 ? [text.slice(0, i), text.slice(i + 1)] : [text, undefined];
}

/** "warn" / "bad" → the tone `UsageBar` / `RingGauge` take; `normal` → none. */
export function toneOf(level: Level | undefined): "warn" | "bad" | undefined {
  return level === "warn" || level === "bad" ? level : undefined;
}

/** Short status word next to a coloured value, so colour is never the only signal. */
export function LevelLabel({ level }: { level: Level | undefined }) {
  const t = useT("overview");
  const tone = toneOf(level);
  if (!tone) return null;
  return <span className={`text-2xs font-medium ${tone === "bad" ? "text-bad" : "text-warn"}`}>{t(tone === "bad" ? "levelBad" : "levelWarn")}</span>;
}

/** Accessible value with the status appended ("92 %, avviso"). */
export function useLevelText(valueText: string, level: Level | undefined): string {
  const t = useT("overview");
  const tone = toneOf(level);
  return tone ? t("levelAria", { value: valueText, level: t(tone === "bad" ? "levelBad" : "levelWarn") }) : valueText;
}
