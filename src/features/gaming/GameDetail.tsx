import { AnimatePresence, m } from "motion/react";
import { useId, useState } from "react";
import { Button } from "@/components/common/Button";
import { DefinitionList, type DefinitionItem } from "@/components/common/DefinitionList";
import { Disclosure } from "@/components/common/Disclosure";
import { Icon } from "@/components/common/Icon";
import { Modal, ModalBody, ModalFooter } from "@/components/common/Modal";
import { StatusBadge } from "@/components/common/StatusBadge";
import { Switch } from "@/components/common/Switch";
import { Tag } from "@/components/common/Tag";
import { Tooltip } from "@/components/common/Tooltip";
import { SupportBadge, SupportReason } from "@/components/control/SupportBadge";
import { useT } from "@/i18n";
import { formatDateTime, formatStorageBytes } from "@/lib/format";
import { duration, ease } from "@/lib/motion";
import type { AppError } from "@/types/generated/AppError";
import type { ControlDomainId } from "@/types/generated/ControlDomainId";
import type { GameEntry } from "@/types/generated/GameEntry";
import type { GameProfile } from "@/types/generated/GameProfile";
import type { GamingState } from "@/types/generated/GamingState";
import { coverBackground } from "./cover";
import { GameCover } from "./GameCover";
import { useGameArt } from "./gamingStore";
import { DOMAIN_ICON, canPlayWithProfile, domainChoices, profileFor, withLink, type DomainChoice } from "./model";

/**
 * Detail dialog of one game: hero backdrop with the cover, the two play actions, the game
 * profile (one selector per center, fed by the profiles the centers publish) and the
 * technical details folded under "Dettagli".
 */
export function GameDetail({
  game,
  state,
  busy,
  error,
  onClose,
  onPlay,
  onSaveProfile,
  onHide,
  onRemove,
}: {
  game: GameEntry | null;
  state: GamingState;
  busy: string | null;
  error: AppError | null;
  onClose: () => void;
  onPlay: (game: GameEntry, withProfile: boolean) => void;
  onSaveProfile: (profile: GameProfile) => void;
  onHide: (game: GameEntry, hidden: boolean) => void;
  onRemove: (game: GameEntry) => void;
}) {
  const titleId = useId();
  // Keep the last game while the dialog animates out.
  const [shown, setShown] = useState<GameEntry | null>(game);
  if (game && game !== shown) setShown(game);
  const current = game ?? shown;
  return (
    <Modal
      open={game !== null}
      onClose={onClose}
      size="lg"
      labelledBy={titleId}
      closeOnBackdrop
      bare
      header={current ? <DetailHeader game={current} titleId={titleId} onClose={onClose} /> : undefined}
      onExitComplete={() => setShown(null)}
    >
      {current && (
        <DetailBody
          game={current}
          state={state}
          busy={busy}
          error={error}
          onPlay={onPlay}
          onSaveProfile={onSaveProfile}
          onHide={onHide}
          onRemove={onRemove}
        />
      )}
    </Modal>
  );
}

function DetailHeader({ game, titleId, onClose }: { game: GameEntry; titleId: string; onClose: () => void }) {
  const t = useT("gaming");
  const tc = useT("common");
  const hero = useGameArt(game.id, "hero", game.hasHero);
  return (
    <header className="relative isolate shrink-0 overflow-hidden border-b border-line">
      <div aria-hidden="true" className="absolute inset-0 -z-10">
        {hero ? (
          <img src={hero} alt="" draggable={false} className="size-full object-cover" />
        ) : (
          <div className="size-full opacity-60" style={{ backgroundImage: coverBackground(game.title) }} />
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-surface-raised via-surface-raised/70 to-transparent" />
      </div>
      <div className="flex items-end gap-5 px-6 pt-16 pb-5">
        <m.div
          initial={{ opacity: 0, y: 10, rotate: -2 }}
          animate={{ opacity: 1, y: 0, rotate: 0, transition: { duration: duration.panel, ease: ease.out } }}
          className="w-28 shrink-0 overflow-hidden rounded-lg shadow-xl ring-1 ring-line"
        >
          <GameCover game={game} size="sm" />
        </m.div>
        <div className="min-w-0 flex-1 pb-1">
          <h2 id={titleId} className="font-display text-2xl font-bold text-fg">
            {game.title}
          </h2>
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            <Tag>{t(`sourceName_${game.source}`)}</Tag>
            {game.hidden && <Tag>{t("hidden")}</Tag>}
            {game.lastPlayed !== null && <span className="text-xs text-fg-muted">{t("heroLastPlayed", { when: formatDateTime(game.lastPlayed) })}</span>}
          </div>
        </div>
      </div>
      <Button variant="ghost" size="sm" iconOnly icon="close" aria-label={tc("close")} onClick={onClose} className="glass-thick absolute top-3 right-3" />
    </header>
  );
}

function DetailBody({
  game,
  state,
  busy,
  error,
  onPlay,
  onSaveProfile,
  onHide,
  onRemove,
}: {
  game: GameEntry;
  state: GamingState;
  busy: string | null;
  error: AppError | null;
  onPlay: (game: GameEntry, withProfile: boolean) => void;
  onSaveProfile: (profile: GameProfile) => void;
  onHide: (game: GameEntry, hidden: boolean) => void;
  onRemove: (game: GameEntry) => void;
}) {
  const t = useT("gaming");
  const enabledId = useId();
  const profile = profileFor(state, game.id);
  const choices = domainChoices(state, profile);
  const launchable = game.launch.state === "supported";
  const withProfile = canPlayWithProfile(profile);
  const session = state.gameMode.session?.gameId === game.id ? state.gameMode.session : null;

  const playProfileButton = (
    <Button
      variant="primary"
      icon="sliders"
      disabled={!launchable || !withProfile}
      loading={busy === `launch:${game.id}:true`}
      onClick={() => onPlay(game, true)}
    >
      {t("playWithProfile")}
    </Button>
  );

  const details: DefinitionItem[] = [
    { label: t("detailSource"), value: t(`sourceName_${game.source}`) },
    { label: t("detailId"), value: <span className="font-mono text-xs">{game.id}</span> },
    { label: t("detailSourceId"), value: <span className="font-mono text-xs">{game.sourceId}</span> },
    ...(game.installPath ? [{ label: t("detailPath"), value: <span className="font-mono text-xs break-all">{game.installPath}</span> }] : []),
    ...(game.sizeBytes !== null ? [{ label: t("detailSize"), value: formatStorageBytes(game.sizeBytes) }] : []),
    ...(game.lastPlayed !== null ? [{ label: t("detailLastPlayed"), value: formatDateTime(game.lastPlayed) }] : []),
  ];

  return (
    <>
      <ModalBody className="space-y-6">
        <div className="flex flex-wrap items-center gap-2">
          <Button icon="play" disabled={!launchable} loading={busy === `launch:${game.id}:false`} onClick={() => onPlay(game, false)}>
            {t("play")}
          </Button>
          {withProfile || !launchable ? (
            playProfileButton
          ) : (
            <Tooltip content={t("playWithProfileDisabled")} side="bottom" multiline>
              <span tabIndex={0} className="rounded-full">
                {playProfileButton}
              </span>
            </Tooltip>
          )}
          {!launchable && <SupportBadge support={game.launch} />}
        </div>
        {error && <p className="text-xs text-bad">{error.message}</p>}

        {/* Activation feedback: the per-center outcome of this game's session. */}
        <AnimatePresence initial={false}>
          {session && session.domains.length > 0 && (
            <m.div
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0, transition: { duration: duration.component, ease: ease.out } }}
              exit={{ opacity: 0, transition: { duration: duration.exit } }}
              className="surface-inset p-3"
            >
              <p className="eyebrow mb-2">{t("activation")}</p>
              <ul className="space-y-1.5">
                {session.domains.map((d, i) => (
                  <m.li
                    key={d.domain}
                    initial={{ opacity: 0, scale: 0.96 }}
                    animate={{ opacity: 1, scale: 1, transition: { delay: 0.08 * i, duration: duration.component, ease: ease.out } }}
                    className="flex items-center gap-2 text-sm"
                  >
                    <Icon name={DOMAIN_ICON[d.domain]} size={14} className="text-fg-muted" />
                    <span className="min-w-0 flex-1 truncate">
                      {t(`domain_${d.domain}`)} · {d.profileName ?? d.profileId}
                    </span>
                    {d.applied ? <StatusBadge size="sm" glyph tone="ok" label={t("applied")} /> : <SupportBadge support={d.support} />}
                  </m.li>
                ))}
              </ul>
            </m.div>
          )}
        </AnimatePresence>

        <section aria-labelledby={`${enabledId}-h`}>
          <div className="mb-3 flex items-start justify-between gap-4">
            <div>
              <h3 id={`${enabledId}-h`} className="font-display text-md font-semibold text-fg">
                {t("detailProfile")}
              </h3>
              <p id={enabledId} className="mt-0.5 text-xs text-fg-muted">
                {t("profileEnabledHint")}
              </p>
            </div>
            <label className="flex shrink-0 items-center gap-2 text-sm text-fg-muted">
              <span className="hidden sm:inline">{t("profileEnabled")}</span>
              <Switch
                label={t("profileEnabled")}
                checked={profile.enabled}
                describedBy={enabledId}
                disabled={busy === "profile"}
                onChange={(enabled) => onSaveProfile({ ...profile, enabled })}
              />
            </label>
          </div>
          <div className={`grid gap-3 sm:grid-cols-3 transition-opacity duration-200 ${profile.enabled ? "" : "opacity-70"}`}>
            {choices.map((c) => (
              <DomainSelect key={c.domain} choice={c} disabled={busy === "profile"} onChange={(id) => onSaveProfile(withLink(profile, c.domain, id))} />
            ))}
          </div>
        </section>

        <Disclosure summary={t("details")}>
          <DefinitionList divided items={details} className="mt-2" />
        </Disclosure>
      </ModalBody>
      <ModalFooter>
        {game.source === "manual" && (
          <Button variant="ghost" icon="minus" loading={busy === `remove:${game.id}`} onClick={() => onRemove(game)}>
            {t("remove")}
          </Button>
        )}
        <Button variant="ghost" icon={game.hidden ? "eye" : "eyeOff"} loading={busy === `hide:${game.id}`} onClick={() => onHide(game, !game.hidden)}>
          {game.hidden ? t("unhide") : t("hide")}
        </Button>
      </ModalFooter>
    </>
  );
}

function DomainSelect({ choice, disabled, onChange }: { choice: DomainChoice; disabled: boolean; onChange: (id: string | null) => void }) {
  const t = useT("gaming");
  const id = useId();
  const center = t(`domain_${choice.domain}` as `domain_${ControlDomainId}`);
  const unavailable = !choice.connected || choice.options.length === 0;
  return (
    <div className="surface-inset flex min-w-0 flex-col gap-2 p-3">
      <label htmlFor={id} className="flex items-center gap-2 text-xs font-medium text-fg-muted">
        <Icon name={DOMAIN_ICON[choice.domain]} size={14} />
        {center}
      </label>
      {unavailable && choice.selected === null ? (
        <p className="text-xs text-fg-subtle">{t(choice.connected ? "domainNoProfiles" : "domainNotConnected", { center })}</p>
      ) : (
        <>
          <select
            id={id}
            value={choice.selected ?? ""}
            disabled={disabled || (unavailable && choice.selected === null)}
            onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
            className="w-full"
          >
            <option value="">{t("noLink")}</option>
            {choice.options.map((o) => (
              <option key={o.id} value={o.id}>
                {o.name}
              </option>
            ))}
            {choice.stale && choice.selected !== null && <option value={choice.selected}>{choice.selected}</option>}
          </select>
          {choice.stale && <SupportReason support={{ state: "unsupported", reason: t("staleProfile") }} />}
        </>
      )}
    </div>
  );
}
