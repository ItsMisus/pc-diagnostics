import { memo, type ReactNode } from "react";
import { Disclosure } from "@/components/common/Disclosure";
import { Icon } from "@/components/common/Icon";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StatusBadge } from "@/components/common/StatusBadge";
import { RisolviButton } from "@/features/assistant/RisolviButton";
import type { Confidence } from "@/types/generated/Confidence";
import type { Finding } from "@/types/generated/Finding";
import type { FindingKind } from "@/types/generated/FindingKind";
import { useT } from "@/i18n";
import {
  componentKindLabel,
  confidenceLabel,
  findingKindHint,
  findingKindLabel,
  severityTone,
} from "./model";
import { simpleFindingText, simpleKindHint, simpleSeverityLabel } from "./simple";

/** Same geometry as `StatusBadge`, so every chip in a finding header lines up. */
const CHIP = "inline-flex h-[22px] shrink-0 items-center gap-1 rounded-full border px-2 text-2xs font-medium whitespace-nowrap";

/**
 * Kind badge: wording, border and icon differ so the three are never confused, and color
 * is never the distinguishing signal. Confirmed = solid neutral chip with a check;
 * hypothesis = dashed and italic (not verified); observation = plain neutral chip.
 */
const KIND_STYLE: Record<FindingKind, string> = {
  observation: "border-line bg-surface-2/60 text-fg-muted",
  hypothesis: "border-dashed border-fg-subtle/70 text-fg-muted italic",
  confirmed: "border-line-strong bg-surface-3 text-fg",
};

export function KindBadge({ kind }: { kind: FindingKind }) {
  // Subscribes to the language: the labels come from the model helpers.
  useT("diagnostics");
  return (
    <span title={findingKindHint(kind)} className={`${CHIP} ${KIND_STYLE[kind]}`}>
      {kind === "confirmed" && <Icon name="check" size={12} strokeWidth={2} className="-ml-0.5" />}
      {findingKindLabel(kind)}
      <span className="sr-only"> — {findingKindHint(kind)}</span>
    </span>
  );
}

const CONFIDENCE_LEVEL: Record<Confidence, number> = { low: 1, medium: 2, high: 3 };

/** Confidence as a qualifier, not a chip: three small bars (decorative) plus the label. */
function ConfidenceLabel({ confidence }: { confidence: Confidence }) {
  useT("diagnostics");
  const level = CONFIDENCE_LEVEL[confidence];
  return (
    <span className="inline-flex h-[22px] shrink-0 items-center gap-1.5 px-0.5 text-2xs whitespace-nowrap text-fg-muted">
      <svg aria-hidden="true" focusable="false" viewBox="0 0 10 10" className="size-2.5">
        {[0, 1, 2].map((i) => (
          <rect
            key={i}
            x={i * 3.5}
            y={6 - i * 3}
            width={2.5}
            height={4 + i * 3}
            rx={0.75}
            className={i < level ? "fill-fg-muted" : "fill-line-strong"}
          />
        ))}
      </svg>
      {confidenceLabel(confidence)}
    </span>
  );
}

/**
 * Flat panel like every other card. Severity is written, not drawn: the badge in the simple
 * view, the group heading ("Avvisi", "Critici") in the advanced one.
 */
const ARTICLE = "rounded-md border border-line bg-surface p-4";

/** `findingIndex`: position in the report's `findings` (backend order); enables "Risolvi". */
export const FindingCard = memo(function FindingCard({ finding, findingIndex }: { finding: Finding; findingIndex: number }) {
  return (
    <article className={ARTICLE}>
      <TechnicalFinding finding={finding} />
      <RisolviRow finding={finding} findingIndex={findingIndex} label={finding.title} />
    </article>
  );
});

function RisolviRow({ finding, findingIndex, label }: { finding: Finding; findingIndex: number; label: string }) {
  return (
    <div className="mt-4 flex empty:hidden">
      <RisolviButton finding={finding} findingIndex={findingIndex} label={label} />
    </div>
  );
}

/**
 * The full technical view of a finding ("Avanzata", and the details of "Semplice"). No
 * severity badge: in "Avanzata" the group heading states it, in "Semplice" the card header.
 */
function TechnicalFinding({ finding }: { finding: Finding }) {
  const t = useT("diagnostics");
  return (
    <>
      <header className="flex flex-wrap items-start justify-between gap-x-4 gap-y-2">
        <div className="min-w-[min(100%,15rem)] flex-1">
          <h3 className="text-sm leading-5 font-medium text-fg">{finding.title}</h3>
          <p className="mt-0.5 text-xs text-fg-subtle">
            {componentKindLabel(finding.component)} · {t("rule")} <span className="font-mono text-2xs">{finding.ruleId}</span>
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <KindBadge kind={finding.kind} />
          <ConfidenceLabel confidence={finding.confidence} />
        </div>
      </header>
      <p className="mt-3 text-sm text-fg">{finding.description}</p>

      {finding.evidence.length > 0 && (
        <Section title={t("evidence")}>
          <ul className="divide-y divide-line border-y border-line text-[13px]">
            {finding.evidence.map((e, i) => (
              <li key={`${e.label}-${i}`} className="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 py-1.5">
                <span className="text-fg-muted">{e.label}</span>
                <span className="ml-auto text-right font-medium text-fg tabular">{e.value}</span>
                <SourceCaption source={e.source} />
              </li>
            ))}
          </ul>
        </Section>
      )}

      {finding.possibleCauses.length > 0 && (
        <Section title={t("possibleCauses")}>
          <ul className="flex list-disc flex-col gap-0.5 pl-4 text-sm marker:text-fg-subtle">
            {finding.possibleCauses.map((c) => (
              <li key={c} className="pl-0.5">
                {c}
              </li>
            ))}
          </ul>
        </Section>
      )}

      {finding.recommendation && (
        <Section title={t("recommendation")}>
          <p className="text-sm">{finding.recommendation}</p>
        </Section>
      )}

      {finding.limitations.length > 0 && (
        <Section title={t("limitations")}>
          <ul className="flex list-disc flex-col gap-0.5 pl-4 text-[13px] text-fg-muted marker:text-fg-subtle">
            {finding.limitations.map((l) => (
              <li key={l} className="pl-0.5">
                {l}
              </li>
            ))}
          </ul>
        </Section>
      )}
    </>
  );
}

/** Visible, plain marker for a hypothesis: it is never presented as certain. */
export function SimpleKindHint({ finding }: { finding: Finding }) {
  useT("diagnostics");
  const hint = simpleKindHint(finding.kind);
  if (!hint) return null;
  return (
    <span title={findingKindHint(finding.kind)} className={`${CHIP} ${KIND_STYLE.hypothesis}`}>
      {hint}
    </span>
  );
}

/** "Semplice" view: what is happening, what it means, what to do; technical view on request. */
export const SimpleFindingCard = memo(function SimpleFindingCard({
  finding,
  findingIndex,
}: {
  finding: Finding;
  findingIndex: number;
}) {
  const t = useT("diagnostics");
  const text = simpleFindingText(finding);
  return (
    <article className={ARTICLE}>
      <header className="flex flex-wrap items-center gap-1.5">
        <StatusBadge tone={severityTone(finding.severity)} label={simpleSeverityLabel(finding.severity)} />
        <SimpleKindHint finding={finding} />
      </header>
      <h3 className="mt-2.5 text-[15px] leading-6 font-medium text-fg">{text.headline}</h3>

      {text.meaning && (
        <Section title={t("meaning")}>
          <p className="text-sm">{text.meaning}</p>
        </Section>
      )}
      {text.action && (
        <Section title={t("whatYouCanDo")}>
          <p className="text-sm">{text.action}</p>
        </Section>
      )}
      <RisolviRow finding={finding} findingIndex={findingIndex} label={text.headline} />

      <Disclosure summary={t("showTechnical")} className="mt-4 border-t border-line pt-3">
        <div className="rounded-md border border-line bg-surface-2/40 p-3.5">
          <TechnicalFinding finding={finding} />
        </div>
      </Disclosure>
    </article>
  );
});

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="mt-4">
      <h4 className="eyebrow mb-1.5">{title}</h4>
      {children}
    </div>
  );
}
