import { useEffect, useSyncExternalStore } from "react";
import { useAppStore } from "@/app/store";
import { useLang } from "@/i18n";
import { usePlatform } from "@/lib/platform";
import { resolveTheme, type ResolvedTheme } from "@/lib/theme";
import type { ThemePreference } from "@/types/generated/ThemePreference";

const useThemePreference = (): ThemePreference => useAppStore((s) => (s.settings.status === "ready" ? s.settings.value.theme : "system"));

/**
 * Mirrors the theme preference onto `<html data-theme>`. With "system" (or before
 * settings load) the attribute is removed and CSS follows `prefers-color-scheme`.
 */
export function useThemeSync() {
  const theme = useThemePreference();

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") delete root.dataset.theme;
    else root.dataset.theme = theme;
  }, [theme]);
}

const DARK_QUERY = "(prefers-color-scheme: dark)";

function subscribeSystemDark(onChange: () => void): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return () => {};
  const mq = window.matchMedia(DARK_QUERY);
  mq.addEventListener("change", onChange);
  return () => mq.removeEventListener("change", onChange);
}

const systemDark = (): boolean => typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia(DARK_QUERY).matches;

/**
 * The theme in effect ("light" | "dark"), following the setting and, for "system", the OS
 * live. For code that needs it as a value (the 3D scene's lights and materials); CSS uses the
 * tokens instead.
 */
export function useResolvedTheme(): ResolvedTheme {
  const preference = useThemePreference();
  const dark = useSyncExternalStore(subscribeSystemDark, systemDark, () => false);
  return resolveTheme(preference, dark);
}

/** Mirrors the effective language onto `<html lang>` (screen readers, hyphenation, spelling). */
export function useLangSync() {
  const lang = useLang();
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
}

/**
 * Mirrors the platform onto `<html data-platform>`, for the few platform-specific styles
 * (the macOS overlay title bar: `--titlebar-h`).
 */
export function usePlatformSync() {
  const platform = usePlatform();
  useEffect(() => {
    document.documentElement.dataset.platform = platform;
  }, [platform]);
}
