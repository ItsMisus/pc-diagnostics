import { useId, useState } from "react";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { Modal } from "@/components/common/Modal";
import { Panel } from "@/components/common/Panel";
import { Segmented } from "@/components/common/Segmented";
import { Tooltip } from "@/components/common/Tooltip";
import { SupportBadge, SupportReason } from "@/components/control/SupportBadge";
import { useT } from "@/i18n";
import type { CoolingCurvePoint } from "@/types/generated/CoolingCurvePoint";
import type { CoolingPolicy } from "@/types/generated/CoolingPolicy";
import type { CoolingState } from "@/types/generated/CoolingState";
import type { PowerSupply } from "@/types/generated/PowerSupply";
import { addPoint, type CurveIssue, type CurvePresetId, matchingPreset, MAX_POINTS, MIN_POINTS, PRESETS, removePoint, validateCurve } from "./curve";
import { CurveChart } from "./CurveChart";
import { ALL_FANS, applyBlockReason, curveOf, type Draft, NAME_MAX, nameIssue, policyFor, userProfiles, withPolicy } from "./model";

const PRESET_IDS: readonly CurvePresetId[] = ["silent", "balanced", "aggressive"];
const SUPPLIES: readonly PowerSupply[] = ["ac", "battery"];
type PolicyChoice = CoolingPolicy | "system";

/**
 * The fan curve editor: which profile (new or one of the user's), which fans (all, or one
 * reported fan), a starting preset, the interactive chart, point add/remove, live
 * validation, the profile's Windows cooling policy where that setting works, then Save
 * (always) and Save and apply (disabled with the reason where nothing in it can act here).
 */
export function CurvePanel({
  state,
  draft,
  onDraft,
  target,
  onTarget,
  onEdit,
  onSave,
  onDelete,
  busy,
}: {
  state: CoolingState;
  draft: Draft;
  onDraft: (d: Draft) => void;
  target: string | null;
  onTarget: (id: string | null) => void;
  /** Starts editing a user profile (id) or a new one (null). */
  onEdit: (id: string | null) => void;
  onSave: (apply: boolean) => void;
  onDelete: () => void;
  busy: boolean;
}) {
  const t = useT("cooling");
  const nameId = useId();
  const issuesId = useId();
  const [focused, setFocused] = useState<number | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const key = target ?? ALL_FANS;
  const points = curveOf(draft, key);
  const issues = validateCurve(points);
  const allIssues = Object.values(draft.curves).flatMap(validateCurve);
  const name = nameIssue(draft.name);
  const blocked = applyBlockReason(state.capabilities, draft);
  const users = userProfiles(state);
  const policySupported = state.capabilities.coolingPolicy.state === "supported";
  const canSave = allIssues.length === 0 && name === null && !busy;
  const preset = matchingPreset(points);

  const setPoints = (next: CoolingCurvePoint[]) => onDraft({ ...draft, curves: { ...draft.curves, [key]: next } });
  const remove = (i: number) => {
    const next = removePoint(points, i);
    if (!next) return;
    setPoints(next);
    setFocused(next.length ? Math.min(i, next.length - 1) : null);
  };
  const add = () => {
    const r = addPoint(points);
    if (!r) return;
    setPoints(r.points);
    setFocused(r.index);
  };
  const issueText = (i: CurveIssue) => (i.kind === "count" ? t("issue_count") : t(`issue_${i.kind}`, { n: i.index + 1 }));

  const policyOptions = [
    { value: "system" as PolicyChoice, label: t("policy_system") },
    { value: "active" as PolicyChoice, label: t("policy_active") },
    { value: "passive" as PolicyChoice, label: t("policy_passive") },
  ];

  const saveApply = (
    <Button variant="primary" icon="check" disabled={!canSave || blocked !== null} loading={busy} onClick={() => onSave(true)}>
      {t("saveApply")}
    </Button>
  );

  return (
    <Panel title={t("curveTitle")} icon="sliders" aside={<SupportBadge support={state.capabilities.curves} />}>
      <SupportReason support={state.capabilities.curves} />
      <div className="mt-4 flex flex-col gap-4">
        <div className="flex flex-wrap items-end gap-x-4 gap-y-3">
          <label className="flex min-w-0 flex-col gap-1">
            <span className="text-xs text-fg-muted">{t("curveEditing")}</span>
            <select value={draft.id ?? ""} onChange={(e) => onEdit(e.target.value || null)} className="h-8 min-w-44">
              <option value="">{t("curveNew")}</option>
              {users.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
          {state.fans.length > 0 && (
            <label className="flex min-w-0 flex-col gap-1">
              <span className="text-xs text-fg-muted">{t("curveTarget")}</span>
              <select value={target ?? ""} onChange={(e) => onTarget(e.target.value || null)} className="h-8 min-w-40">
                <option value="">{t("curveAllFans")}</option>
                {state.fans.map((f) => (
                  <option key={f.id} value={f.id}>
                    {f.label}
                  </option>
                ))}
              </select>
            </label>
          )}
          <div className="flex min-w-0 flex-col gap-1">
            <span className="text-xs text-fg-muted" id={`${nameId}-presets`}>
              {t("curvePresets")}
            </span>
            <div role="group" aria-labelledby={`${nameId}-presets`} className="flex flex-wrap gap-2">
              {PRESET_IDS.map((id) => (
                <Button
                  key={id}
                  size="sm"
                  variant={preset === id ? "primary" : "secondary"}
                  aria-pressed={preset === id}
                  onClick={() => {
                    setPoints(PRESETS[id].map((p) => ({ ...p })));
                    setFocused(null);
                  }}
                >
                  {t(`preset_${id}`)}
                </Button>
              ))}
            </div>
          </div>
        </div>

        <div className="surface-inset p-3 pe-4">
          <CurveChart points={points} onChange={setPoints} focused={focused} onFocusPoint={setFocused} onRemovePoint={remove} markerC={state.cpuTemperatureC} />
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <Button size="sm" icon="plus" onClick={add} disabled={points.length >= MAX_POINTS}>
              {t("curveAddPoint")}
            </Button>
            <Button size="sm" icon="minus" onClick={() => focused !== null && remove(focused)} disabled={focused === null || points.length <= MIN_POINTS}>
              {t("curveRemovePoint")}
            </Button>
            <span className="text-xs text-fg-subtle tabular">{t.plural("curvePointsCount", points.length)}</span>
          </div>
          <div id={issuesId} aria-live="polite" className="text-xs">
            {issues.length === 0 ? (
              <span className="flex items-center gap-1.5 text-fg-muted">
                <Icon name="check" size={14} className="text-ok" />
                {t("curveValid")}
              </span>
            ) : (
              <ul className="flex flex-col gap-0.5 text-warn">
                {issues.map((i, n) => (
                  <li key={n} className="flex items-center gap-1.5">
                    <Icon name="warning" size={14} />
                    {issueText(i)}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>

        {policySupported && (
          <fieldset className="flex flex-col gap-2 border-t border-line pt-4">
            <legend className="mb-2 text-xs text-fg-muted">{t("draftPolicy")}</legend>
            <div className="flex flex-wrap gap-x-6 gap-y-3">
              {SUPPLIES.map((s) => (
                <div key={s} className="flex items-center gap-3">
                  <span className="w-24 text-sm text-fg">{t(`supply_${s}`)}</span>
                  <Segmented<PolicyChoice>
                    size="sm"
                    label={`${t("draftPolicy")}: ${t(`supply_${s}`)}`}
                    options={policyOptions}
                    value={policyFor(draft.policy, s) ?? "system"}
                    onChange={(v) => onDraft({ ...draft, policy: withPolicy(draft.policy, s, v === "system" ? null : v) })}
                  />
                </div>
              ))}
            </div>
          </fieldset>
        )}

        <div className="flex flex-wrap items-end justify-between gap-3 border-t border-line pt-4">
          <label htmlFor={nameId} className="flex min-w-0 flex-1 flex-col gap-1 sm:max-w-xs">
            <span className="text-xs text-fg-muted">{t("profileName")}</span>
            <input
              id={nameId}
              type="text"
              value={draft.name}
              maxLength={NAME_MAX + 10}
              placeholder={t("profileNamePlaceholder")}
              onChange={(e) => onDraft({ ...draft, name: e.target.value })}
              aria-invalid={draft.name.length > 0 && name !== null}
              className="h-8"
            />
            {draft.name.length > 0 && name !== null && <span className="text-xs text-warn">{name === "required" ? t("nameRequired") : t("nameTooLong", { max: NAME_MAX })}</span>}
          </label>
          <div className="flex flex-wrap items-center gap-2">
            {draft.id !== null && (
              <Button variant="ghost" icon="close" onClick={() => setConfirmDelete(true)} disabled={busy}>
                {t("delete")}
              </Button>
            )}
            <Button icon="download" disabled={!canSave} onClick={() => onSave(false)}>
              {t("save")}
            </Button>
            {blocked !== null ? (
              <Tooltip content={blocked || t("savedNotApplicable")} multiline side="top-end">
                <span tabIndex={0} className="rounded-full">
                  {saveApply}
                </span>
              </Tooltip>
            ) : (
              saveApply
            )}
          </div>
        </div>
        {blocked !== null && <p className="-mt-2 text-end text-xs text-fg-muted">{t("savedNotApplicable")}</p>}
      </div>

      <Modal
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        title={t("deleteTitle", { name: users.find((p) => p.id === draft.id)?.name ?? "" })}
        icon="warning"
        size="sm"
        footer={
          <>
            <Button onClick={() => setConfirmDelete(false)}>{t("cancel")}</Button>
            <Button
              variant="danger"
              onClick={() => {
                setConfirmDelete(false);
                onDelete();
              }}
            >
              {t("delete")}
            </Button>
          </>
        }
      >
        <p className="text-base text-fg-muted">{t("deleteBody")}</p>
      </Modal>
    </Panel>
  );
}
