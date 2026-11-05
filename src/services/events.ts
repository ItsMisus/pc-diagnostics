import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { BenchmarkProgress } from "@/types/generated/BenchmarkProgress";
import type { BenchmarkResult } from "@/types/generated/BenchmarkResult";
import type { LiveSample } from "@/types/generated/LiveSample";
import type { ProcessSnapshot } from "@/types/generated/ProcessSnapshot";
import { isTauriRuntime } from "./ipc";

/** Must match `models::monitor::LIVE_SAMPLE_EVENT`. */
export const LIVE_SAMPLE_EVENT = "monitor:sample";

/** Must match `models::benchmark_run::BENCHMARK_PROGRESS_EVENT` / `BENCHMARK_FINISHED_EVENT`. */
export const BENCHMARK_PROGRESS_EVENT = "benchmark:progress";
export const BENCHMARK_FINISHED_EVENT = "benchmark:finished";

export async function onBenchmarkProgress(handler: (p: BenchmarkProgress) => void): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => {};
  return listen<BenchmarkProgress>(BENCHMARK_PROGRESS_EVENT, (event) => handler(event.payload));
}

export async function onBenchmarkFinished(handler: (r: BenchmarkResult) => void): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => {};
  return listen<BenchmarkResult>(BENCHMARK_FINISHED_EVENT, (event) => handler(event.payload));
}

/** Must match `models::process::PROCESS_SNAPSHOT_EVENT`. */
export const PROCESS_SNAPSHOT_EVENT = "processes:snapshot";

/** Subscribes to process rankings (backend cadence ~2 s). Outside Tauri: no-op. */
export async function onProcessSnapshot(handler: (snapshot: ProcessSnapshot) => void): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => {};
  return listen<ProcessSnapshot>(PROCESS_SNAPSHOT_EVENT, (event) => handler(event.payload));
}

/** Subscribes to live samples. Outside Tauri resolves to a no-op unsubscribe. */
export async function onLiveSample(handler: (sample: LiveSample) => void): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => {};
  return listen<LiveSample>(LIVE_SAMPLE_EVENT, (event) => handler(event.payload));
}
