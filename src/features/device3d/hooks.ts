import { useEffect, useState, type RefObject } from "react";
import type { StageTheme } from "./types";

function media(query: string): MediaQueryList | null {
  return typeof window !== "undefined" && typeof window.matchMedia === "function" ? window.matchMedia(query) : null;
}

function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() => media(query)?.matches ?? false);
  useEffect(() => {
    const mq = media(query);
    if (!mq) return;
    const update = () => setMatches(mq.matches);
    update();
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  }, [query]);
  return matches;
}

/** The OS "reduce motion" setting, live. */
export function usePrefersReducedMotion(): boolean {
  return useMediaQuery("(prefers-reduced-motion: reduce)");
}

/** True while the window is minimised or hidden (visibilitychange). */
export function useDocumentHidden(): boolean {
  const [hidden, setHidden] = useState(() => typeof document !== "undefined" && document.hidden);
  useEffect(() => {
    const update = () => setHidden(document.hidden);
    document.addEventListener("visibilitychange", update);
    return () => document.removeEventListener("visibilitychange", update);
  }, []);
  return hidden;
}

/** Whether the element intersects the viewport (true when IntersectionObserver is missing). */
export function useInView(ref: RefObject<Element | null>): boolean {
  const [inView, setInView] = useState(true);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver((entries) => {
      const last = entries[entries.length - 1];
      if (last) setInView(last.isIntersecting);
    });
    io.observe(el);
    return () => io.disconnect();
  }, [ref]);
  return inView;
}

/** `<html data-theme>` (explicit choice) or, without it, the OS colour scheme. */
export function resolveTheme(dataTheme: string | undefined, prefersDark: boolean): StageTheme {
  if (dataTheme === "light" || dataTheme === "dark") return dataTheme;
  return prefersDark ? "dark" : "light";
}

/** The app's effective theme, following both the setting and the OS. */
export function useDocumentTheme(): StageTheme {
  const prefersDark = useMediaQuery("(prefers-color-scheme: dark)");
  const [dataTheme, setDataTheme] = useState(() => (typeof document !== "undefined" ? document.documentElement.dataset.theme : undefined));
  useEffect(() => {
    const root = document.documentElement;
    const update = () => setDataTheme(root.dataset.theme);
    update();
    const mo = new MutationObserver(update);
    mo.observe(root, { attributes: true, attributeFilter: ["data-theme"] });
    return () => mo.disconnect();
  }, []);
  return resolveTheme(dataTheme, prefersDark);
}
