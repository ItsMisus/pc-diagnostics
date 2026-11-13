import { useEffect, useState } from "react";
import { create } from "zustand";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { GameArtKind } from "@/types/generated/GameArtKind";
import type { GamingState } from "@/types/generated/GamingState";
import { gamingApi } from "./api";

export type GamingLoad =
  | { status: "idle" | "loading" }
  | { status: "ready"; data: GamingState }
  | { status: "error"; error: AppError };

interface GamingStore {
  load: GamingLoad;
  /** Key of the action in flight (e.g. `launch:steam:1`), for button spinners. */
  busy: string | null;
  refreshing: boolean;
  fetch: (refresh?: boolean) => Promise<void>;
  /** Runs an action that returns the new state; rejects with the `AppError` (state kept). */
  run: (key: string, action: () => Promise<GamingState>) => Promise<GamingState>;
}

export const useGaming = create<GamingStore>((set, get) => ({
  load: { status: "idle" },
  busy: null,
  refreshing: false,
  fetch: async (refresh = false) => {
    const first = get().load.status !== "ready";
    set(first ? { load: { status: "loading" } } : { refreshing: refresh });
    try {
      const data = await gamingApi.getState(refresh);
      if (refresh) clearArtCache();
      set({ load: { status: "ready", data }, refreshing: false });
    } catch (raw) {
      const error = toAppError(raw);
      if (first) set({ load: { status: "error", error } });
      set({ refreshing: false });
    }
  },
  run: async (key, action) => {
    set({ busy: key });
    try {
      const data = await action();
      set({ load: { status: "ready", data } });
      return data;
    } catch (raw) {
      throw toAppError(raw);
    } finally {
      set({ busy: null });
    }
  },
}));

/** Art per `id:kind`, fetched once per session (the backend caches too). */
const artCache = new Map<string, Promise<string | null>>();

export function clearArtCache(): void {
  artCache.clear();
}

export function loadArt(id: string, kind: GameArtKind): Promise<string | null> {
  const key = `${id}:${kind}`;
  let p = artCache.get(key);
  if (!p) {
    p = gamingApi.getArt(id, kind).catch(() => null);
    artCache.set(key, p);
  }
  return p;
}

/** The art data URL, or null while loading / when there is none. `enabled` false skips the call. */
export function useGameArt(id: string | null, kind: GameArtKind, enabled: boolean): string | null {
  const [url, setUrl] = useState<{ key: string; url: string | null } | null>(null);
  const key = id ? `${id}:${kind}` : "";
  useEffect(() => {
    if (!id || !enabled) return;
    let alive = true;
    void loadArt(id, kind).then((u) => {
      if (alive) setUrl({ key: `${id}:${kind}`, url: u });
    });
    return () => {
      alive = false;
    };
  }, [id, kind, enabled]);
  return enabled && url?.key === key ? url.url : null;
}
