import { useCallback, useEffect, useRef, type ReactNode, type RefObject } from "react";
import { resolveShortcut } from "@/app/shortcuts";
import { useAppStore } from "@/app/store";
import { getPlatform } from "@/lib/platform";
import { PageTransition } from "@/components/motion/PageTransition";
import { RunningBar } from "@/features/benchmarks/RunningBar";
import { Sidebar } from "./Sidebar";

/**
 * Window frame: the ambient canvas, the floating glass sidebar and the scrolling main area.
 *
 * - Pages cross-fade on section change (`PageTransition`); the scroll position resets once
 *   the old page has left, so nothing jumps mid-fade.
 * - The main area reserves its scrollbar gutter, so content never shifts when a page starts
 *   scrolling.
 * - `data-scrolled` on <main> is true while content sits under the sticky page toolbar
 *   (an IntersectionObserver on a 1 px sentinel: no scroll listener, no re-render).
 * - The running-benchmark strip floats at the bottom of the main area, over the content.
 */
export function AppShell({ sectionKey, children }: { sectionKey: string; children: ReactNode }) {
  const mainRef = useRef<HTMLElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const resetScroll = useCallback(() => mainRef.current?.scrollTo({ top: 0 }), []);
  useShortcuts();
  useScrolledFlag(mainRef, sentinelRef);

  return (
    <div className="ambient-canvas flex h-full">
      <Sidebar />
      <div className="relative min-w-0 flex-1">
        <main ref={mainRef} data-scrolled="false" className="h-full overflow-y-auto [scrollbar-gutter:stable]">
          <div ref={sentinelRef} aria-hidden="true" className="h-px" />
          <div className="page-gutter -mt-px pb-24">
            <PageTransition pageKey={sectionKey} onExitComplete={resetScroll}>
              {children}
            </PageTransition>
          </div>
        </main>
        <RunningBar />
      </div>
    </div>
  );
}

function useScrolledFlag(main: RefObject<HTMLElement | null>, sentinel: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const root = main.current;
    const target = sentinel.current;
    if (!root || !target || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver(
      ([entry]) => {
        if (entry) root.dataset.scrolled = String(!entry.isIntersecting);
      },
      { root, threshold: 0 },
    );
    io.observe(target);
    return () => io.disconnect();
  }, [main, sentinel]);
}

/**
 * Keyboard shortcuts (`app/shortcuts.ts`): Ctrl+B / ⌘B toggles the sidebar, Ctrl+1…8 on
 * Windows and ⌘1…7 on macOS (no Drivers there) open the sections in sidebar order. Ignored
 * while a modal dialog is open or while typing in a field.
 */
function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const action = resolveShortcut(e, getPlatform());
      if (!action) return;
      if (document.querySelector("dialog[open]")) return;
      const t = e.target as HTMLElement | null;
      if (t && (t.isContentEditable || t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
      e.preventDefault();
      const { toggleSidebar, setSection } = useAppStore.getState();
      if (action.kind === "toggleSidebar") toggleSidebar();
      else setSection(action.id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
