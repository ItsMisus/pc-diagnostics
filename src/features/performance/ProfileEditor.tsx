import { useEffect, useId, useState, type ReactNode } from "react";
import { Button } from "@/components/common/Button";
import { Modal, ModalBody, ModalFooter } from "@/components/common/Modal";
import { Segmented } from "@/components/common/Segmented";
import { StateView } from "@/components/common/StateView";
import { SupportBadge, SupportReason } from "@/components/control/SupportBadge";
import { useT } from "@/i18n";
import type { PerformanceBoostMode } from "@/types/generated/PerformanceBoostMode";
import type { PerformanceCapabilities } from "@/types/generated/PerformanceCapabilities";
import type { PerformanceControl } from "@/types/generated/PerformanceControl";
import type { PerformanceKeepAwake } from "@/types/generated/PerformanceKeepAwake";
import type { PerformanceSupplySettings } from "@/types/generated/PerformanceSupplySettings";
import type { PowerSupply } from "@/types/generated/PowerSupply";
import type { Support } from "@/types/generated/Support";
import {
  ADVANCED,
  EDITABLE_BOOST,
  MODE_ICON,
  MODES,
  anyAdvancedSupported,
  copySupply,
  draftErrors,
  minutesToSeconds,
  parseField,
  type ProfileDraft,
  secondsToMinutes,
  settingsFor,
  supportOf,
  toInput,
  updateSupply,
} from "./model";
import { performanceStore, usePerformance } from "./store";

const KEEP_AWAKE: readonly PerformanceKeepAwake[] = ["off", "system", "display"];

/** One labelled control with its support badge and, when it cannot act, the reason. */
function Field({ label, support, children, hint, htmlFor }: { label: string; support: Support; children: ReactNode; hint?: string; htmlFor?: string }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between gap-3">
        <label htmlFor={htmlFor} className="text-sm font-medium text-fg">
          {label}
        </label>
        <SupportBadge support={support} />
      </div>
      {children}
      {hint && <p className="text-xs text-fg-subtle">{hint}</p>}
      <SupportReason support={support} />
    </div>
  );
}

function NumberInput({
  id,
  value,
  onChange,
  disabled,
  placeholder,
  min,
  max,
}: {
  id: string;
  value: number | null;
  onChange: (v: number | null) => void;
  disabled: boolean;
  placeholder: string;
  min: number;
  max: number;
}) {
  // Text state so an empty field (= leave as is) and partial input survive re-renders.
  const [text, setText] = useState(value === null ? "" : String(value));
  useEffect(() => setText(value === null ? "" : String(value)), [value]);
  return (
    <input
      id={id}
      type="number"
      inputMode="numeric"
      min={min}
      max={max}
      value={text}
      disabled={disabled}
      placeholder={placeholder}
      onChange={(e) => {
        setText(e.target.value);
        onChange(parseField(e.target.value));
      }}
      className="h-8 w-full tabular"
    />
  );
}

/**
 * Creates or edits a user profile: name, then per supply (adapter / battery, only the adapter
 * on a desktop) the mode, keep-awake and the Windows advanced values. Controls this computer
 * cannot use are disabled with their reason; their values are kept in the profile.
 */
export function ProfileEditor({
  open,
  draft: initial,
  hasBattery,
  capabilities,
  currentSupply,
  onClose,
  onDelete,
}: {
  open: boolean;
  draft: ProfileDraft;
  hasBattery: boolean;
  capabilities: PerformanceCapabilities;
  currentSupply: PowerSupply;
  onClose: (savedId: string | null) => void;
  onDelete?: () => void;
}) {
  const t = useT("performance");
  const [draft, setDraft] = useState(initial);
  const [supply, setSupply] = useState<PowerSupply>(hasBattery ? currentSupply : "ac");
  const [touched, setTouched] = useState(false);
  const save = usePerformance((s) => s.save);
  const saving = usePerformance((s) => s.busy?.action === "save");
  const saveError = usePerformance((s) => (s.actionError?.action === "save" ? s.actionError.error : null));
  const clearError = usePerformance((s) => s.clearActionError);
  const ids = { name: useId(), max: useId(), min: useId(), boost: useId(), display: useId(), sleep: useId() };

  useEffect(() => {
    if (!open) return;
    setDraft(initial);
    setSupply(hasBattery ? currentSupply : "ac");
    setTouched(false);
    clearError();
  }, [open, initial, hasBattery, currentSupply, clearError]);

  const s = settingsFor(draft, supply);
  const patch = (p: Partial<PerformanceSupplySettings>) => setDraft((d) => updateSupply(d, supply, p));
  const sup = (c: PerformanceControl) => supportOf(capabilities, c);
  const off = (c: PerformanceControl) => sup(c).state !== "supported";
  const errors = draftErrors(draft, hasBattery);
  const showErrors = touched ? errors : errors.filter((e) => e !== "nameEmpty");

  const submit = async () => {
    setTouched(true);
    if (errors.length > 0) return;
    // The ids before saving, to find the one a save created.
    const before = new Set((performanceStore.getState().data?.profiles ?? []).map((p) => p.id));
    const state = await save(toInput(draft, hasBattery));
    if (!state) return;
    const savedId = draft.id ?? state.profiles.find((p) => !p.builtin && !before.has(p.id))?.id ?? null;
    onClose(savedId);
  };

  const showAdvanced = anyAdvancedSupported(capabilities);

  return (
    <Modal
      open={open}
      onClose={() => onClose(null)}
      title={draft.id ? t("editorEdit") : t("editorNew")}
      icon="sliders"
      size="lg"
      dismissible={!saving}
      bare
    >
      <ModalBody className="flex flex-col gap-5">
        <div className="flex flex-col gap-1.5">
          <label htmlFor={ids.name} className="text-sm font-medium text-fg">
            {t("name")}
          </label>
          <input
            id={ids.name}
            type="text"
            autoFocus
            maxLength={60}
            value={draft.name}
            onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
            onBlur={() => setTouched(true)}
            className="h-8 w-full"
          />
        </div>

        {hasBattery ? (
          <div className="flex flex-wrap items-center justify-between gap-3">
            <Segmented
              label={t("supplyLabel")}
              value={supply}
              onChange={setSupply}
              options={[
                { value: "ac", label: t("supply_ac"), icon: "power" },
                { value: "battery", label: t("supply_battery"), icon: "battery" },
              ]}
            />
            <Button variant="ghost" size="sm" icon="copy" onClick={() => setDraft((d) => copySupply(d, supply))}>
              {supply === "ac" ? t("copyToBattery") : t("copyToAc")}
            </Button>
          </div>
        ) : (
          <p className="text-xs text-fg-muted">{t("desktopOnlyAc")}</p>
        )}

        <div className="surface-inset flex flex-col gap-5">
          <Field label={t("modeLabel")} support={sup("mode")}>
            <Segmented
              label={t("modeLabel")}
              value={s.mode}
              onChange={(mode) => patch({ mode })}
              fullWidth
              options={MODES.map((m) => ({ value: m, label: t(`mode_${m}`), icon: MODE_ICON[m], disabled: off("mode") && m !== s.mode }))}
            />
          </Field>
          <Field label={t("keepAwakeLabel")} support={sup("keepAwake")}>
            <Segmented
              label={t("keepAwakeLabel")}
              value={s.keepAwake}
              onChange={(keepAwake) => patch({ keepAwake })}
              fullWidth
              options={KEEP_AWAKE.map((k) => ({ value: k, label: t(`keepAwake_${k}`), disabled: off("keepAwake") && k !== s.keepAwake }))}
            />
          </Field>
        </div>

        {showAdvanced ? (
          <section className="flex flex-col gap-3" aria-label={t("advanced")}>
            <div>
              <h3 className="font-display text-md font-semibold text-fg">{t("advanced")}</h3>
              <p className="text-xs text-fg-muted">{t("advancedHint")}</p>
            </div>
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Field label={t("processorMax")} support={sup("processorState")} htmlFor={ids.max}>
                <NumberInput id={ids.max} value={s.processorMaxPercent} onChange={(v) => patch({ processorMaxPercent: v })} disabled={off("processorState")} placeholder={t("asNow")} min={5} max={100} />
              </Field>
              <Field label={t("processorMin")} support={sup("processorState")} htmlFor={ids.min}>
                <NumberInput id={ids.min} value={s.processorMinPercent} onChange={(v) => patch({ processorMinPercent: v })} disabled={off("processorState")} placeholder={t("asNow")} min={0} max={100} />
              </Field>
              <Field label={t("boostLabel")} support={sup("boostMode")} htmlFor={ids.boost}>
                <select
                  id={ids.boost}
                  value={s.boostMode ?? ""}
                  disabled={off("boostMode")}
                  onChange={(e) => patch({ boostMode: e.target.value === "" ? null : (e.target.value as PerformanceBoostMode) })}
                  className="h-8 w-full"
                >
                  <option value="">{t("boostKeep")}</option>
                  {(s.boostMode && !EDITABLE_BOOST.includes(s.boostMode) ? [...EDITABLE_BOOST, s.boostMode] : EDITABLE_BOOST).map((b) => (
                    <option key={b} value={b}>
                      {t(`boost_${b}`)}
                    </option>
                  ))}
                </select>
              </Field>
              <div className="hidden sm:block" aria-hidden="true" />
              <Field label={t("displayMinutes")} support={sup("displayTimeout")} hint={t("zeroIsNever")} htmlFor={ids.display}>
                <NumberInput
                  id={ids.display}
                  value={secondsToMinutes(s.displayOffSeconds)}
                  onChange={(v) => patch({ displayOffSeconds: minutesToSeconds(v) })}
                  disabled={off("displayTimeout")}
                  placeholder={t("asNow")}
                  min={0}
                  max={300}
                />
              </Field>
              <Field label={t("sleepMinutes")} support={sup("sleepTimeout")} hint={t("zeroIsNever")} htmlFor={ids.sleep}>
                <NumberInput
                  id={ids.sleep}
                  value={secondsToMinutes(s.sleepSeconds)}
                  onChange={(v) => patch({ sleepSeconds: minutesToSeconds(v) })}
                  disabled={off("sleepTimeout")}
                  placeholder={t("asNow")}
                  min={0}
                  max={300}
                />
              </Field>
            </div>
          </section>
        ) : (
          // Advanced values exist only where the OS exposes them: one line with the reason.
          <AdvancedUnavailable capabilities={capabilities} />
        )}

        {showErrors.length > 0 && (
          <ul className="flex flex-col gap-1 text-xs text-bad" role="alert">
            {showErrors.map((e) => (
              <li key={e}>{t(`error_${e}`)}</li>
            ))}
          </ul>
        )}
        {saveError && <StateView kind="error" compact title={t("saveFailed")} error={saveError} />}
      </ModalBody>
      <ModalFooter className="justify-between">
        <div>
          {onDelete && (
            <Button variant="danger" icon="close" onClick={onDelete} disabled={saving}>
              {t("delete")}
            </Button>
          )}
        </div>
        <div className="flex gap-2">
          <Button onClick={() => onClose(null)} disabled={saving}>
            {t("cancel")}
          </Button>
          <Button variant="primary" loading={saving} onClick={() => void submit()} disabled={touched && errors.length > 0}>
            {draft.id ? t("save") : t("saveAndApply")}
          </Button>
        </div>
      </ModalFooter>
    </Modal>
  );
}

function AdvancedUnavailable({ capabilities }: { capabilities: PerformanceCapabilities }) {
  const t = useT("performance");
  const reason = ADVANCED.map((c) => supportOf(capabilities, c).reason).find((r) => r);
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between gap-3">
        <h3 className="font-display text-md font-semibold text-fg">{t("advanced")}</h3>
        <SupportBadge support={supportOf(capabilities, "processorState")} />
      </div>
      {reason && <p className="text-xs text-fg-muted">{reason}</p>}
    </div>
  );
}
