import { useEffect, useId, useMemo, useState } from "react";
import { useAppStore } from "@/app/store";
import { sectionLabel } from "@/app/sections";
import { Button } from "@/components/common/Button";
import { Modal } from "@/components/common/Modal";
import { Panel } from "@/components/common/Panel";
import { Skeleton } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { Tooltip } from "@/components/common/Tooltip";
import { PageHeader } from "@/components/layout/PageHeader";
import { useBench } from "@/features/benchmarks/benchStore";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatPercent } from "@/lib/format";
import type { LightingDevice } from "@/types/generated/LightingDevice";
import { DevicesPanel } from "./DevicesPanel";
import { EffectGallery } from "./EffectGallery";
import { EffectParams } from "./EffectParams";
import { LightStage, type StageEntry } from "./LightStage";
import { ProfilesPanel } from "./ProfilesPanel";
import { STAGE_MODELS, modelForKind, type StageModel } from "./scene/layouts";
import { Slider } from "./Slider";
import { controllable, reachedDevices, sameSetup, useLighting } from "./store";

/** Stage entries: this computer's devices first, then generic models labelled as previews. */
function stageEntries(devices: readonly LightingDevice[], modelName: (m: StageModel) => string): StageEntry[] {
  const real = devices.filter(controllable).map((d) => ({ key: d.id, model: modelForKind(d.kind), name: d.name, preview: false }));
  const shown = new Set(real.map((r) => r.model));
  const previews = STAGE_MODELS.filter((m) => !shown.has(m)).map((m) => ({ key: `preview:${m}`, model: m, name: modelName(m), preview: true }));
  return [...real, ...previews];
}

/**
 * Lighting center (docs/control-centers.md, "Lighting"). The focal panel is the stage: a 3D
 * model of a device lit live by the effect being edited, next to brightness, the state of the
 * devices and "Applica". Below: the effect gallery with its parameters and color picker,
 * profiles, and the devices. Without a compatible device the page works as a preview only
 * (Applica disabled, saying why).
 */
export function LightingPage() {
  const t = useT("lighting");
  const status = useLighting((s) => s.status);
  const state = useLighting((s) => s.state);
  const loadError = useLighting((s) => s.loadError);
  const load = useLighting((s) => s.load);
  const refresh = useLighting((s) => s.refresh);
  const busy = useLighting((s) => s.busy);
  const [confirmRestore, setConfirmRestore] = useState(false);
  const restore = useLighting((s) => s.restore);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <>
      <PageHeader
        title={sectionLabel("lighting")}
        actions={
          state && (
            <>
              <Button variant="ghost" icon="refresh" loading={busy === "refresh"} onClick={() => void refresh()}>
                {t("refresh")}
              </Button>
              <Tooltip content={state.canRestore ? t("restoreHint") : t("restoreNothing")} side="bottom-end" multiline>
                <Button icon="undo" disabled={!state.canRestore} onClick={() => setConfirmRestore(true)}>
                  {t("restore")}
                </Button>
              </Tooltip>
            </>
          )
        }
      />

      {status === "error" && loadError && <StateView layout="hero" kind={stateKindForError(loadError)} title={t("loadFailed")} error={loadError} onRetry={() => void load()} />}
      {(status === "idle" || status === "loading") && !state && <LoadingShape />}
      {state && <Content />}

      <Modal
        open={confirmRestore}
        onClose={() => setConfirmRestore(false)}
        size="sm"
        icon="undo"
        title={t("restoreTitle")}
        footer={
          <>
            <Button onClick={() => setConfirmRestore(false)}>{t("cancel")}</Button>
            <Button
              variant="primary"
              loading={busy === "restore"}
              onClick={async () => {
                await restore();
                setConfirmRestore(false);
              }}
            >
              {t("restore")}
            </Button>
          </>
        }
      >
        <p className="text-base text-fg-muted">{t("restoreBody")}</p>
      </Modal>
    </>
  );
}

function LoadingShape() {
  const t = useT("lighting");
  return (
    <div role="status" aria-busy="true" className="flex flex-col gap-6">
      <span className="sr-only">{t("loading")}</span>
      <Skeleton className="h-[380px] w-full rounded-2xl" />
      <Skeleton className="h-48 w-full rounded-2xl" />
    </div>
  );
}

function Content() {
  const t = useT("lighting");
  const state = useLighting((s) => s.state)!;
  const draft = useLighting((s) => s.draft);
  const setDraft = useLighting((s) => s.setDraft);
  const setEffect = useLighting((s) => s.setEffect);
  const recent = useLighting((s) => s.recent);
  const rememberColor = useLighting((s) => s.rememberColor);
  const actionError = useLighting((s) => s.actionError);
  const clearError = useLighting((s) => s.clearError);

  const usable = useMemo(() => state.devices.filter(controllable), [state.devices]);
  const colorless = usable.length > 0 && usable.every((d) => !d.colorCapable);
  const modelName = (m: StageModel) => t(`model_${m}`);
  const entries = useMemo(() => stageEntries(state.devices, modelName), [state.devices, t]);
  const [stageKey, setStageKey] = useState<string | null>(null);
  const selectedKey = entries.some((e) => e.key === stageKey) ? stageKey! : (entries[0]?.key ?? "");
  const pageVisible = useAppStore((s) => s.section === "lighting");
  const benchmarkRunning = useBench((s) => s.activeRun !== null);

  return (
    <div className="@container flex flex-col gap-8">
      <section aria-label={t("stageSection")} className="surface-primary relative isolate grid grid-cols-1 overflow-hidden rounded-2xl! @3xl:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)]">
        <LightStage entries={entries} selected={selectedKey} onSelect={setStageKey} effect={draft.effect} brightness={draft.brightness} paused={!pageVisible || benchmarkRunning} />
        <ControlColumn usable={usable} />
      </section>

      {actionError && (
        <StateView compact kind={stateKindForError(actionError)} title={t("actionFailed")} error={actionError} action={<Button size="sm" variant="ghost" onClick={clearError}>{t("dismiss")}</Button>} />
      )}

      <div className="grid grid-cols-1 gap-4 @4xl:grid-cols-5">
        <Panel title={t("effectsTitle")} icon="sparkle" className="@container min-w-0 @4xl:col-span-3">
          <EffectGallery effect={draft.effect} brightness={draft.brightness} onChange={setEffect} colorless={colorless} />
        </Panel>
        <Panel title={t("paramsTitle")} icon="palette" className="min-w-0 @4xl:col-span-2">
          <EffectParams effect={draft.effect} onChange={setEffect} recent={recent} onRemember={rememberColor} colorNote={colorless ? t("colorIgnoredWhite") : null} />
        </Panel>
      </div>

      <div className="grid grid-cols-1 gap-4 @4xl:grid-cols-5">
        <div className="min-w-0 @4xl:col-span-3">
          <ProfilesPanel profiles={state.profiles} gameOverride={state.gameOverride} />
        </div>
        <div className="@container min-w-0 @4xl:col-span-2">
          <DevicesPanel devices={state.devices} setup={draft} onChange={setDraft} platformNote={state.platformNote} />
        </div>
      </div>
    </div>
  );
}

/** Right side of the hero: what is being edited, brightness, device state and the actions. */
function ControlColumn({ usable }: { usable: readonly LightingDevice[] }) {
  const t = useT("lighting");
  const state = useLighting((s) => s.state)!;
  const draft = useLighting((s) => s.draft);
  const profileId = useLighting((s) => s.profileId);
  const busy = useLighting((s) => s.busy);
  const apply = useLighting((s) => s.apply);
  const turnOff = useLighting((s) => s.turnOff);
  const setDraft = useLighting((s) => s.setDraft);
  const setBrightnessLive = useLighting((s) => s.setBrightnessLive);
  const brightnessHint = useId();

  const reached = reachedDevices(usable, draft);
  const profile = state.profiles.find((p) => p.id === profileId);
  const applied = state.applied;
  const upToDate = applied !== null && sameSetup(applied.setup, draft);
  const brightnessSupported = usable.length === 0 || reached.some((d) => d.brightness.state === "supported");
  const brightnessReason = usable.length > 0 && !brightnessSupported ? (reached[0]?.brightness.reason ?? usable[0]?.brightness.reason ?? t("brightnessNoDevice")) : null;

  let applyReason: string | null = null;
  if (usable.length === 0) applyReason = t("applyNoDevice");
  else if (reached.length === 0) applyReason = colorOnly(draft.effect.kind) && usable.every((d) => !d.colorCapable) ? t("applyWhiteOnly") : t("applyNoTarget");

  const applyButton = (
    <Button variant="primary" icon="check" disabled={applyReason !== null || upToDate} loading={busy === "apply"} onClick={() => void apply()}>
      {upToDate ? t("applied") : t("apply")}
    </Button>
  );

  return (
    <div className="flex min-w-0 flex-col gap-5 p-5 @3xl:p-6 @3xl:ps-2">
      <div className="flex flex-col gap-1">
        <p className="eyebrow">{profile ? profile.name : t("customSetup")}</p>
        <p className="font-display text-3xl font-semibold text-fg">{t(`effect_${draft.effect.kind}`)}</p>
        <p className="text-sm text-fg-muted">{t(`effectHint_${draft.effect.kind}`)}</p>
      </div>

      <div className="flex flex-col gap-2">
        <div className="flex items-baseline justify-between">
          <span className="text-sm text-fg">{t("brightness")}</span>
          <span className="text-xs text-fg-muted tabular">{formatPercent(draft.brightness)}</span>
        </div>
        <Slider
          label={t("brightness")}
          value={draft.brightness}
          min={0}
          max={100}
          valueText={formatPercent(draft.brightness)}
          disabled={!brightnessSupported}
          describedBy={brightnessReason ? brightnessHint : undefined}
          onChange={(brightness) => setDraft({ brightness })}
          onCommit={(b) => {
            if (applied && !busy) void setBrightnessLive(b);
          }}
        />
        {brightnessReason && (
          <p id={brightnessHint} className="text-xs text-fg-muted">
            {brightnessReason}
          </p>
        )}
      </div>

      <DeviceStatus usable={usable} reachedCount={reached.length} />

      <div className="mt-auto flex flex-wrap items-center gap-2 border-t border-line pt-5">
        {applyReason ? (
          <Tooltip content={applyReason} multiline side="top">
            <span tabIndex={0} className="rounded-full">
              {applyButton}
            </span>
          </Tooltip>
        ) : (
          applyButton
        )}
        <Button icon="power" disabled={usable.length === 0} loading={busy === "apply" && draft.effect.kind === "off"} onClick={() => void turnOff()}>
          {t("turnOff")}
        </Button>
      </div>
      {applyReason && <p className="-mt-3 text-xs text-fg-muted">{applyReason}</p>}
    </div>
  );
}

const colorOnly = (k: string) => k === "rainbow" || k === "wave" || k === "gradient" || k === "colorCycle";

function DeviceStatus({ usable, reachedCount }: { usable: readonly LightingDevice[]; reachedCount: number }) {
  const t = useT("lighting");
  const state = useLighting((s) => s.state)!;
  const applied = state.applied;
  const line =
    usable.length === 0
      ? t("statusPreviewOnly")
      : applied
        ? t.plural("statusApplied", applied.deviceIds.length)
        : t.plural("statusReady", reachedCount);
  return (
    <div className="flex flex-col gap-1.5 rounded-lg bg-surface-2 p-3">
      <p className="flex items-center gap-2 text-sm text-fg">
        <span aria-hidden="true" className={`size-2 shrink-0 rounded-full ${applied ? "bg-ok" : "bg-fg-subtle"}`} />
        {line}
      </p>
      {state.engineRunning && <p className="text-xs text-fg-muted">{t("statusEngine")}</p>}
      {!applied && usable.length > 0 && <p className="text-xs text-fg-subtle">{t("statusNothingChanged")}</p>}
    </div>
  );
}
