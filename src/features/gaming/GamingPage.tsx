import { AnimatePresence, m } from "motion/react";
import { useEffect, useMemo, useState } from "react";
import { sectionLabel } from "@/app/sections";
import { Button } from "@/components/common/Button";
import { Panel } from "@/components/common/Panel";
import { SearchField } from "@/components/common/SearchField";
import { Segmented } from "@/components/common/Segmented";
import { Skeleton } from "@/components/common/Skeleton";
import { StateView } from "@/components/common/StateView";
import { Switch } from "@/components/common/Switch";
import { toast } from "@/components/common/Toast";
import { Tooltip } from "@/components/common/Tooltip";
import { PageHeader } from "@/components/layout/PageHeader";
import { SectionHeading } from "@/components/layout/SectionHeading";
import { localeTag, useLang, useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { duration, ease } from "@/lib/motion";
import { useIsMac } from "@/lib/platform";
import type { AppError } from "@/types/generated/AppError";
import type { GameEntry } from "@/types/generated/GameEntry";
import type { GameProfile } from "@/types/generated/GameProfile";
import type { GamingState } from "@/types/generated/GamingState";
import { gamingApi, onGamingSession, pickGameFile } from "./api";
import { GameCard } from "./GameCard";
import { GameDetail } from "./GameDetail";
import { GameModeHero } from "./GameModeHero";
import { useGaming } from "./gamingStore";
import { DEFAULT_QUERY, filterLibrary, hiddenCount, sourceOptions, type LibraryQuery, type SortKey, type SourceFilter } from "./library";
import { emptyReason, profileFor, profileIndicator } from "./model";

/**
 * Gaming center: game mode (focal hero), the library as a cover grid, a detail dialog per game.
 * Nothing is applied on entering the page: profiles apply only on "Gioca con profilo" or while
 * game mode (a user choice) sees a game running.
 */
export function GamingPage() {
  const t = useT("gaming");
  const isMac = useIsMac();
  const load = useGaming((s) => s.load);
  const fetchState = useGaming((s) => s.fetch);
  const refreshing = useGaming((s) => s.refreshing);
  const busy = useGaming((s) => s.busy);

  useEffect(() => {
    void fetchState(false);
    let unlisten: (() => void) | null = null;
    let alive = true;
    void onGamingSession(() => void useGaming.getState().fetch(false)).then((u) => {
      if (alive) unlisten = u;
      else u();
    });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [fetchState]);

  const addGame = async () => {
    try {
      const path = await pickGameFile({ title: t("addGamePicker"), filterName: t("addGameFilter"), mac: isMac });
      if (!path) return;
      const before = new Set(load.status === "ready" ? load.data.library.map((e) => e.id) : []);
      const next = await useGaming.getState().run("add", () => gamingApi.addManual(path));
      const added = next.library.find((e) => !before.has(e.id));
      toast.success(t("addGameDone", { title: added?.title ?? path }));
    } catch (e) {
      toast.error(t("actionFailed"), (e as AppError).message);
    }
  };

  return (
    <div className="pb-12">
      <PageHeader
        title={sectionLabel("gaming")}
        actions={
          <>
            <Tooltip content={t("refresh")} side="bottom">
              <Button variant="ghost" iconOnly icon="refresh" aria-label={t("refresh")} loading={refreshing} onClick={() => void fetchState(true)} />
            </Tooltip>
            <Button icon="plus" loading={busy === "add"} onClick={() => void addGame()} disabled={load.status !== "ready"}>
              {t("addGame")}
            </Button>
          </>
        }
      />
      {load.status === "ready" ? (
        <GamingContent state={load.data} busy={busy} />
      ) : load.status === "error" ? (
        <Panel variant="primary">
          <StateView layout="hero" kind={stateKindForError(load.error)} title={t("loadError")} error={load.error} onRetry={() => void fetchState(false)} />
        </Panel>
      ) : (
        <LoadingSkeleton />
      )}
    </div>
  );
}

function LoadingSkeleton() {
  return (
    <div aria-busy="true">
      <Skeleton className="mb-8 h-[320px] w-full rounded-2xl" />
      <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-5">
        {Array.from({ length: 6 }, (_, i) => (
          <Skeleton key={i} className="aspect-[2/3] w-full rounded-xl" />
        ))}
      </div>
    </div>
  );
}

function GamingContent({ state, busy }: { state: GamingState; busy: string | null }) {
  const t = useT("gaming");
  const lang = useLang();
  const run = useGaming((s) => s.run);
  const [query, setQuery] = useState<LibraryQuery>(DEFAULT_QUERY);
  const [openId, setOpenId] = useState<string | null>(null);
  const [detailError, setDetailError] = useState<AppError | null>(null);

  const list = useMemo(() => filterLibrary(state.library, query, localeTag(lang)), [state.library, query, lang]);
  const sources = sourceOptions(state.sources, state.library);
  const hidden = hiddenCount(state.library);
  const searching = query.search.trim() !== "";
  const empty = emptyReason(state, query.source, list.length, searching);
  const openGame = openId ? (state.library.find((e) => e.id === openId) ?? null) : null;
  const playingId = state.gameMode.session?.gameId ?? null;

  // A filter for a source that disappeared falls back to all.
  const source: SourceFilter = sources.includes(query.source) ? query.source : "all";
  useEffect(() => {
    if (source !== query.source) setQuery((q) => ({ ...q, source }));
  }, [source, query.source]);

  const act = async (key: string, action: () => Promise<GamingState>, done?: (s: GamingState) => void) => {
    setDetailError(null);
    try {
      const next = await run(key, action);
      done?.(next);
    } catch (e) {
      const error = e as AppError;
      if (openId) setDetailError(error);
      else toast.error(t("actionFailed"), error.message);
    }
  };

  const play = (game: GameEntry, withProfile: boolean) =>
    void act(`launch:${game.id}:${withProfile}`, () => gamingApi.launch(game.id, withProfile), () =>
      toast.success(t(withProfile ? "launchedWithProfile" : "launched", { title: game.title })),
    );
  const saveProfile = (profile: GameProfile) => void act("profile", () => gamingApi.setProfile(profile));
  const hide = (game: GameEntry, hiddenNow: boolean) => void act(`hide:${game.id}`, () => gamingApi.setHidden(game.id, hiddenNow));
  const remove = (game: GameEntry) =>
    void act(`remove:${game.id}`, () => gamingApi.removeManual(game.id), () => {
      setOpenId(null);
      toast.success(t("removed", { title: game.title }));
    });

  return (
    <>
      <GameModeHero
        state={state}
        busy={busy}
        onToggleMode={(enabled) => void act("mode", () => gamingApi.setGameMode(enabled))}
        onStop={() => void act("stop", () => gamingApi.stopSession(), () => toast.success(t("restoreDone")))}
        onOpen={(g) => setOpenId(g.id)}
        onPlay={(g) => play(g, false)}
      />

      <section aria-labelledby="gaming-library">
        <SectionHeading id="gaming-library" count={list.length} className="mb-4">
          {t("library")}
        </SectionHeading>
        <div className="mb-5 flex flex-wrap items-center gap-3">
          <SearchField
            label={t("search")}
            placeholder={t("searchPlaceholder")}
            value={query.search}
            onChange={(search) => setQuery((q) => ({ ...q, search }))}
            className="w-full sm:w-56"
          />
          {sources.length > 2 && (
            <Segmented
              size="sm"
              label={t("sourceFilter")}
              value={source}
              onChange={(s) => setQuery((q) => ({ ...q, source: s }))}
              options={sources.map((s) => ({ value: s, label: t(`source_${s}`) }))}
            />
          )}
          <Segmented<SortKey>
            size="sm"
            label={t("sort")}
            value={query.sort}
            onChange={(sort) => setQuery((q) => ({ ...q, sort }))}
            options={[
              { value: "name", label: t("sortName") },
              { value: "lastPlayed", label: t("sortRecent") },
            ]}
          />
          {hidden > 0 && (
            <label className="ms-auto flex items-center gap-2 text-sm text-fg-muted">
              {t("showHidden")} <span className="tabular text-fg-subtle">{hidden}</span>
              <Switch label={t("showHidden")} checked={query.showHidden} onChange={(showHidden) => setQuery((q) => ({ ...q, showHidden }))} />
            </label>
          )}
        </div>

        {empty ? (
          <Panel>
            <StateView
              layout="hero"
              kind="empty"
              title={
                empty.kind === "sourceMissing"
                  ? t("sourceMissing", { source: t(`sourceName_${empty.source}`) })
                  : t(empty.kind === "noGames" ? "emptyNoGames" : "emptyNoMatches")
              }
              description={
                empty.kind === "sourceMissing"
                  ? t("sourceMissingDesc", { source: t(`sourceName_${empty.source}`) })
                  : t(empty.kind === "noGames" ? "emptyNoGamesDesc" : "emptyNoMatchesDesc")
              }
            />
          </Panel>
        ) : (
          <m.ul layout={false} className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-x-5 gap-y-6">
            <AnimatePresence initial={false}>
              {list.map((g) => (
                <m.li
                  key={g.id}
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0, transition: { duration: duration.component, ease: ease.out } }}
                  exit={{ opacity: 0, transition: { duration: duration.exit } }}
                  className="min-w-0"
                >
                  <GameCard game={g} indicator={profileIndicator(profileFor(state, g.id))} playing={g.id === playingId} onOpen={() => setOpenId(g.id)} />
                </m.li>
              ))}
            </AnimatePresence>
          </m.ul>
        )}
      </section>

      <GameDetail
        game={openGame}
        state={state}
        busy={busy}
        error={detailError}
        onClose={() => {
          setOpenId(null);
          setDetailError(null);
        }}
        onPlay={play}
        onSaveProfile={saveProfile}
        onHide={hide}
        onRemove={remove}
      />
    </>
  );
}
