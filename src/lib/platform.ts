import { create } from "zustand";
import type { Platform } from "@/types/generated/Platform";

/**
 * The operating system the app runs on, available app-wide (React and plain modules).
 *
 * - Seeded synchronously from the WebView's user agent, so the first render already hides
 *   features of other platforms: WebView2 on Windows reports "Windows NT", WKWebView on macOS
 *   "Macintosh; … Mac OS X". Nothing Windows-only flashes on a Mac before the backend answers.
 * - Then confirmed by the backend (`get_app_info().platform`, the OS the binary was built
 *   for): the app store calls `setPlatform` once at startup (`loadPlatform`).
 *
 * The seed is only a UI hint for what to show; it is never reported as data. An unknown user
 * agent yields "other", which hides Windows-only features until the backend has answered.
 *
 * Kept free of other app imports: `@/i18n` reads it to pick platform-specific texts.
 */

/** Best guess from a user-agent string. Exported for tests. */
export function platformFromUserAgent(userAgent: string): Platform {
  if (/Windows/.test(userAgent)) return "windows";
  if (/Macintosh|Mac OS X/.test(userAgent)) return "macos";
  if (/Linux|X11/.test(userAgent)) return "linux";
  return "other";
}

function initialPlatform(): Platform {
  return typeof navigator !== "undefined" ? platformFromUserAgent(navigator.userAgent) : "other";
}

interface PlatformStore {
  platform: Platform;
  /** True once the backend has answered; until then `platform` is the user-agent guess. */
  confirmed: boolean;
  setPlatform: (platform: Platform, confirmed?: boolean) => void;
}

export const usePlatformStore = create<PlatformStore>((set) => ({
  platform: initialPlatform(),
  confirmed: false,
  setPlatform: (platform, confirmed = true) => set({ platform, confirmed }),
}));

export const getPlatform = (): Platform => usePlatformStore.getState().platform;
export const setPlatform = (platform: Platform, confirmed = true): void =>
  usePlatformStore.getState().setPlatform(platform, confirmed);
/** React hook: re-renders when the platform changes (at most once, guess → backend). */
export const usePlatform = (): Platform => usePlatformStore((s) => s.platform);

export const isMac = (platform: Platform = getPlatform()): boolean => platform === "macos";
export const useIsMac = (): boolean => usePlatformStore((s) => s.platform === "macos");

/**
 * Whether features built on Windows-only services (SetupAPI driver inventory, Windows
 * Update, the Windows tools the assistant can open) are offered.
 */
export const hasWindowsFeatures = (platform: Platform = getPlatform()): boolean => platform === "windows";
