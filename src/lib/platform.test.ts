import { afterEach, describe, expect, it } from "vitest";
import { getPlatform, hasWindowsFeatures, isMac, platformFromUserAgent, setPlatform, usePlatformStore } from "./platform";
import { hasCommandModifier, shortcutLabel } from "./shortcuts";

// User agents of the two WebViews the app ships with.
const WEBVIEW2 =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";
const WKWEBVIEW = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)";

describe("platform guess from the user agent", () => {
  it("recognises WebView2 on Windows and WKWebView on macOS", () => {
    expect(platformFromUserAgent(WEBVIEW2)).toBe("windows");
    // WKWebView reports "Intel Mac OS X" on Apple silicon as well.
    expect(platformFromUserAgent(WKWEBVIEW)).toBe("macos");
  });

  it("falls back to linux or other, never to windows", () => {
    expect(platformFromUserAgent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15")).toBe("linux");
    expect(platformFromUserAgent("Node.js/24")).toBe("other");
    expect(platformFromUserAgent("")).toBe("other");
  });
});

describe("platform store", () => {
  afterEach(() => setPlatform("windows"));

  it("is replaced by the backend answer and marked confirmed", () => {
    setPlatform("linux", false);
    expect(usePlatformStore.getState().confirmed).toBe(false);
    setPlatform("macos");
    expect(getPlatform()).toBe("macos");
    expect(usePlatformStore.getState().confirmed).toBe(true);
    expect(isMac()).toBe(true);
  });

  it("offers Windows-only features on Windows only", () => {
    expect(hasWindowsFeatures("windows")).toBe(true);
    expect(hasWindowsFeatures("macos")).toBe(false);
    expect(hasWindowsFeatures("linux")).toBe(false);
    expect(hasWindowsFeatures("other")).toBe(false);
  });
});

const key = (patch: Partial<Record<"ctrlKey" | "metaKey" | "altKey" | "shiftKey", boolean>>) => ({
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...patch,
});

describe("command modifier", () => {
  it("is Ctrl on Windows and Cmd on macOS", () => {
    expect(hasCommandModifier(key({ ctrlKey: true }), "windows")).toBe(true);
    expect(hasCommandModifier(key({ metaKey: true }), "windows")).toBe(false);
    expect(hasCommandModifier(key({ metaKey: true }), "macos")).toBe(true);
    // Ctrl+<n> switches desktops on macOS: never taken over.
    expect(hasCommandModifier(key({ ctrlKey: true }), "macos")).toBe(false);
  });

  it("rejects extra modifiers", () => {
    expect(hasCommandModifier(key({ ctrlKey: true, shiftKey: true }), "windows")).toBe(false);
    expect(hasCommandModifier(key({ ctrlKey: true, altKey: true }), "windows")).toBe(false);
    expect(hasCommandModifier(key({ ctrlKey: true, metaKey: true }), "windows")).toBe(false);
    expect(hasCommandModifier(key({ metaKey: true, ctrlKey: true }), "macos")).toBe(false);
    expect(hasCommandModifier(key({ metaKey: true, altKey: true }), "macos")).toBe(false);
  });

  it("is written ⌘ on macOS and Ctrl+ elsewhere", () => {
    expect(shortcutLabel("b", "macos")).toBe("⌘B");
    expect(shortcutLabel("3", "macos")).toBe("⌘3");
    expect(shortcutLabel("b", "windows")).toBe("Ctrl+B");
    expect(shortcutLabel("8", "windows")).toBe("Ctrl+8");
  });
});
