import { create } from "zustand";
import { commands } from "@/services/commands";
import { toAppError } from "@/services/ipc";
import { setLang } from "@/i18n";
import { setByteUnits } from "@/lib/format";
import type { AppError } from "@/types/generated/AppError";
import type { AppSettings } from "@/types/generated/AppSettings";
import type { ViewMode } from "@/types/generated/ViewMode";
import type { SectionId } from "./sections";

export type SettingsState =
  | { status: "loading" }
  | { status: "ready"; value: AppSettings; saving: boolean; saveError: AppError | null }
  | { status: "error"; error: AppError };

interface AppStore {
  section: SectionId;
  /** Local mirror so the sidebar works even when settings could not be loaded. */
  sidebarCollapsed: boolean;
  /**
   * Local mirror of `settings.viewMode` ("Semplice" by default) so the toggle works even
   * when settings could not be loaded. Read it through `useViewMode()`.
   */
  viewMode: ViewMode;
  settings: SettingsState;
  setSection: (id: SectionId) => void;
  toggleSidebar: () => void;
  setViewMode: (mode: ViewMode) => void;
  loadSettings: () => Promise<void>;
  /** Asks the backend for the effective language ("system" resolved) and applies it. */
  syncLanguage: () => Promise<void>;
  updateSettings: (patch: Partial<AppSettings>) => Promise<void>;
}

export const useAppStore = create<AppStore>((set, get) => ({
  section: "overview",
  sidebarCollapsed: false,
  viewMode: "simple",
  settings: { status: "loading" },

  setSection: (section) => set({ section }),

  toggleSidebar: () => {
    const next = !get().sidebarCollapsed;
    set({ sidebarCollapsed: next });
    if (get().settings.status === "ready") void get().updateSettings({ sidebarCollapsed: next });
  },

  setViewMode: (mode) => {
    if (get().viewMode === mode) return;
    set({ viewMode: mode });
    if (get().settings.status === "ready") void get().updateSettings({ viewMode: mode });
  },

  syncLanguage: async () => {
    try {
      const lang = await commands.getLanguage();
      setLang(lang === "it" ? "it" : "en");
    } catch {
      // Keep the language seeded from the WebView (it follows Windows).
    }
  },

  loadSettings: async () => {
    set({ settings: { status: "loading" } });
    void get().syncLanguage();
    try {
      const value = await commands.getSettings();
      set({
        settings: { status: "ready", value, saving: false, saveError: null },
        sidebarCollapsed: value.sidebarCollapsed,
        viewMode: normalizeViewMode(value.viewMode),
      });
    } catch (e) {
      set({ settings: { status: "error", error: toAppError(e) } });
    }
  },

  /** Optimistic update; the backend validates and returns the stored value, or we roll back. */
  updateSettings: async (patch) => {
    const current = get().settings;
    if (current.status !== "ready") return;
    const previous = current.value;
    const optimistic = { ...previous, ...patch };
    set({ settings: { status: "ready", value: optimistic, saving: true, saveError: null } });
    try {
      const stored = await commands.updateSettings(optimistic);
      set({
        settings: { status: "ready", value: stored, saving: false, saveError: null },
        viewMode: normalizeViewMode(stored.viewMode),
      });
      if (stored.language !== previous.language) void get().syncLanguage();
    } catch (e) {
      set({
        settings: { status: "ready", value: previous, saving: false, saveError: toAppError(e) },
        sidebarCollapsed: previous.sidebarCollapsed,
        viewMode: normalizeViewMode(previous.viewMode),
      });
    }
  },
}));

/** Anything but an explicit "advanced" (e.g. a backend that predates the field) is simple. */
export function normalizeViewMode(value: unknown): ViewMode {
  return value === "advanced" ? "advanced" : "simple";
}

/**
 * Byte labels follow the view mode (KB/MB/GB in "Semplice", KiB/MiB/GiB in "Avanzata").
 * A store listener runs synchronously inside `set`, before React re-renders, so every
 * component that renders after a mode change already formats with the new labels.
 */
useAppStore.subscribe((state, previous) => {
  if (state.viewMode !== previous.viewMode) setByteUnits(state.viewMode === "advanced" ? "iec" : "windows");
});
