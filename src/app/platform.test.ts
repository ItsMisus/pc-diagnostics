import { afterEach, describe, expect, it, vi } from "vitest";
import { getPlatform, setPlatform } from "@/lib/platform";
import { commands } from "@/services/commands";
import type { AppInfo } from "@/types/generated/AppInfo";
import { isSectionAvailable, SECTIONS, visibleSections } from "./sections";
import { resolveShortcut, sectionShortcutKey } from "./shortcuts";
import { useAppStore } from "./store";

const info = (platform: AppInfo["platform"]): AppInfo => ({
  name: "PC Diagnostics",
  version: "0.1.0",
  buildProfile: "debug",
  schemaVersion: 1,
  platform,
});

afterEach(() => {
  vi.restoreAllMocks();
  setPlatform("windows");
  useAppStore.setState({ section: "overview" });
});

describe("sections per platform", () => {
  it("Windows shows all twelve sections, the control centers after Overview", () => {
    const ids = visibleSections("windows").map((s) => s.id);
    expect(ids).toEqual(SECTIONS.map((s) => s.id));
    expect(ids).toHaveLength(12);
    expect(ids.slice(0, 5)).toEqual(["overview", "performance", "cooling", "lighting", "gaming"]);
    expect(ids[8]).toBe("drivers");
  });

  it("macOS has no Drivers section and keeps the order of the others", () => {
    const ids = visibleSections("macos").map((s) => s.id);
    expect(ids).toEqual(["overview", "performance", "cooling", "lighting", "gaming", "hardware", "benchmarks", "processes", "diagnostics", "history", "settings"]);
    expect(isSectionAvailable("drivers", "macos")).toBe(false);
    expect(isSectionAvailable("drivers", "windows")).toBe(true);
  });

  it("an unknown platform hides Windows-only sections too", () => {
    expect(isSectionAvailable("drivers", "other")).toBe(false);
    expect(isSectionAvailable("drivers", "linux")).toBe(false);
  });
});

describe("shortcuts follow the visible sections", () => {
  const press = (key: string, mods: { ctrl?: boolean; meta?: boolean } = {}) => ({
    key,
    ctrlKey: mods.ctrl ?? false,
    metaKey: mods.meta ?? false,
    altKey: false,
    shiftKey: false,
  });

  it("Windows: Ctrl+1…9, Ctrl+9 is Drivers, Ctrl+, is Settings, Ctrl+B toggles the sidebar", () => {
    expect(resolveShortcut(press("1", { ctrl: true }), "windows")).toEqual({ kind: "section", id: "overview" });
    expect(resolveShortcut(press("2", { ctrl: true }), "windows")).toEqual({ kind: "section", id: "performance" });
    expect(resolveShortcut(press("9", { ctrl: true }), "windows")).toEqual({ kind: "section", id: "drivers" });
    expect(resolveShortcut(press(",", { ctrl: true }), "windows")).toEqual({ kind: "section", id: "settings" });
    expect(resolveShortcut(press("0", { ctrl: true }), "windows")).toBeNull();
    expect(resolveShortcut(press("b", { ctrl: true }), "windows")).toEqual({ kind: "toggleSidebar" });
    // Cmd is not the Windows modifier (the Windows key).
    expect(resolveShortcut(press("1", { meta: true }), "windows")).toBeNull();
  });

  it("macOS: ⌘1…9 without Drivers, ⌘, is Settings, ⌘B toggles the sidebar, Ctrl does nothing", () => {
    expect(resolveShortcut(press("5", { meta: true }), "macos")).toEqual({ kind: "section", id: "gaming" });
    expect(resolveShortcut(press("8", { meta: true }), "macos")).toEqual({ kind: "section", id: "processes" });
    expect(resolveShortcut(press("9", { meta: true }), "macos")).toEqual({ kind: "section", id: "diagnostics" });
    expect(resolveShortcut(press(",", { meta: true }), "macos")).toEqual({ kind: "section", id: "settings" });
    expect(resolveShortcut(press("0", { meta: true }), "macos")).toBeNull();
    expect(resolveShortcut(press("B", { meta: true }), "macos")).toEqual({ kind: "toggleSidebar" });
    expect(resolveShortcut(press("1", { ctrl: true }), "macos")).toBeNull();
    expect(resolveShortcut(press("b"), "macos")).toBeNull();
  });

  it("tooltips show the same keys the shortcuts use", () => {
    expect(sectionShortcutKey("overview", 0)).toBe("1");
    expect(sectionShortcutKey("diagnostics", 8)).toBe("9");
    expect(sectionShortcutKey("history", 9)).toBeNull();
    expect(sectionShortcutKey("settings", 10)).toBe(",");
  });
});

describe("app store and platform", () => {
  it("never selects a section the platform does not have", () => {
    setPlatform("macos");
    useAppStore.getState().setSection("drivers");
    expect(useAppStore.getState().section).toBe("overview");
    useAppStore.getState().setSection("history");
    expect(useAppStore.getState().section).toBe("history");
  });

  it("leaves a section that disappears when the platform is confirmed", () => {
    useAppStore.getState().setSection("drivers");
    expect(useAppStore.getState().section).toBe("drivers");
    setPlatform("macos");
    expect(useAppStore.getState().section).toBe("overview");
  });

  it("loadPlatform applies the backend answer", async () => {
    setPlatform("other", false);
    const spy = vi.spyOn(commands, "getAppInfo").mockResolvedValue(info("macos"));
    await useAppStore.getState().loadPlatform();
    expect(spy).toHaveBeenCalledOnce();
    expect(getPlatform()).toBe("macos");
  });

  it("loadPlatform keeps the guess when the backend cannot answer", async () => {
    setPlatform("macos", false);
    vi.spyOn(commands, "getAppInfo").mockRejectedValue({ code: "notSupported", message: "x", detail: null, retriable: false });
    await useAppStore.getState().loadPlatform();
    expect(getPlatform()).toBe("macos");
  });
});
