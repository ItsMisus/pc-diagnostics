import { useId, useState } from "react";
import { Icon, type IconName } from "@/components/common/Icon";
import { Panel } from "@/components/common/Panel";
import { Readout, Stat } from "@/components/common/Readout";
import { Tooltip } from "@/components/common/Tooltip";
import { bySupport } from "@/components/control/support";
import { SupportBadge, SupportReason } from "@/components/control/SupportBadge";
import { useT } from "@/i18n";
import { formatCelsius, formatNumber } from "@/lib/format";
import type { CoolingFan } from "@/types/generated/CoolingFan";
import type { CoolingState } from "@/types/generated/CoolingState";
import type { PowerSupply } from "@/types/generated/PowerSupply";
import { CAPABILITY_KEYS, type CoolingType, focalModel, galleryTypes, policyFor, presentType } from "./model";
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

const TYPE_ICON: Record<CoolingType, IconName> = { fan: "fan", tower: "snowflake", aio: "temperature", passive: "laptop" };

/**
 * The page's focal element. On entry the stage shows this computer's own cooling (only what
 * the system reported: the fanless laptop with its heat leaving the case, a fan spinning at
 * the measured speed, or a plain illustration when the system does not say). Under it, the
 * other kinds of cooling can be shown as examples, each marked as not present here.
 * The right column is about this computer: what it is, the readings, what the app can change.
 */
export function CoolingHero({ state }: { state: CoolingState }) {
  const t = useT("cooling");
  const headingId = useId();
  const model = focalModel(state);
  const present = presentType(model);
  const [shown, setShown] = useState<CoolingType | null>(present);
  const example = shown !== null && shown !== present;
  const rpm = model.kind === "fan" && !example ? model.rpm : null;
  const ownLabel =
    model.kind === "passive" ? t("stage_passive") : model.kind === "fan" ? (rpm !== null ? t("stage_fan", { rpm: `${formatNumber(rpm)} ${t("rpmUnit")}` }) : t("stage_fanStill")) : t("stage_unknown");
  const label = example ? t("stage_example", { type: t(`type_${shown}`) }) : ownLabel;
  const options: { key: CoolingType | null; name: string; icon: IconName; own: boolean }[] = [
    { key: present, name: present ? t(`type_${present}`) : t("typeUnknown"), icon: present ? TYPE_ICON[present] : "info", own: true },
    ...galleryTypes(model).map((k) => ({ key: k, name: t(`type_${k}`), icon: TYPE_ICON[k], own: false })),
  ];
  const caps = bySupport(CAPABILITY_KEYS, (k) => state.capabilities[k]);

  return (
    <section aria-labelledby={headingId} className="surface-primary relative grid grid-cols-1 rounded-2xl! @3xl:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)]">
      <div className="flex min-w-0 flex-col">
        <div className="relative min-w-0 aspect-[16/10] max-h-[420px] w-full @3xl:aspect-auto @3xl:max-h-none @3xl:min-h-[360px] @3xl:flex-1">
          {shown === null ? (
            <div role="img" aria-label={label} className="absolute inset-0 opacity-50">
              <CoolingIllustration type="fan" />
            </div>
          ) : (
            <CoolingStage key={shown} type={shown} rpm={rpm} label={label} aspectRatio={null} className="absolute! inset-0" />
          )}
          <div className="pointer-events-none absolute start-4 top-4">
            {example ? (
              <span className="glass inline-flex h-7 items-center gap-1.5 rounded-full px-2.5 text-xs text-fg">
                <Icon name="info" size={14} className="text-fg-subtle" />
                {t("exampleBadge")}
              </span>
            ) : (
              <span className="glass inline-flex h-7 items-center gap-1.5 rounded-full px-2.5 text-xs text-fg">
                <span aria-hidden="true" className="size-1.5 rounded-full bg-ok" />
                {t("thisComputer")}
              </span>
            )}
          </div>
          {shown === "passive" && (
            <p aria-hidden="true" className="pointer-events-none absolute end-4 bottom-3 flex items-center gap-1.5 text-2xs text-fg-muted">
              <span className="inline-block h-1.5 w-3 rounded-full bg-[#f5782e]" />
              {t("legendHeat")}
            </p>
          )}
          {shown !== null && (
            <span aria-hidden="true" className="pointer-events-none absolute bottom-3 start-4 flex items-center gap-1.5 text-2xs text-fg-subtle">
              <Icon name="refresh" size={12} />
              {t("dragHint")}
            </span>
          )}
        </div>
        <div className="flex flex-col gap-2 px-4 pb-4">
          {example && shown && <p className="text-xs text-fg-muted">{t(`type_${shown}Text`)}</p>}
          <div role="radiogroup" aria-label={t("typesLabel")} className="flex flex-wrap gap-1.5">
            {options.map((o) => {
              const on = o.key === shown;
              return (
                <button
                  key={o.key ?? "unknown"}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  onClick={() => setShown(o.key)}
                  className={`pressable inline-flex h-8 max-w-[16rem] items-center gap-1.5 rounded-full px-3 text-xs transition-colors duration-150 ${
                    on ? "bg-accent-soft text-accent-text shadow-[inset_0_0_0_1px_var(--accent)]" : "satin text-fg-muted hover:text-fg"
                  }`}
                >
                  <Icon name={o.icon} size={14} className="shrink-0" />
                  <span className="truncate">{o.name}</span>
                  {o.own && <span className="text-2xs text-fg-subtle">· {t("thisComputer")}</span>}
                </button>
              );
            })}
          </div>
        </div>
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
            {caps.map((k) => (
              <li key={k} className="min-w-0">
                <div className="flex items-center justify-between gap-3">
                  <span className="text-sm text-fg">{t(`cap_${k}`)}</span>
                  <SupportBadge support={state.capabilities[k]} />
                </div>
                <SupportReason support={state.capabilities[k]} />
                {k === "coolingPolicy" && state.capabilities.coolingPolicy.state !== "unsupported" && <PolicyValues state={state} />}
              </li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}

const SUPPLIES: readonly PowerSupply[] = ["ac", "battery"];

/** The Windows "System cooling policy" now in effect, per supply, under its row in the hero. */
function PolicyValues({ state }: { state: CoolingState }) {
  const t = useT("cooling");
  return (
    <div className="mt-2 flex flex-col gap-1.5">
      <dl className="flex flex-wrap gap-x-5 gap-y-1 text-xs">
        {SUPPLIES.map((s) => {
          const p = policyFor(state.currentPolicy, s);
          return (
            <div key={s} className="flex items-center gap-1.5">
              <dt className="text-fg-muted">{t(`supply_${s}`)}</dt>
              <dd className="text-fg">{p ? t(`policy_${p}`) : <NotAvailable reason={null} />}</dd>
            </div>
          );
        })}
      </dl>
      <p className="text-xs text-fg-subtle">{t("policyExplain")}</p>
    </div>
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

