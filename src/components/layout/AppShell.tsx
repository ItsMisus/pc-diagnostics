import { useCallback, useEffect, useRef, type ReactNode } from "react";
import { SECTIONS } from "@/app/sections";
import { useAppStore } from "@/app/store";
import { PageTransition } from "@/components/motion/PageTransition";
import { RunningBar } from "@/features/benchmarks/RunningBar";
import { Sidebar } from "./Sidebar";

/**
 * Window frame: sidebar + scrolling main area. The page content cross-fades on section
 * change and the scroll position resets once the old page has left (no jump mid-fade).
 */
export function AppShell({ sectionKey, children }: { sectionKey: string; children: ReactNode }) {
  const mainRef = useRef<HTMLElement>(null);
  const resetScroll = useCallback(() => mainRef.current?.scrollTo({ top: 0 }), []);
  useShortcuts();

  return (
    <div className="flex h-full bg-canvas">
      <Sidebar />
      <main ref={mainRef} className="relative min-w-0 flex-1 overflow-y-auto [scrollbar-gutter:stable]">
        <RunningBar />
        <div className="mx-auto max-w-6xl px-6 pt-7 pb-12 lg:px-10">
          <PageTransition pageKey={sectionKey} onExitComplete={resetScroll}>
            {children}
          </PageTransition>
        </div>
      </main>
    </div>
  );
}

/**
 * Ctrl+B toggles the sidebar; Ctrl+1…8 jumps to a section. Ignored while a modal dialog is
 * open or while typing in a field.
 */
function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;
      if (document.querySelector("dialog[open]")) return;
      const t = e.target as HTMLElement | null;
      if (t && (t.isContentEditable || t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
      const { toggleSidebar, setSection } = useAppStore.getState();
      if (e.key.toLowerCase() === "b") {
        e.preventDefault();
        toggleSidebar();
        return;
      }
      const n = Number(e.key);
      const target = Number.isInteger(n) && n >= 1 ? SECTIONS[n - 1] : undefined;
      if (target) {
        e.preventDefault();
        setSection(target.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
