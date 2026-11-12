import { lazy, Suspense, useEffect, type ReactNode } from "react";
import { Toaster } from "@/components/common/Toast";
import { AppShell } from "@/components/layout/AppShell";
import { MotionProvider } from "@/components/motion/MotionProvider";
import { benchStore } from "@/features/benchmarks/benchStore";
import { DiagnosticsPage } from "@/features/diagnostics/DiagnosticsPage";
import { FixDialog } from "@/features/assistant/FixDialog";
import { liveStore } from "@/features/monitoring/liveStore";
import { OverviewPage } from "@/features/overview/OverviewPage";
import { useLangSync, usePlatformSync, useThemeSync } from "@/hooks/useThemeSync";
import { getPlatform, usePlatform } from "@/lib/platform";
import { isSectionAvailable, type SectionId } from "./sections";
import { useAppStore } from "./store";

// Overview (the start page) and Diagnostics (Overview embeds its insights card) stay in the
// main bundle. The other pages are split out to keep startup parsing small, and preloaded
// once the app is idle so that switching sections never waits on a chunk.
const pageLoaders = {
  hardware: () => import("@/features/hardware/HardwarePage").then((m) => ({ default: m.HardwarePage })),
  benchmarks: () => import("@/features/benchmarks/BenchmarksPage").then((m) => ({ default: m.BenchmarksPage })),
  processes: () => import("@/features/processes/ProcessesPage").then((m) => ({ default: m.ProcessesPage })),
  drivers: () => import("@/features/drivers/DriversPage").then((m) => ({ default: m.DriversPage })),
  history: () => import("@/features/history/HistoryPage").then((m) => ({ default: m.HistoryPage })),
  settings: () => import("@/features/settings/SettingsPage").then((m) => ({ default: m.SettingsPage })),
};
const HardwarePage = lazy(pageLoaders.hardware);
const BenchmarksPage = lazy(pageLoaders.benchmarks);
const ProcessesPage = lazy(pageLoaders.processes);
const DriversPage = lazy(pageLoaders.drivers);
const HistoryPage = lazy(pageLoaders.history);
const SettingsPage = lazy(pageLoaders.settings);

function preloadPages(): () => void {
  const run = () => {
    // Read when the preload runs (by then the backend has normally confirmed the platform):
    // a page the platform does not have (Drivers on macOS) is not preloaded.
    const platform = getPlatform();
    for (const [id, load] of Object.entries(pageLoaders)) {
      if (isSectionAvailable(id as SectionId, platform)) void load();
    }
  };
  if (typeof requestIdleCallback === "function") {
    const id = requestIdleCallback(run, { timeout: 2000 });
    return () => cancelIdleCallback(id);
  }
  const id = setTimeout(run, 500);
  return () => clearTimeout(id);
}

function Page({ id }: { id: SectionId }): ReactNode {
  const platform = usePlatform();
  // The store never selects a section the platform lacks; this guards the render as well.
  if (!isSectionAvailable(id, platform)) return <OverviewPage />;
  switch (id) {
    case "overview":
      return <OverviewPage />;
    case "hardware":
      return <HardwarePage />;
    case "benchmarks":
      return <BenchmarksPage />;
    case "processes":
      return <ProcessesPage />;
    case "drivers":
      return <DriversPage />;
    case "diagnostics":
      return <DiagnosticsPage />;
    case "history":
      return <HistoryPage />;
    case "settings":
      return <SettingsPage />;
  }
}

export function App() {
  const sectionId = useAppStore((s) => s.section);
  // Re-renders the page tree when the view mode changes, so every size picks up the new
  // byte labels (KB/MB/GB or KiB/MiB/GiB, lib/format.ts). Memoised components that format
  // bytes subscribe on their own.
  useAppStore((s) => s.viewMode);
  const loadSettings = useAppStore((s) => s.loadSettings);
  const loadPlatform = useAppStore((s) => s.loadPlatform);
  useThemeSync();
  useLangSync();
  usePlatformSync();

  useEffect(() => {
    void loadPlatform();
  }, [loadPlatform]);
  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  // The single app-level live subscription. The cleanup detaches the event listener.
  useEffect(() => liveStore.getState().start(), []);
  // The single app-level benchmark subscription (progress + finished), for the running bar.
  useEffect(() => benchStore.getState().start(), []);
  useEffect(preloadPages, []);

  return (
    <MotionProvider>
      <AppShell sectionKey={sectionId}>
        <Suspense fallback={null}>
          <Page id={sectionId} />
        </Suspense>
      </AppShell>
      <FixDialog />
      <Toaster />
    </MotionProvider>
  );
}
