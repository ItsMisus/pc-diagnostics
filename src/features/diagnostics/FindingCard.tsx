import { memo, useId, type ReactNode } from "react";
import { useAppStore } from "@/app/store";
import { SECTIONS, sectionLabel } from "@/app/sections";
import { Button } from "@/components/common/Button";
import { Disclosure } from "@/components/common/Disclosure";
import { Icon } from "@/components/common/Icon";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StatusBadge } from "@/components/common/StatusBadge";
import { canShowRisolvi } from "@/features/assistant/model";
import { RisolviButton } from "@/features/assistant/RisolviButton";
import { usePlatform } from "@/lib/platform";
import type { Confidence } from "@/types/generated/Confidence";
import type { Finding } from "@/types/generated/Finding";
import type { FindingKind } from "@/types/generated/FindingKind";
import { useT } from "@/i18n";
import {
  causeLabel,
  causeStatus,
  componentKindLabel,
  confidenceLabel,
  findingKindHint,
  relatedSection,
  severityLabel,
  severityTone,
  type CauseStatus,
} from "./model";
import { simpleFindingText, simpleKindHint, simpleSeverityLabel } from "./simple";

/** Same geometry as `StatusBadge`, so every chip in a finding header lines up. */
const CHIP = "inline-flex h-[22px] shrink-0 items-center gap-1 rounded-full border px-2 text-2xs font-medium whitespace-nowrap";

/**
 * Cause chip: wording, border and glyph differ so the three are never confused, and color
 * is never the distinguishing signal. Confirmed = solid neutral chip with a check; possible =
 * dashed outline (not verified); undetermined = plain neutral chip.
 */
const CAUSE_STYLE: Record<CauseStatus, string> = {
  confirmed: "border-line-strong bg-surface-3 text-fg",
  possible: "border-dashed border-fg-subtle/70 text-fg-muted",
  undetermined: "border-line bg-control text-fg-muted",
};

/**
 * "Causa confermata" / "Causa possibile" / "Causa non determinata", from the finding's kind.
 * `simple` words a hypothesis in everyday language ("Possibile causa, non certa"). The hint is
 * read out to assistive technology; the details state it visibly.
 */
export function CauseBadge({ kind, simple = false }: { kind: FindingKind; simple?: boolean }) {
  // Subscribes to the language: the labels come from the model helpers.
  useT("diagnostics");
  const status = causeStatus(kind);
  const label = simple ? (simpleKindHint(kind) ?? causeLabel(status)) : causeLabel(status);
  return (
    <span className={`${CHIP} ${CAUSE_STYLE[status]}`}>
      {status === "confirmed" && <Icon name="check" size={12} strokeWidth={2} className="-ml-0.5" />}
      {label}
      <span className="sr-only"> — {findingKindHint(kind)}</span>
    </span>
  );
}

/** Visible, plain marker for a hypothesis (Overview, simple view): never presented as certain. */
export function SimpleKindHint({ finding }: { finding: Finding }) {
  useT("diagnostics");
  if (!simpleKindHint(finding.kind)) return null;
  return <CauseBadge kind={finding.kind} simple />;
}

const CONFIDENCE_LEVEL: Record<Confidence, number> = { low: 1, medium: 2, high: 3 };

/** Confidence as a qualifier, not a chip: three small bars (decorative) plus the label. */
function ConfidenceLabel({ confidence }: { confidence: Confidence }) {
  useT("diagnostics");
  const level = CONFIDENCE_LEVEL[confidence];
  return (
    <span className="inline-flex items-center gap-1.5 whitespace-nowrap">
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

/** Finding panel: solid secondary surface (it carries measured data), container for its own grid. */
const ARTICLE = "surface-secondary @container min-w-0 p-4 sm:p-5";

/**
 * "Avanzata" finding: severity badge with glyph and cause chip, title, description, then the
 * evidence next to the cause, and the suggested actions last. `findingIndex`: position in
 * the report's `findings` (backend order); enables "Risolvi".
 */
export const FindingCard = memo(function FindingCard({ finding, findingIndex }: { finding: Finding; findingIndex: number }) {
  const t = useT("diagnostics");
  const titleId = useId();
  return (
    <article aria-labelledby={titleId} className={ARTICLE}>
      <header>
        <div className="flex flex-wrap items-center gap-1.5">
          <StatusBadge glyph tone={severityTone(finding.severity)} label={severityLabel(finding.severity)} />
          <CauseBadge kind={finding.kind} />
        </div>
        <h3 id={titleId} className="mt-3 font-display text-md font-semibold break-words text-fg">
          {finding.title}
        </h3>
        <p className="mt-0.5 text-xs break-words text-fg-subtle">
          {componentKindLabel(finding.component)} · {t("rule")} <span className="font-mono text-2xs break-all">{finding.ruleId}</span>
        </p>
      </header>
      <FindingDetails finding={finding} />
      <ActionsBlock finding={finding} findingIndex={findingIndex} label={finding.title} text={finding.recommendation.trim()} />
    </article>
  );
});

/**
 * Description, evidence (the values the rule measured), cause and limitations. Shared by the
 * "Avanzata" card and the technical details of the "Semplice" one.
 */
function FindingDetails({ finding }: { finding: Finding }) {
  const t = useT("diagnostics");
  const hasEvidence = finding.evidence.length > 0;
  return (
    <>
      <p className="mt-3 max-w-prose text-sm break-words text-fg">{finding.description}</p>

      <div className={`mt-4 grid gap-4 ${hasEvidence ? "@2xl:grid-cols-2" : ""}`}>
        {hasEvidence && (
          <div className="min-w-0">
            <h4 className="eyebrow mb-1.5">{t("evidence")}</h4>
            <dl className="surface-inset divide-y divide-line px-3 py-0.5 text-sm">
              {finding.evidence.map((e, i) => (
                <div key={`${e.label}-${i}`} className="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 py-2">
                  <dt className="min-w-0 break-words text-fg-muted">{e.label}</dt>
                  <dd className="ml-auto min-w-0 text-right font-medium break-words text-fg tabular">{e.value}</dd>
                  <dd className="w-full text-right empty:hidden">
                    <SourceCaption source={e.source} />
                  </dd>
                </div>
              ))}
            </dl>
          </div>
        )}
        <CauseBlock finding={finding} />
      </div>

      {finding.limitations.length > 0 && (
        <Disclosure summary={`${t("limitations")} (${finding.limitations.length})`} className="mt-4">
          <ul className="mt-2 flex list-disc flex-col gap-0.5 pl-4 text-sm text-fg-muted marker:text-fg-subtle">
            {finding.limitations.map((l) => (
              <li key={l} className="pl-0.5 break-words">
                {l}
              </li>
            ))}
          </ul>
        </Disclosure>
      )}
    </>
  );
}

/** What is known about the cause, stated in words, then the candidate causes. */
function CauseBlock({ finding }: { finding: Finding }) {
  const t = useT("diagnostics");
  const status = causeStatus(finding.kind);
  return (
    <div className="min-w-0">
      <h4 className="eyebrow mb-1.5">{t("cause")}</h4>
      <div className="surface-inset px-3 py-2.5">
        <p className="flex items-start gap-2 text-sm">
          <Icon
            name={status === "confirmed" ? "success" : "info"}
            size={14}
            className={`mt-0.5 shrink-0 ${status === "confirmed" ? "text-fg" : "text-fg-subtle"}`}
          />
          <span className="min-w-0">
            <span className={`font-semibold text-fg ${status === "possible" ? "underline decoration-dashed decoration-fg-subtle underline-offset-4" : ""}`}>
              {causeLabel(status)}
            </span>
            <span className="text-fg-muted">: {findingKindHint(finding.kind).toLowerCase()}</span>
          </span>
        </p>
        <p className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 pl-[22px] text-xs text-fg-muted">
          <ConfidenceLabel confidence={finding.confidence} />
          {finding.evidence.length > 0 && <span className="tabular">{t.plural("evidenceBasis", finding.evidence.length)}</span>}
        </p>
      </div>
      {finding.possibleCauses.length > 0 && (
        <>
          <h4 className="eyebrow mt-3 mb-1.5">{t("possibleCauses")}</h4>
          <ul className="flex list-disc flex-col gap-0.5 pl-4 text-sm marker:text-fg-subtle">
            {finding.possibleCauses.map((c) => (
              <li key={c} className="pl-0.5 break-words">
                {c}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

/**
 * Suggested actions: what to do in words, then the buttons that exist for this finding
 * ("Risolvi" for warnings and critical findings, the app section where to act). Hidden when
 * there is neither text nor a button.
 */
function ActionsBlock({
  finding,
  findingIndex,
  label,
  text,
  heading,
}: {
  finding: Finding;
  findingIndex: number;
  label: string;
  text: string;
  heading?: string;
}) {
  const t = useT("diagnostics");
  const platform = usePlatform();
  const buttons = (findingIndex >= 0 && canShowRisolvi(finding)) || relatedSection(finding.ruleId, platform) !== null;
  if (!text && !buttons) return null;
  return (
    <div className="mt-4 border-t border-line pt-4">
      <h4 className="eyebrow mb-1.5">{heading ?? t("suggestedActions")}</h4>
      {text && <p className="max-w-prose text-sm break-words">{text}</p>}
      {buttons && (
        <div className={`flex flex-wrap items-center gap-2 ${text ? "mt-3" : ""}`}>
          <RisolviButton finding={finding} findingIndex={findingIndex} label={label} />
          <RelatedSectionButton finding={finding} label={label} />
        </div>
      )}
    </div>
  );
}

/** "Apri Processes" / "Apri Drivers" / "Apri History", when that section helps with the finding. */
function RelatedSectionButton({ finding, label }: { finding: Finding; label: string }) {
  const t = useT("diagnostics");
  const platform = usePlatform();
  const setSection = useAppStore((s) => s.setSection);
  const target = relatedSection(finding.ruleId, platform);
  if (!target) return null;
  const section = sectionLabel(target);
  const icon = SECTIONS.find((s) => s.id === target)?.icon;
  return (
    <Button
      variant="ghost"
      {...(icon ? { icon } : {})}
      aria-label={t("openSectionAria", { section, label })}
      onClick={() => setSection(target)}
    >
      {t("openSection", { section })}
    </Button>
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
  const titleId = useId();
  const text = simpleFindingText(finding);
  const status = causeStatus(finding.kind);
  return (
    <article aria-labelledby={titleId} className={ARTICLE}>
      <header>
        <div className="flex flex-wrap items-center gap-1.5">
          <StatusBadge glyph tone={severityTone(finding.severity)} label={simpleSeverityLabel(finding.severity)} />
          {status !== "undetermined" && <CauseBadge kind={finding.kind} simple />}
        </div>
        <h3 id={titleId} className="mt-3 font-display text-md font-semibold break-words text-fg">
          {text.headline}
        </h3>
      </header>

      {text.meaning && (
        <Section title={t("meaning")}>
          <p className="max-w-prose text-sm break-words">{text.meaning}</p>
        </Section>
      )}
      <ActionsBlock finding={finding} findingIndex={findingIndex} label={text.headline} text={text.action} heading={t("whatYouCanDo")} />

      <Disclosure summary={t("showTechnical")} className="mt-4 border-t border-line pt-3">
        <div className="mt-3">
          <h4 className="text-sm font-medium break-words text-fg">{finding.title}</h4>
          <p className="mt-0.5 text-xs break-words text-fg-subtle">
            {componentKindLabel(finding.component)} · {t("rule")} <span className="font-mono text-2xs break-all">{finding.ruleId}</span>
          </p>
          <FindingDetails finding={finding} />
          {finding.recommendation.trim() && (
            <Section title={t("recommendation")}>
              <p className="max-w-prose text-sm break-words">{finding.recommendation}</p>
            </Section>
          )}
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
