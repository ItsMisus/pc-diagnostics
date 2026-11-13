import { useId, useMemo, useState } from "react";
import { Button } from "@/components/common/Button";
import { UnavailableReason, useUnavailabilityText } from "@/components/common/MetricValue";
import { StateView } from "@/components/common/StateView";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatMib, formatStorageBytes } from "@/lib/format";
import { useInventory } from "@/features/monitoring/inventoryStore";
import { mediaLabel } from "@/features/monitoring/labels";
import { volumeDisplayName } from "@/features/monitoring/selection";
import type { AppError } from "@/types/generated/AppError";
import type { BenchmarkCapability } from "@/types/generated/BenchmarkCapability";
import type { BenchmarkGpuTarget } from "@/types/generated/BenchmarkGpuTarget";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";
import { useBench } from "./benchStore";
import {
  buildRequest,
  componentOfKind,
  defaultDraft,
  DURATION_PRESETS,
  durationBounds,
  FILE_PRESETS,
  fileBounds,
  gpuStopCopy,
  isAvailable,
  isStorageKind,
  isTargetAvailable,
  kindLabel,
  presets,
  type RunDraft,
  resolvedFileMib,
  resolveGpuTarget,
  stateKindForReason,
  storageConsentCopy,
  threadLimit,
} from "./model";
import { INSET } from "./ui";

const EMPTY_VOLUMES: readonly VolumeInfo[] = [];
/** Native selects are styled globally: sizing only. Left padding only, the chevron owns the right. */
const SELECT = "h-8 w-full min-w-0 pl-2.5 tabular";
const FIELD = "flex min-w-0 flex-col gap-1.5 text-xs font-medium text-fg-muted";

/**
 * Description, parameters and Start for one test kind: the body of the test panel. Remount
 * (key) per kind. While a run of this component is in progress, Start gives way to the run
 * block the screen renders below (one action at a time).
 */
export function TestPanel({ cap }: { cap: BenchmarkCapability }) {
  const t = useT("benchmarks");
  const volumes = useInventory((s) => s.data?.volumes ?? EMPTY_VOLUMES);
  const logicalCores = useInventory((s) => s.data?.cpu.logicalCores ?? 0);
  // Primitive selectors: progress events do not re-render this panel.
  const activeKind = useBench((s) => s.activeRun?.kind ?? null);
  const starting = useBench((s) => s.starting);
  const startError = useBench((s) => (s.startError?.kind === cap.kind ? s.startError.error : null));
  const startRun = useBench((s) => s.startRun);

  const [draft, setDraft] = useState<RunDraft>(() => defaultDraft(cap, volumes));
  // The inventory may arrive after mount: fall back to the default volume until one is picked.
  // A reloaded capability may no longer list the picked GPU: fall back to its default target.
  const effective = useMemo<RunDraft>(() => {
    const storageVolume = draft.storageVolume ?? defaultDraft(cap, volumes).storageVolume;
    const gpuId = resolveGpuTarget(cap.gpuTargets, draft.gpuId)?.id ?? null;
    return storageVolume === draft.storageVolume && gpuId === draft.gpuId ? draft : { ...draft, storageVolume, gpuId };
  }, [draft, cap, volumes]);
  const built = useMemo(
    () => buildRequest(cap, effective, { logicalCores: Math.max(1, logicalCores), volumes }, t),
    [cap, effective, logicalCores, volumes, t],
  );

  const reasonId = useId();
  const reasonText = useUnavailabilityText();
  const available = isAvailable(cap);
  const busy = activeKind !== null;
  // The run of this component is shown (with Stop) right below: no Start next to it.
  const runningHere = activeKind !== null && componentOfKind(activeKind) === componentOfKind(cap.kind);
  const blockedReason = !available
    ? null
    : busy
      ? t("busyReason", { kind: kindLabel(activeKind, t) })
      : built.ok
        ? null
        : built.reason;

  const update = (patch: Partial<RunDraft>) =>
    setDraft((d) => ({ ...d, storageVolume: d.storageVolume ?? effective.storageVolume, ...patch }));

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-col gap-1.5">
        <p className="max-w-prose text-sm text-fg-muted">{cap.description}</p>
        {cap.measures.length > 0 && (
          <p className="text-xs text-fg-subtle">
            <span className="text-fg-muted">{t("monitoredMeasures")} </span>
            {cap.measures.join(" · ")}
          </p>
        )}
      </div>

      {!available && cap.availability.status === "unavailable" && (
        <div className="flex flex-col items-start gap-4 border-t border-line pt-5">
          <StateView
            compact
            kind={stateKindForReason(cap.availability.reason)}
            title={t("testUnavailable")}
            description={reasonText(cap.availability.reason)}
          />
          <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
            <Button variant="primary" size="lg" icon="play" disabled aria-describedby={reasonId}>
              {t("start")}
            </Button>
            <span id={reasonId} className="min-w-0 text-xs text-fg-subtle">
              {t("startDisabled", { reason: reasonText(cap.availability.reason).toLowerCase() })}
            </span>
          </div>
        </div>
      )}

      {available && (
        <div className="flex flex-col gap-5 border-t border-line pt-5">
          <Controls
            cap={cap}
            draft={effective}
            volumes={volumes}
            logicalCores={logicalCores}
            disabled={busy || starting}
            onChange={update}
          />
          {!runningHere && (
            <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
              <Button
                variant="primary"
                size="lg"
                icon="play"
                disabled={blockedReason !== null}
                loading={starting}
                aria-describedby={blockedReason ? reasonId : undefined}
                onClick={() => {
                  if (built.ok) void startRun(built.request);
                }}
              >
                {starting ? t("starting") : t("start")}
              </Button>
              {blockedReason && (
                <span id={reasonId} className="min-w-0 flex-1 basis-60 text-xs text-fg-muted">
                  {blockedReason}
                </span>
              )}
            </div>
          )}
          {startError && <StartError error={startError} />}
        </div>
      )}
    </div>
  );
}

function Controls({
  cap,
  draft,
  volumes,
  logicalCores,
  disabled,
  onChange,
}: {
  cap: BenchmarkCapability;
  draft: RunDraft;
  volumes: readonly VolumeInfo[];
  logicalCores: number;
  disabled: boolean;
  onChange: (patch: Partial<RunDraft>) => void;
}) {
  const t = useT("benchmarks");
  const tm = useT("monitoring");
  const idBase = useId();
  const selectedGpu = cap.gpuTargets.length > 0 ? resolveGpuTarget(cap.gpuTargets, draft.gpuId) : null;
  const dBounds = durationBounds(cap, selectedGpu);
  const fBounds = fileBounds(cap);
  const limit = threadLimit(cap, Math.max(1, logicalCores));
  const storage = isStorageKind(cap.kind);
  const fileMib = resolvedFileMib(cap, draft);
  const hasFields = dBounds !== null || limit !== null || storage || fBounds !== null;

  return (
    <div className="flex flex-col gap-4">
      {cap.gpuTargets.length > 0 && (
        <GpuTargetSelect targets={cap.gpuTargets} gpuId={draft.gpuId} disabled={disabled} onChange={onChange} />
      )}
      {hasFields && (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,12rem),1fr))] gap-x-4 gap-y-3">
          {dBounds && (
            <label className={FIELD}>
              {t("duration")}
              <select
                className={SELECT}
                disabled={disabled}
                value={draft.durationS ?? dBounds.def}
                onChange={(e) => onChange({ durationS: Number(e.target.value) })}
              >
                {presets(dBounds, DURATION_PRESETS).map((v) => (
                  <option key={v} value={v}>
                    {v === dBounds.def ? t("optionDefault", { value: `${v} s` }) : `${v} s`}
                  </option>
                ))}
              </select>
            </label>
          )}
          {limit !== null && (
            <label className={FIELD}>
              {t("threads")}
              <select
                className={SELECT}
                disabled={disabled}
                value={draft.threads ?? "all"}
                onChange={(e) => onChange({ threads: e.target.value === "all" ? null : Number(e.target.value) })}
              >
                <option value="all">{t("allLogicalProcessors")}</option>
                {Array.from({ length: limit }, (_, i) => i + 1).map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
          )}
          {storage && (
            <label className={`${FIELD} col-span-full sm:col-span-2`}>
              {t("volume")}
              <select
                className={SELECT}
                disabled={disabled || volumes.length === 0}
                value={draft.storageVolume ?? ""}
                onChange={(e) => onChange({ storageVolume: e.target.value, consent: false })}
              >
                {volumes.length === 0 && <option value="">{t("noVolumes")}</option>}
                {volumes.map((v) => (
                  <option key={v.mountPoint} value={v.mountPoint}>
                    {volumeDisplayName(v)} · {mediaLabel(v.media, tm)} ·{" "}
                    {t("volumeFree", { size: formatStorageBytes(v.availableBytes) })}
                    {v.removable ? ` · ${t("removable")}` : ""}
                  </option>
                ))}
              </select>
            </label>
          )}
          {fBounds && (
            <label className={FIELD}>
              {t("fileSize")}
              <select
                className={SELECT}
                disabled={disabled}
                value={fileMib ?? fBounds.def}
                onChange={(e) => onChange({ storageFileMib: Number(e.target.value), consent: false })}
              >
                {presets(fBounds, FILE_PRESETS).map((v) => (
                  <option key={v} value={v}>
                    {v === fBounds.def ? t("optionDefault", { value: formatMib(v) }) : formatMib(v)}
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
      )}

      {storage && (
        <div className={`${INSET} p-3`}>
          <label
            htmlFor={`${idBase}-consent`}
            className="flex cursor-pointer items-start gap-2.5 text-sm text-fg has-[:disabled]:cursor-default"
          >
            <input
              id={`${idBase}-consent`}
              type="checkbox"
              className="mt-0.5 shrink-0"
              checked={draft.consent}
              disabled={disabled}
              onChange={(e) => onChange({ consent: e.target.checked })}
            />
            <span className="min-w-0">{storageConsentCopy(fileMib, t)}</span>
          </label>
          {cap.maxBytesWritten !== null && (
            <p className="mt-1.5 pl-6.5 text-xs text-fg-muted tabular">
              {t("maxWritten", { size: formatStorageBytes(cap.maxBytesWritten) })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

/** Adapter picker for GPU tests. Unusable adapters stay listed, disabled, with their reason. */
function GpuTargetSelect({
  targets,
  gpuId,
  disabled,
  onChange,
}: {
  targets: readonly BenchmarkGpuTarget[];
  gpuId: string | null;
  disabled: boolean;
  onChange: (patch: Partial<RunDraft>) => void;
}) {
  const t = useT("benchmarks");
  const noteId = useId();
  const selected = resolveGpuTarget(targets, gpuId);
  const unusable = targets.filter((g) => !isTargetAvailable(g));
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <label className={FIELD}>
        {t("gpuToTest")}
        <select
          className={`${SELECT} max-w-md`}
          disabled={disabled || selected === null}
          value={selected?.id ?? ""}
          aria-describedby={noteId}
          onChange={(e) => onChange({ gpuId: e.target.value })}
        >
          {selected === null && <option value="">{t("noUsableGpu")}</option>}
          {targets.map((g) => (
            <option key={g.id} value={g.id} disabled={!isTargetAvailable(g)}>
              {g.isDefault ? t("optionDefault", { value: g.name }) : g.name}
              {isTargetAvailable(g) ? "" : ` · ${t("notUsable")}`}
            </option>
          ))}
        </select>
      </label>
      <div id={noteId} className="flex flex-col gap-0.5 text-xs">
        {selected && <p className="text-fg-muted tabular">{gpuStopCopy(selected, t)}</p>}
        {unusable.map((g) => (
          <p key={g.id} className="break-words text-fg-subtle">
            {g.name}: <TargetReason target={g} />
          </p>
        ))}
      </div>
    </div>
  );
}

function TargetReason({ target }: { target: BenchmarkGpuTarget }) {
  const t = useT("benchmarks");
  return target.availability.status === "unavailable" ? (
    <UnavailableReason reason={target.availability.reason} />
  ) : (
    <>{t("notUsableForTest")}</>
  );
}

function StartError({ error }: { error: AppError }) {
  const t = useT("benchmarks");
  if (error.code === "busy") {
    return <StateView compact kind="error" title={t("busyTitle")} description={t("busyDesc")} error={error} />;
  }
  if (error.code === "invalidInput") {
    return <StateView compact kind="error" title={t("rejectedTitle")} error={error} />;
  }
  return <StateView compact kind={stateKindForError(error)} title={t("startFailed")} error={error} />;
}
