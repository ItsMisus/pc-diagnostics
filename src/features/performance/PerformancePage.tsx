import { AnimatePresence, m } from "motion/react";
import { useEffect, useId, useMemo, useState } from "react";
import { useAppStore } from "@/app/store";
import { sectionLabel } from "@/app/sections";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { Modal, ModalBody, ModalFooter } from "@/components/common/Modal";
import { Panel } from "@/components/common/Panel";
import { Skeleton, SkeletonLines } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { Tag } from "@/components/common/Tag";
import { toast } from "@/components/common/Toast";
import { Tooltip } from "@/components/common/Tooltip";
import { type ProfileOption, ProfileSwitcher } from "@/components/control/ProfileSwitcher";
import { SupportBadge, SupportReason } from "@/components/control/SupportBadge";
import { PageHeader } from "@/components/layout/PageHeader";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { type Translator, useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatNumber } from "@/lib/format";
import { duration, ease } from "@/lib/motion";
import type { PerformanceControl } from "@/types/generated/PerformanceControl";
import type { PerformanceOsSupply } from "@/types/generated/PerformanceOsSupply";
import type { PerformanceProfile } from "@/types/generated/PerformanceProfile";
import type { PerformanceState } from "@/types/generated/PerformanceState";
import { CoreStage } from "./CoreStage";
import {
  controlList,
  displayedMode,
  draftOf,
  effectiveProfileId,
  findProfile,
  newDraft,
  osSupply,
  profileApplicable,
  profileIcon,
  type ProfileDraft,
  settingsFor,
} from "./model";
import { ProfileEditor } from "./ProfileEditor";
import { usePerformance } from "./store";

/** OS state can change outside the app (supply, System Settings): re-read while visible. */
const REFRESH_MS = 10_000;

const stripIn = {
  initial: { opacity: 0, y: -4 },
  animate: { opacity: 1, y: 0, transition: { duration: duration.component, ease: ease.out } },
  exit: { opacity: 0, transition: { duration: duration.exit, ease: ease.in } },
} as const;

/**
 * Performance center: the power core (focal, the mode in effect), the profiles (the three
 * modes and the user's own; choosing one applies it), what this computer lets the app control,
 * and "Ripristina". Nothing changes without a click.
 */
export function PerformancePage() {
  const t = useT("performance");
  const status = usePerformance((s) => s.status);
  const data = usePerformance((s) => s.data);
  const loadError = usePerformance((s) => s.loadError);
  const load = usePerformance((s) => s.load);
  const refresh = usePerformance((s) => s.refresh);
  const visible = useAppStore((s) => s.section === "performance");
  const [confirmRestore, setConfirmRestore] = useState(false);

  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    if (!visible) return;
    const id = setInterval(() => {
      if (!document.hidden) void refresh();
    }, REFRESH_MS);
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => {
      clearInterval(id);
      window.removeEventListener("focus", onFocus);
    };
  }, [visible, refresh]);

  return (
    <>
      <PageHeader
        title={sectionLabel("performance")}
        actions={
          data && (
            <Tooltip content={t("restoreNothing")} side="bottom-end" enabled={!data.canRestore} multiline>
              <span tabIndex={data.canRestore ? -1 : 0} className="rounded-full">
                <Button icon="undo" disabled={!data.canRestore} onClick={() => setConfirmRestore(true)}>
                  {t("restore")}
                </Button>
              </span>
            </Tooltip>
          )
        }
      />
      {status === "error" && loadError ? (
        <StateView kind={stateKindForError(loadError)} layout="hero" title={t("loadFailed")} error={loadError} onRetry={() => void load()} />
      ) : data ? (
        <Content data={data} paused={!visible} />
      ) : (
        <LoadingSkeleton />
      )}
      {data && <RestoreDialog open={confirmRestore} onClose={() => setConfirmRestore(false)} />}
    </>
  );
}

function LoadingSkeleton() {
  return (
    <div className="flex flex-col gap-10" role="status" aria-busy="true">
      <div className="surface-primary grid grid-cols-1 gap-6 rounded-2xl! p-6 md:grid-cols-2">
        <Skeleton className="aspect-[4/3] w-full rounded-xl" />
        <SkeletonLines lines={4} />
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {[0, 1, 2].map((i) => (
          <Skeleton key={i} className="h-20 rounded-2xl" />
        ))}
      </div>
    </div>
  );
}

function Content({ data, paused }: { data: PerformanceState; paused: boolean }) {
  const t = useT("performance");
  const [editor, setEditor] = useState<{ draft: ProfileDraft; profile: PerformanceProfile | null } | null>(null);
  const [deleting, setDeleting] = useState<PerformanceProfile | null>(null);
  const apply = usePerformance((s) => s.apply);
  const effective = findProfile(data, effectiveProfileId(data));

  const openNew = () => setEditor({ draft: newDraft(t("defaultName"), effective), profile: null });
  const openEdit = (p: PerformanceProfile) => setEditor({ draft: draftOf(p), profile: p });

  return (
    <div className="@container flex flex-col gap-10">
      <AnimatePresence initial={false}>
        {data.gameOverride && (
          <m.div key="override" {...stripIn} role="status" className="glass flex items-center gap-3 rounded-2xl px-4 py-3">
            <span aria-hidden="true" className="satin grid size-8 shrink-0 place-items-center rounded-full text-accent-text">
              <Icon name="gamepad" size={16} />
            </span>
            <div className="min-w-0">
              <p className="truncate text-sm font-medium text-fg">{t("overrideActive", { name: data.gameOverride.profileName })}</p>
              <p className="text-xs text-fg-muted">{t("overrideHint")}</p>
            </div>
          </m.div>
        )}
      </AnimatePresence>

      <Hero data={data} paused={paused} />

      <Profiles data={data} onNew={openNew} onEdit={openEdit} apply={apply} />

      <Capabilities data={data} />

      {editor && (
        <ProfileEditor
          open
          draft={editor.draft}
          hasBattery={data.hasBattery}
          capabilities={data.capabilities}
          currentSupply={data.supply}
          onClose={(savedId) => {
            const isNew = editor.profile === null;
            setEditor(null);
            if (savedId === null) return;
            toast.success(t("saved"));
            if (isNew) void apply(savedId);
          }}
          {...(editor.profile ? { onDelete: () => setDeleting(editor.profile) } : {})}
        />
      )}
      <DeleteDialog
        profile={deleting}
        onClose={(deleted) => {
          setDeleting(null);
          if (deleted) setEditor(null);
        }}
      />
    </div>
  );
}

// ---------------------------------------------------------------- Hero

function Hero({ data, paused }: { data: PerformanceState; paused: boolean }) {
  const t = useT("performance");
  const headingId = useId();
  const { mode, source } = displayedMode(data);
  const effective = findProfile(data, effectiveProfileId(data));
  const modeName = mode ? t(`mode_${mode}`) : t("modeUnknown");
  const openSettings = usePerformance((s) => s.openSettings);
  const settingsError = usePerformance((s) => (s.actionError?.action === "openSettings" ? s.actionError.error : null));
  const modeSupport = data.capabilities.mode;

  return (
    <section aria-labelledby={headingId} className="surface-primary relative grid grid-cols-1 rounded-2xl! @3xl:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)]">
      <div className="relative aspect-[16/10] max-h-[400px] w-full min-w-0 @3xl:aspect-auto @3xl:max-h-none @3xl:min-h-[340px]">
        <CoreStage mode={mode} paused={paused} label={mode ? t("stageLabel", { mode: modeName }) : t("stageLabelUnknown")}>
          <span className="satin absolute start-3 top-3 inline-flex h-7 items-center gap-1.5 rounded-full px-2.5 text-xs text-fg-muted">
            <Icon name={data.supply === "battery" ? "battery" : "power"} size={14} className="text-fg-subtle" />
            {t(`supplyNow_${data.supply}`)}
          </span>
        </CoreStage>
      </div>

      <div className="flex min-w-0 flex-col gap-5 p-5 pt-2 @3xl:p-6 @3xl:ps-2">
        <div className="flex flex-col gap-1">
          <p id={headingId} className="eyebrow">
            {t("heroLabel")}
          </p>
          <p className="font-display text-3xl font-semibold text-fg">{modeName}</p>
          <p className="text-sm text-fg-muted">
            {source === "profile" && effective ? t("sourceProfile", { name: effective.name }) : t("sourceSystem")}
          </p>
          {mode && <p className="text-xs text-fg-subtle">{t(`modeHint_${mode}`)}</p>}
        </div>

        <div className="flex flex-col gap-3 border-t border-line pt-5">
          <div className="flex items-start justify-between gap-3">
            <span className="text-sm text-fg">{t("control_mode")}</span>
            <SupportBadge support={modeSupport} />
          </div>
          {source === "system" && modeSupport.state !== "supported" && (
            <>
              <SupportReason support={modeSupport} />
              {modeSupport.state === "readOnly" && (
                <div>
                  <Button size="sm" icon="external" onClick={() => void openSettings()}>
                    {t("openBatterySettings")}
                  </Button>
                </div>
              )}
              {settingsError && <StateView kind="error" compact title={t("openSettingsFailed")} error={settingsError} />}
            </>
          )}
          <div className="flex items-center justify-between gap-3">
            <span className="text-sm text-fg">{t("keepAwakeNow")}</span>
            <span className="text-sm text-fg-muted">{t(`keepAwake_${data.os.keepAwake}`)}</span>
          </div>
          {data.os.schemeName && <p className="text-xs text-fg-subtle">{t("schemeName", { name: data.os.schemeName })}</p>}
          {!effective && <p className="text-xs text-fg-subtle">{t("noneSelected")}</p>}
        </div>

        {data.issues.length > 0 && (
          <div className="surface-inset flex flex-col gap-1.5" role="status">
            <p className="flex items-center gap-1.5 text-sm font-medium text-fg">
              <Icon name="warning" size={14} className="text-warn" />
              {t("issuesTitle")}
            </p>
            <ul className="flex flex-col gap-1 text-xs text-fg-muted">
              {data.issues.map((i, n) => (
                <li key={n}>
                  <span className="text-fg">{t(`control_${i.control}`)}</span>: {i.message}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------- Profiles

function profileHint(p: PerformanceProfile, data: PerformanceState, t: Translator<"performance">): string {
  const s = settingsFor(p, data.supply);
  const parts: string[] = [];
  if (data.capabilities.mode.state === "supported") parts.push(t(`modeHint_${s.mode}`));
  if (s.keepAwake !== "off" && data.capabilities.keepAwake.state === "supported") parts.push(t(`hintKeepAwake_${s.keepAwake}`));
  if (parts.length === 0) return t("hintNothingHere");
  const text = parts.join(" · ");
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function Profiles({
  data,
  onNew,
  onEdit,
  apply,
}: {
  data: PerformanceState;
  onNew: () => void;
  onEdit: (p: PerformanceProfile) => void;
  apply: (id: string) => Promise<boolean>;
}) {
  const t = useT("performance");
  const busy = usePerformance((s) => s.busy);
  const applyError = usePerformance((s) => (s.actionError?.action === "apply" ? s.actionError.error : null));
  const selected = data.gameOverride ? data.gameOverride.profileId : data.selectedProfileId;
  const selectedProfile = findProfile(data, data.selectedProfileId);

  const options = useMemo<ProfileOption[]>(
    () =>
      data.profiles.map((p) => {
        const applicable = profileApplicable(data.capabilities, p);
        return {
          id: p.id,
          name: p.name,
          hint: profileHint(p, data, t),
          icon: profileIcon(p),
          ...(p.builtin ? {} : { badge: <Tag>{t("custom")}</Tag> }),
          ...(applicable ? {} : { disabled: true, disabledReason: t("notApplicable") }),
        };
      }),
    [data, t],
  );

  return (
    <section className="flex flex-col gap-4">
      <SectionHeading
        aside={
          selectedProfile && !selectedProfile.builtin ? (
            <Button variant="ghost" size="sm" icon="sliders" onClick={() => onEdit(selectedProfile)}>
              {t("editProfile")}
            </Button>
          ) : undefined
        }
      >
        {t("profilesHeading")}
      </SectionHeading>
      <ProfileSwitcher
        label={t("profilesLabel")}
        options={options}
        value={selected}
        onChange={(id) => {
          if (!busy) void apply(id);
        }}
        trailing={
          <button
            type="button"
            onClick={onNew}
            className="group flex w-full min-w-0 items-start gap-3 rounded-2xl border border-dashed border-line-strong p-4 text-start transition-colors duration-200 hover:bg-control-hover active:scale-[0.985]"
          >
            <span aria-hidden="true" className="satin grid size-10 shrink-0 place-items-center rounded-full text-fg-muted group-hover:text-fg">
              <Icon name="plus" size={20} />
            </span>
            <span className="min-w-0 flex-1">
              <span className="block truncate font-display text-md font-semibold text-fg">{t("newProfile")}</span>
              <span className="mt-0.5 block text-xs text-fg-muted">{t("newProfileHint")}</span>
            </span>
          </button>
        }
      />
      {applyError && <StateView kind="error" compact title={t("applyFailed")} error={applyError} />}
    </section>
  );
}

// ---------------------------------------------------------------- Capabilities

function valueText(control: PerformanceControl, v: PerformanceOsSupply | null, t: Translator<"performance">): string | null {
  if (!v) return null;
  const minutes = (s: number | null) => (s === null ? null : s === 0 ? t("never") : t("minutes", { n: formatNumber(Math.round(s / 60)) }));
  switch (control) {
    case "mode":
      return v.mode ? t(`mode_${v.mode}`) : null;
    case "processorState":
      return v.processorMinPercent !== null && v.processorMaxPercent !== null
        ? `${t("percent", { n: formatNumber(v.processorMinPercent) })} – ${t("percent", { n: formatNumber(v.processorMaxPercent) })}`
        : null;
    case "boostMode":
      return v.boostMode ? t(`boost_${v.boostMode}`) : null;
    case "displayTimeout":
      return minutes(v.displayOffSeconds);
    case "sleepTimeout":
      return minutes(v.sleepSeconds);
    case "keepAwake":
      return null;
  }
}

function Capabilities({ data }: { data: PerformanceState }) {
  const t = useT("performance");
  const supplies = data.hasBattery ? (["ac", "battery"] as const) : (["ac"] as const);
  return (
    <Panel title={t("capsTitle")} icon="shield">
      <ul className="flex flex-col divide-y divide-line">
        {controlList(data.capabilities).map(({ control, support }) => {
          const values = supplies
            .map((s) => ({ s, v: control === "keepAwake" ? (s === data.supply ? t(`keepAwake_${data.os.keepAwake}`) : null) : valueText(control, osSupply(data, s), t) }))
            .filter((x): x is { s: "ac" | "battery"; v: string } => x.v !== null);
          return (
            <li key={control} className="flex flex-col gap-1 py-3 first:pt-0 last:pb-0 sm:flex-row sm:items-start sm:justify-between sm:gap-6">
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium text-fg">{t(`control_${control}`)}</p>
                <p className="text-xs text-fg-muted">{t(`controlHint_${control}`)}</p>
                <SupportReason support={support} />
              </div>
              <div className="flex shrink-0 flex-col items-start gap-1 sm:items-end">
                <SupportBadge support={support} />
                {values.map(({ s, v }) => (
                  <span key={s} className="inline-flex items-center gap-1 text-xs text-fg-subtle tabular">
                    <Icon name={s === "battery" ? "battery" : "power"} size={12} aria-hidden="true" />
                    <span className="sr-only">{t(`supply_${s}`)}</span>
                    {t("nowValue", { value: v })}
                  </span>
                ))}
              </div>
            </li>
          );
        })}
      </ul>
    </Panel>
  );
}

// ---------------------------------------------------------------- Dialogs

function RestoreDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT("performance");
  const restore = usePerformance((s) => s.restore);
  const busy = usePerformance((s) => s.busy?.action === "restore");
  const error = usePerformance((s) => (s.actionError?.action === "restore" ? s.actionError.error : null));
  return (
    <Modal open={open} onClose={onClose} title={t("restoreTitle")} icon="undo" size="sm" dismissible={!busy} bare>
      <ModalBody className="flex flex-col gap-3">
        <p className="text-base text-fg-muted">{t("restoreBody")}</p>
        {error && <StateView kind="error" compact title={t("restoreFailed")} error={error} />}
      </ModalBody>
      <ModalFooter>
        <Button onClick={onClose} disabled={busy}>
          {t("cancel")}
        </Button>
        <Button
          variant="primary"
          loading={busy}
          onClick={async () => {
            if (await restore()) {
              toast.success(t("restored"));
              onClose();
            }
          }}
        >
          {t("restoreConfirm")}
        </Button>
      </ModalFooter>
    </Modal>
  );
}

function DeleteDialog({ profile, onClose }: { profile: PerformanceProfile | null; onClose: (deleted: boolean) => void }) {
  const t = useT("performance");
  const remove = usePerformance((s) => s.remove);
  const busy = usePerformance((s) => s.busy?.action === "delete");
  const error = usePerformance((s) => (s.actionError?.action === "delete" ? s.actionError.error : null));
  return (
    <Modal open={profile !== null} onClose={() => onClose(false)} title={profile ? t("deleteTitle", { name: profile.name }) : ""} icon="warning" size="sm" dismissible={!busy} bare>
      <ModalBody className="flex flex-col gap-3">
        <p className="text-base text-fg-muted">{t("deleteBody")}</p>
        {error && <StateView kind="error" compact title={t("deleteFailed")} error={error} />}
      </ModalBody>
      <ModalFooter>
        <Button onClick={() => onClose(false)} disabled={busy}>
          {t("cancel")}
        </Button>
        <Button
          variant="danger"
          loading={busy}
          onClick={async () => {
            if (profile && (await remove(profile.id))) {
              toast.success(t("deleted"));
              onClose(true);
            }
          }}
        >
          {t("delete")}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
