import { useId } from "react";
import { Icon } from "@/components/common/Icon";
import { Panel } from "@/components/common/Panel";
import { Readout, Stat } from "@/components/common/Readout";
import { Tag } from "@/components/common/Tag";
import { Tooltip } from "@/components/common/Tooltip";
import { SupportBadge, SupportReason } from "@/components/control/SupportBadge";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { useT } from "@/i18n";
import { formatCelsius, formatNumber } from "@/lib/format";
import type { CoolingFan } from "@/types/generated/CoolingFan";
import type { CoolingState } from "@/types/generated/CoolingState";
import type { PowerSupply } from "@/types/generated/PowerSupply";
import { CAPABILITY_KEYS, type CoolingType, focalModel, galleryTypes, policyFor } from "./model";
import { CoolingIllustration } from "./Illustration";
import { CoolingStage } from "./Stage";

/** "n/d" with its reason on focus and hover (never a made-up number). */
function NotAvailable({ reason }: { reason: string | null }) {
  const t = useT("cooling");
  const text = <span className="text-fg-subtle underline decoration-dotted underline-offset-4">{t("notAvailable")}</span>;
  return reason ? (
    <Tooltip content={reason} multiline side="top">
      <span tabIndex={0} className="rounded-sm">
        {text}
      </span>
    </Tooltip>
  ) : (
    text
  );
}

// ---------------------------------------------------------------- Hero

/**
 * The page's focal element: this computer's cooling system as a 3D model (only what the
 * system reported: the fanless chassis, a fan spinning at the measured speed, or a plain
 * illustration when the system does not say), with what it is and what the app can change.
 */
export function CoolingHero({ state }: { state: CoolingState }) {
  const t = useT("cooling");
  const headingId = useId();
  const model = focalModel(state);
  const rpm = model.kind === "fan" ? model.rpm : null;
  const label =
    model.kind === "passive" ? t("stage_passive") : model.kind === "fan" ? (rpm !== null ? t("stage_fan", { rpm: `${formatNumber(rpm)} ${t("rpmUnit")}` }) : t("stage_fanStill")) : t("stage_unknown");
  return (
    <section aria-labelledby={headingId} className="surface-primary relative grid grid-cols-1 rounded-2xl! @3xl:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)]">
      <div className="relative min-w-0 aspect-[16/10] max-h-[420px] w-full @3xl:aspect-auto @3xl:max-h-none @3xl:min-h-[360px]">
        {model.kind === "unknown" ? (
          <div role="img" aria-label={label} className="absolute inset-0 opacity-50">
            <CoolingIllustration type="fan" />
          </div>
        ) : (
          <CoolingStage type={model.kind} rpm={rpm} label={label} aspectRatio={null} className="absolute! inset-0" />
        )}
        {model.kind !== "unknown" && (
          <span aria-hidden="true" className="pointer-events-none absolute bottom-3 start-4 flex items-center gap-1.5 text-2xs text-fg-subtle">
            <Icon name="refresh" size={12} />
            {t("dragHint")}
          </span>
        )}
      </div>
      <div className="flex min-w-0 flex-col gap-5 p-5 pt-2 @3xl:p-6 @3xl:ps-2">
        <div className="flex flex-col gap-1.5">
          <span className="eyebrow text-fg-subtle">{t("thisComputer")}</span>
          <h2 id={headingId} className="font-display text-3xl font-semibold text-fg">
            {t(`kind_${state.system.kind}`)}
          </h2>
          <p className="max-w-prose text-sm text-fg-muted">{state.system.description}</p>
        </div>
        <div className="grid grid-cols-2 gap-4">
          <Stat
            label={t("cpuTemperature")}
            value={state.cpuTemperatureC !== null ? formatCelsius(state.cpuTemperatureC) : <NotAvailable reason={t("cpuTemperatureMissing")} />}
          />
          <Stat label={t("fanCount")} value={formatNumber(state.fans.length)} />
        </div>
        <div className="flex flex-col gap-3 border-t border-line pt-5">
          <h3 className="text-xs font-medium text-fg-muted">{t("controls")}</h3>
          <ul className="flex flex-col gap-3">
            {CAPABILITY_KEYS.map((k) => (
              <li key={k} className="min-w-0">
                <div className="flex items-center justify-between gap-3">
                  <span className="text-sm text-fg">{t(`cap_${k}`)}</span>
                  <SupportBadge support={state.capabilities[k]} />
                </div>
                <SupportReason support={state.capabilities[k]} />
              </li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}

// ---------------------------------------------------------------- Fans

function FanCard({ fan, selected, onSelect }: { fan: CoolingFan; selected: boolean; onSelect: () => void }) {
  const t = useT("cooling");
  const rpm = (v: number | null) => (v !== null ? formatNumber(v) : <NotAvailable reason={fan.rpmReason} />);
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onSelect}
      className={`pressable flex min-w-0 flex-col gap-3 rounded-lg border p-3 text-start transition-colors duration-150 ${
        selected ? "border-accent bg-accent-soft" : "border-line bg-surface-2 hover:border-line-strong"
      }`}
    >
      <span className="flex items-center justify-between gap-2">
        <span className="flex min-w-0 items-center gap-2 text-sm font-medium text-fg">
          <Icon name="fan" size={16} className="shrink-0 text-fg-muted" />
          <span className="truncate">{fan.label}</span>
        </span>
        <SupportBadge support={fan.speedControl} />
      </span>
      <span className="flex items-baseline gap-2">
        {fan.currentRpm === 0 ? <Readout value={t("fanStopped")} size="md" /> : <Readout value={rpm(fan.currentRpm)} unit={fan.currentRpm !== null ? t("rpmUnit") : ""} size="md" />}
      </span>
      <span className="grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-fg-muted">
        <span>{t("fanMin")}</span>
        <span className="tabular text-end text-fg">{rpm(fan.minRpm)}</span>
        <span>{t("fanMax")}</span>
        <span className="tabular text-end text-fg">{rpm(fan.maxRpm)}</span>
        <span>{t("fanTarget")}</span>
        <span className="tabular text-end text-fg">{rpm(fan.targetRpm)}</span>
        <span>{t("fanMode")}</span>
        <span className="text-end text-fg">{fan.mode ? t(`fanMode_${fan.mode}`) : <NotAvailable reason={fan.rpmReason} />}</span>
      </span>
    </button>
  );
}

/** The fans the system reports; selecting one edits its own curve below. */
export function FansPanel({ fans, target, onTarget }: { fans: CoolingFan[]; target: string | null; onTarget: (id: string | null) => void }) {
  const t = useT("cooling");
  return (
    <Panel title={t("fansTitle")} icon="fan">
      <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {fans.map((f) => (
          <FanCard key={f.id} fan={f} selected={target === f.id} onSelect={() => onTarget(target === f.id ? null : f.id)} />
        ))}
      </div>
    </Panel>
  );
}

// ---------------------------------------------------------------- Cooling policy

const SUPPLIES: readonly PowerSupply[] = ["ac", "battery"];

/** The Windows "System cooling policy" now in effect, per supply, and whether the app may change it. */
export function PolicyPanel({ state, className = "" }: { state: CoolingState; className?: string }) {
  const t = useT("cooling");
  const support = state.capabilities.coolingPolicy;
  return (
    <Panel title={t("policyTitle")} icon="snowflake" aside={<SupportBadge support={support} />} className={className}>
      <div className="mt-3 flex flex-col gap-3">
        {support.state !== "unsupported" && (
          <dl className="grid grid-cols-2 gap-3">
            {SUPPLIES.map((s) => {
              const p = policyFor(state.currentPolicy, s);
              return (
                <div key={s} className="surface-inset flex flex-col gap-1 p-3">
                  <dt className="text-xs text-fg-muted">{t(`supply_${s}`)}</dt>
                  <dd className="font-display text-lg text-fg">{p ? t(`policy_${p}`) : <NotAvailable reason={null} />}</dd>
                </div>
              );
            })}
          </dl>
        )}
        <SupportReason support={support} />
        <p className="text-xs text-fg-subtle">{t("policyExplain")}</p>
      </div>
    </Panel>
  );
}

// ---------------------------------------------------------------- Gallery

/**
 * The other kinds of cooling, as types: each card says it is not present on this computer, so
 * nothing here reads as detected hardware. Static models (no spin), rotatable by drag.
 */
export function CoolingGallery({ state }: { state: CoolingState }) {
  const t = useT("cooling");
  const headingId = useId();
  const types = galleryTypes(focalModel(state));
  const cols = types.length === 4 ? "@3xl:grid-cols-4 @xl:grid-cols-2" : "@3xl:grid-cols-3 @xl:grid-cols-2";
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-3">
      <SectionHeading id={headingId}>{t("galleryTitle")}</SectionHeading>
      <p className="-mt-1 text-sm text-fg-muted">{t("galleryHint")}</p>
      <div className={`grid grid-cols-1 gap-4 ${cols}`}>
        {types.map((type) => (
          <GalleryCard key={type} type={type} />
        ))}
      </div>
    </section>
  );
}

function GalleryCard({ type }: { type: CoolingType }) {
  const t = useT("cooling");
  const name = t(`type_${type}`);
  return (
    <Panel as="article" flush className="flex min-w-0 flex-col overflow-hidden">
      <CoolingStage type={type} label={name} className="w-full" />
      <div className="flex flex-1 flex-col gap-2 p-4 pt-0">
        <h3 className="font-display text-md font-semibold text-fg">{name}</h3>
        <p className="flex-1 text-xs text-fg-muted">{t(`type_${type}Text`)}</p>
        <div>
          <Tag>{t("notPresent")}</Tag>
        </div>
      </div>
    </Panel>
  );
}
