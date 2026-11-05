import { lazy, Suspense, useEffect, type ReactNode } from "react";
import { Toaster } from "@/components/common/Toast";
import { AppShell } from "@/components/layout/AppShell";
import { MotionProvider } from "@/components/motion/MotionProvider";
import { benchStore } from "@/features/benchmarks/benchStore";
import { DiagnosticsPage } from "@/features/diagnostics/DiagnosticsPage";
import { FixDialog } from "@/features/assistant/FixDialog";
import { liveStore } from "@/features/monitoring/liveStore";
import { OverviewPage } from "@/features/overview/OverviewPage";
import { useLangSync, useThemeSync } from "@/hooks/useThemeSync";
import type { SectionId } from "./sections";
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
    for (const load of Object.values(pageLoaders)) void load();
  };
  if (typeof requestIdleCallback === "function") {
    const id = requestIdleCallback(run, { timeout: 2000 });
    return () => cancelIdleCallback(id);
  }
  const id = setTimeout(run, 500);
  return () => clearTimeout(id);
}

function Page({ id }: { id: SectionId }): ReactNode {
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
  const loadSettings = useAppStore((s) => s.loadSettings);
  useThemeSync();
  useLangSync();

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
