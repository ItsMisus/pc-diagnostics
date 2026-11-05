import { useEffect } from "react";
import { useAppStore } from "@/app/store";
import { useLang } from "@/i18n";

/**
 * Mirrors the theme preference onto `<html data-theme>`. With "system" (or before
 * settings load) the attribute is removed and CSS follows `prefers-color-scheme`.
 */
export function useThemeSync() {
  const theme = useAppStore((s) => (s.settings.status === "ready" ? s.settings.value.theme : "system"));

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") delete root.dataset.theme;
    else root.dataset.theme = theme;
  }, [theme]);
}

/** Mirrors the effective language onto `<html lang>` (screen readers, hyphenation, spelling). */
export function useLangSync() {
  const lang = useLang();
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
}
