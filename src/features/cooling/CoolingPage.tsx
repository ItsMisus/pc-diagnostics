import { useCallback, useEffect, useId, useRef, useState } from "react";
import { sectionLabel } from "@/app/sections";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { Modal } from "@/components/common/Modal";
import { Panel } from "@/components/common/Panel";
import { Skeleton, SkeletonLines } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { Tag } from "@/components/common/Tag";
import { toast } from "@/components/common/Toast";
import { Tooltip } from "@/components/common/Tooltip";
import { ProfileSwitcher } from "@/components/control/ProfileSwitcher";
import { PageHeader } from "@/components/layout/PageHeader";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { CoolingState } from "@/types/generated/CoolingState";
import { coolingApi } from "./api";
import { CurvePanel } from "./CurvePanel";
import { type Draft, draftFromProfile, newDraft, profileName, profileOptions, toProfileDraft, userProfiles } from "./model";
import { CoolingGallery, CoolingHero, FansPanel, PolicyPanel } from "./Panels";

/** Fan speeds are re-read this often while fans with a measured speed are shown. */
const FAN_REFRESH_MS = 2000;

type Load = { status: "loading" } | { status: "ready"; data: CoolingState } | { status: "error"; error: AppError };

/**
 * Cooling center (docs/control-centers.md). Composition like the Overview:
 * 1. the hero, the focal element: this computer's cooling system in 3D, what it is and what
 *    the app can change here;
 * 2. the profiles (applied at once when the system allows it);
 * 3. the fans the system reports, the fan curve editor and the cooling policy;
 * 4. the other kinds of cooling as types, never as detected hardware.
 * Nothing changes without a user action; "Ripristina" puts back what was found first.
 */
export function CoolingPage() {
  const t = useT("cooling");
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [actionError, setActionError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmRestore, setConfirmRestore] = useState(false);
  const [draft, setDraft] = useState<Draft>(newDraft);
  const [target, setTarget] = useState<string | null>(null);
  const alive = useRef(true);

  const fetchState = useCallback(() => {
    setLoad({ status: "loading" });
    coolingApi.state().then(
      (data) => alive.current && setLoad({ status: "ready", data }),
      (e: unknown) => alive.current && setLoad({ status: "error", error: toAppError(e) }),
    );
  }, []);

  useEffect(() => {
    alive.current = true;
    fetchState();
    return () => {
      alive.current = false;
    };
  }, [fetchState]);

  const data = load.status === "ready" ? load.data : null;
  const liveFans = data?.fans.some((f) => f.currentRpm !== null) ?? false;

  // Live fan speeds: re-read quietly (no loading state) only while there is something to show.
  useEffect(() => {
    if (!liveFans) return;
    const id = setInterval(() => {
      coolingApi.state().then(
        (next) => alive.current && setLoad((cur) => (cur.status === "ready" ? { status: "ready", data: { ...cur.data, fans: next.fans, cpuTemperatureC: next.cpuTemperatureC } } : cur)),
        () => {},
      );
    }, FAN_REFRESH_MS);
    return () => clearInterval(id);
  }, [liveFans]);

  /** Runs a mutation; the returned state replaces the page's. Errors stay inline. */
  const run = async (fn: () => Promise<CoolingState>, done?: (s: CoolingState) => void, failTitle?: string): Promise<CoolingState | null> => {
    setBusy(true);
    setActionError(null);
    try {
      const s = await fn();
      if (!alive.current) return null;
      setLoad({ status: "ready", data: s });
      done?.(s);
      return s;
    } catch (e) {
      const err = toAppError(e);
      if (alive.current) setActionError(failTitle ? { ...err, message: `${failTitle}: ${err.message}` } : err);
      return null;
    } finally {
      if (alive.current) setBusy(false);
    }
  };

  const apply = (id: string) =>
    run(
      () => coolingApi.apply(id),
      (s) => toast.success(t("applied", { name: profileName(s, id) ?? id })),
      t("applyFailed"),
    );

  const save = async (andApply: boolean) => {
    if (!data) return;
    const before = new Set(userProfiles(data).map((p) => p.id));
    const s = await run(() => coolingApi.save(toProfileDraft(draft, data.fans.map((f) => f.id))), undefined, t("saveFailed"));
    if (!s) return;
    const saved = draft.id ?? userProfiles(s).find((p) => !before.has(p.id))?.id ?? null;
    const entry = s.profiles.find((e) => e.profile.id === saved);
    if (entry) setDraft(draftFromProfile(entry.profile));
    if (andApply && saved) await apply(saved);
    else toast.success(t("saved", { name: draft.name.trim() }), entry && !entry.applicable ? t("savedNotApplicable") : undefined);
  };

  const remove = () => {
    if (!draft.id) return;
    const id = draft.id;
    void run(
      () => coolingApi.remove(id),
      () => {
        setDraft(newDraft());
        toast.success(t("deleted"));
      },
    );
  };

  const edit = (id: string | null) => {
    const p = id && data ? userProfiles(data).find((x) => x.id === id) : undefined;
    setDraft(p ? draftFromProfile(p) : newDraft());
  };

  const restoreButton = (
    <Button icon="undo" disabled={!data?.canRestore || busy} onClick={() => setConfirmRestore(true)}>
      {t("restore")}
    </Button>
  );

  return (
    <>
      <PageHeader
        title={sectionLabel("cooling")}
        actions={
          data ? (
            <Tooltip content={data.canRestore ? t("restoreHint") : t("restoreNothing")} side="bottom-end" multiline>
              <span tabIndex={data.canRestore ? -1 : 0} className="rounded-full">
                {restoreButton}
              </span>
            </Tooltip>
          ) : null
        }
      />

      {load.status === "loading" && <LoadingView />}
      {load.status === "error" && (
        <Panel variant="primary">
          <StateView kind={stateKindForError(load.error)} title={t("loadError")} error={load.error} onRetry={fetchState} layout="hero" />
        </Panel>
      )}
      {data && (
        <div className="@container flex flex-col gap-10">
          <div className="flex flex-col gap-4">
            <CoolingHero state={data} />
            {data.gameOverride && <OverrideStrip name={profileName(data, data.gameOverride) ?? data.gameOverride} />}
            {actionError && (
              <Panel>
                <StateView kind={stateKindForError(actionError)} error={actionError} compact />
              </Panel>
            )}
          </div>

          <Profiles state={data} busy={busy} onApply={(id) => void apply(id)} />

          <div className="flex flex-col gap-4">
            {data.fans.length > 0 && <FansPanel fans={data.fans} target={target} onTarget={setTarget} />}
            <div className="grid grid-cols-1 gap-4 @4xl:grid-cols-[minmax(0,1.7fr)_minmax(0,1fr)]">
              <CurvePanel
                state={data}
                draft={draft}
                onDraft={setDraft}
                target={target}
                onTarget={setTarget}
                onEdit={edit}
                onSave={(a) => void save(a)}
                onDelete={remove}
                busy={busy}
              />
              <PolicyPanel state={data} className="self-start" />
            </div>
          </div>

          <CoolingGallery state={data} />
        </div>
      )}

      <Modal
        open={confirmRestore}
        onClose={() => setConfirmRestore(false)}
        title={t("restoreTitle")}
        icon="undo"
        size="sm"
        footer={
          <>
            <Button onClick={() => setConfirmRestore(false)}>{t("cancel")}</Button>
            <Button
              variant="primary"
              onClick={() => {
                setConfirmRestore(false);
                void run(() => coolingApi.restore(), () => toast.success(t("restoreDone")));
              }}
            >
              {t("restoreConfirm")}
            </Button>
          </>
        }
      >
        <p className="text-base text-fg-muted">{t("restoreBody")}</p>
      </Modal>
    </>
  );
}

function Profiles({ state, busy, onApply }: { state: CoolingState; busy: boolean; onApply: (id: string) => void }) {
  const t = useT("cooling");
  const headingId = useId();
  const options = profileOptions(state, (mode) => t(`profileHint_${mode}`)).map(({ builtin, ...o }) => ({
    ...o,
    ...(builtin ? {} : { badge: <Tag>{t("badgeUser")}</Tag> }),
    ...(busy ? { disabled: true } : {}),
  }));
  const active = state.profiles.find((e) => e.profile.id === state.active);
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-3">
      <SectionHeading id={headingId}>{t("profiles")}</SectionHeading>
      <ProfileSwitcher label={t("profilesLabel")} options={options} value={state.active} onChange={onApply} columns={options.length >= 4 ? 4 : 3} />
      {active?.note && <p className="text-xs text-fg-muted">{active.note}</p>}
    </section>
  );
}

function OverrideStrip({ name }: { name: string }) {
  const t = useT("cooling");
  return (
    <Panel variant="glass" className="flex items-center gap-3">
      <span aria-hidden="true" className="satin grid size-9 shrink-0 place-items-center rounded-full text-accent-text">
        <Icon name="gamepad" size={18} />
      </span>
      <div className="min-w-0">
        <p className="text-sm font-medium text-fg">{t("overrideTitle")}</p>
        <p className="text-xs text-fg-muted">{t("overrideBody", { game: name })}</p>
      </div>
    </Panel>
  );
}

function LoadingView() {
  const t = useT("cooling");
  return (
    <div className="flex flex-col gap-10" role="status" aria-busy="true">
      <span className="sr-only">{t("loadingLabel")}</span>
      <div className="surface-primary grid grid-cols-1 gap-6 rounded-2xl! p-6 @3xl:grid-cols-2">
        <Skeleton className="aspect-[16/10] w-full rounded-xl" />
        <div className="flex flex-col gap-4">
          <Skeleton className="h-4 w-24" />
          <Skeleton className="h-8 w-64" />
          <SkeletonLines lines={3} />
        </div>
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        {[0, 1, 2].map((i) => (
          <Skeleton key={i} className="h-20 rounded-2xl" />
        ))}
      </div>
      <Skeleton className="h-80 rounded-2xl" />
    </div>
  );
}
