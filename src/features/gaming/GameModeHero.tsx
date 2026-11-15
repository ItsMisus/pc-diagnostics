import { AnimatePresence, m } from "motion/react";
import { useEffect, useId, useState } from "react";
import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { StatusBadge } from "@/components/common/StatusBadge";
import { Switch } from "@/components/common/Switch";
import { SupportBadge } from "@/components/control/SupportBadge";
import { type Translator, useT } from "@/i18n";
import { formatDateTime, formatTime } from "@/lib/format";
import { duration, ease } from "@/lib/motion";
import type { GameEntry } from "@/types/generated/GameEntry";
import type { GamingState } from "@/types/generated/GamingState";
import { coverBackground } from "./cover";
import { GameCover } from "./GameCover";
import { useGameArt } from "./gamingStore";
import { DOMAIN_ICON, featuredGame, sessionSummary } from "./model";

/**
 * Focal panel of the page: game mode over the featured game's wide art (the game being
 * played, else the last one played). The art sits under a scrim; every control is on a glass
 * plate so text never sits directly on the picture.
 */
export function GameModeHero({
  state,
  busy,
  onToggleMode,
  onStop,
  onOpen,
  onPlay,
}: {
  state: GamingState;
  busy: string | null;
  onToggleMode: (enabled: boolean) => void;
  onStop: () => void;
  onOpen: (game: GameEntry) => void;
  onPlay: (game: GameEntry) => void;
}) {
  const t = useT("gaming");
  const hintId = useId();
  const featured = featuredGame(state);
  const session = state.gameMode.session;
  const hero = useGameArt(featured?.id ?? null, "hero", Boolean(featured?.hasHero));
  const logo = useGameArt(featured?.id ?? null, "logo", Boolean(featured?.hasLogo));
  const summary = sessionSummary(state);
  const playing = Boolean(session?.detected);
  const now = useNow(session ? 30_000 : null);

  return (
    <section aria-labelledby={`${hintId}-title`} className="surface-primary relative isolate mb-8 min-h-[300px] overflow-hidden">
      {/* Backdrop: the game's hero art, else its own cover or icon blurred into a wash of its colours. */}
      <AnimatePresence initial={false}>
        <m.div
          key={featured?.id ?? "none"}
          aria-hidden="true"
          className="absolute inset-0 -z-10"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1, transition: { duration: duration.panel, ease: ease.out } }}
          exit={{ opacity: 0, transition: { duration: duration.exit } }}
        >
          {hero ? (
            <img src={hero} alt="" className="size-full object-cover" draggable={false} />
          ) : featured ? (
            <Wash game={featured} />
          ) : (
            <div className="ambient-canvas size-full" />
          )}
          <div className="absolute inset-0 bg-gradient-to-t from-canvas via-canvas/60 to-canvas/10" />
          {/* Second scrim behind the text column, so the eyebrow and lines stay readable over bright art in both themes. */}
          <div className="absolute inset-0 bg-gradient-to-r from-canvas/85 via-canvas/40 to-transparent" />
        </m.div>
      </AnimatePresence>

      <div className="flex min-h-[300px] flex-col justify-end gap-5 p-5 md:flex-row md:items-center md:justify-between md:p-6">
        {/* Featured game: cover, logo (Steam) or title, what is happening. */}
        <div className="flex min-w-0 items-center gap-5">
          {featured && (
            <button
              type="button"
              onClick={() => onOpen(featured)}
              className={`relative hidden w-28 shrink-0 rounded-xl transition-transform duration-200 hover:-translate-y-1 sm:block md:w-32 ${playing ? "playing-glow" : ""}`}
              aria-label={featured.title}
            >
              <span className={`block overflow-hidden rounded-xl shadow-lg ring-1 ${playing ? "ring-2 ring-ok" : "ring-line"}`}>
                <GameCover game={featured} size="sm" />
              </span>
            </button>
          )}
          <div className="min-w-0">
            {session ? (
              <span className="glass mb-3 inline-flex h-7 items-center gap-2 rounded-full px-3 text-xs font-medium text-fg">
                <span className={`relative flex size-2 ${session.detected ? "text-ok" : "text-warn"}`} aria-hidden="true">
                  {session.detected && <span className="absolute inset-0 rounded-full bg-current opacity-60 motion-safe:animate-ping" />}
                  <span className="relative size-2 rounded-full bg-current" />
                </span>
                {session.detected ? t("heroPlaying") : t("heroStarting")}
                {session.detected && <span className="tabular text-fg-muted">· {elapsedText(t, now - session.since)}</span>}
              </span>
            ) : (
              <p className="eyebrow mb-2">{t("gameMode")}</p>
            )}
            {featured && logo ? (
              <h2 id={`${hintId}-title`}>
                <span className="sr-only">{session ? session.title : featured.title}</span>
                <img src={logo} alt="" draggable={false} className="max-h-24 max-w-[min(22rem,60vw)] object-contain object-left drop-shadow-[0_4px_16px_rgb(0_0_0/0.35)]" />
              </h2>
            ) : (
              <h2 id={`${hintId}-title`} className="truncate font-display text-4xl font-bold tracking-tight text-fg">
                {session ? session.title : featured ? featured.title : t("heroIdle")}
              </h2>
            )}
            <p className="mt-1.5 text-sm text-fg-muted">
              {session
                ? `${t(session.trigger === "launch" ? "triggerLaunch" : "triggerDetected")} · ${t("since", { time: formatTime(session.since) })}`
                : featured?.lastPlayed
                  ? t("heroLastPlayed", { when: formatDateTime(featured.lastPlayed) })
                  : t("heroIdleHint")}
            </p>
            {!session && featured && featured.launch.state === "supported" && (
              <div className="mt-4">
                <Button variant="primary" icon="play" loading={busy === `launch:${featured.id}:false`} onClick={() => onPlay(featured)}>
                  {t("play")}
                </Button>
              </div>
            )}
          </div>
        </div>

        {/* Game mode plate. */}
        <div className="glass w-full shrink-0 rounded-2xl p-4 md:w-[22rem]">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <p className="flex items-center gap-2 font-display text-md font-semibold text-fg">
                <Icon name="gamepad" size={16} className="text-accent-text" />
                {t("gameMode")}
              </p>
              <p id={hintId} className="mt-0.5 text-xs text-fg-muted">
                {t("gameModeHint")}
              </p>
            </div>
            <Switch label={t("gameModeSwitch")} checked={state.gameMode.enabled} onChange={onToggleMode} disabled={busy === "mode"} describedBy={hintId} />
          </div>

          <AnimatePresence initial={false}>
            {session && (
              <m.div
                key={session.gameId}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0, transition: { duration: duration.component, ease: ease.out } }}
                exit={{ opacity: 0, transition: { duration: duration.exit } }}
              >
                <div className="mt-4 border-t border-line pt-3">
                  {session.domains.length === 0 ? (
                    <p className="text-xs text-fg-muted">{t("noProfileApplied")}</p>
                  ) : (
                    <>
                      <p className="mb-2 text-2xs text-fg-subtle">{t("appliedCount", { applied: summary.applied, total: summary.total })}</p>
                      <ul className="space-y-1.5">
                        {session.domains.map((d, i) => (
                          <m.li
                            key={d.domain}
                            initial={{ opacity: 0, x: -6 }}
                            animate={{ opacity: 1, x: 0, transition: { delay: 0.06 * i, duration: duration.component, ease: ease.out } }}
                            className="flex items-center gap-2.5 text-sm"
                          >
                            <span className="satin grid size-7 shrink-0 place-items-center rounded-full text-fg-muted" aria-hidden="true">
                              <Icon name={DOMAIN_ICON[d.domain]} size={14} />
                            </span>
                            <span className="min-w-0 flex-1">
                              <span className="block text-2xs text-fg-subtle">{t(`domain_${d.domain}`)}</span>
                              <span className="block truncate text-fg">{d.profileName ?? d.profileId}</span>
                            </span>
                            {d.applied ? <StatusBadge size="sm" glyph tone="ok" label={t("applied")} /> : <SupportBadge support={d.support} />}
                          </m.li>
                        ))}
                      </ul>
                    </>
                  )}
                  <div className="mt-3 flex items-center justify-between gap-2">
                    <p className="text-2xs text-fg-subtle">{t("restoreHint")}</p>
                    <Button size="sm" icon="undo" loading={busy === "stop"} onClick={onStop}>
                      {t("restore")}
                    </Button>
                  </div>
                </div>
              </m.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </section>
  );
}

/** The clock, re-read every `everyMs` (null: read once). */
function useNow(everyMs: number | null): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    setNow(Date.now());
    if (everyMs === null) return;
    const id = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(id);
  }, [everyMs]);
  return now;
}

/** "da 12 min", "da 1 h 05 min", "da meno di un minuto". */
function elapsedText(t: Translator<"gaming">, ms: number): string {
  const min = Math.floor(Math.max(ms, 0) / 60_000);
  if (min < 1) return t("elapsedNow");
  if (min < 60) return t("elapsedMin", { min });
  return t("elapsedHours", { h: Math.floor(min / 60), min: String(min % 60).padStart(2, "0") });
}

/**
 * Backdrop for a game without wide art: its cover, else its app icon, blown up and blurred
 * into a wash of its own colours; the title's gradient when it has neither.
 */
function Wash({ game }: { game: GameEntry }) {
  const cover = useGameArt(game.id, "cover", game.hasCover);
  const icon = useGameArt(game.id, "icon", !game.hasCover && game.hasIcon);
  const art = cover ?? icon;
  if (!art) return <div className="size-full opacity-70" style={{ backgroundImage: coverBackground(game.title) }} />;
  return <img src={art} alt="" draggable={false} className="size-full scale-125 object-cover opacity-80 blur-3xl saturate-150" />;
}
