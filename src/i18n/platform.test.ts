import { afterEach, describe, expect, it } from "vitest";
import { setPlatform } from "@/lib/platform";
import { simpleError, unavailabilitySimpleText } from "@/lib/plain";
import { dictionaries, type Namespace } from "./locales";
import { setLang, translatorFor, tr } from "./index";

const placeholders = (s: string) => new Set([...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]));
const PLATFORMS = ["macos", "linux", "other"];

type Dict = Record<string, string>;
const entries = (lang: keyof typeof dictionaries) =>
  Object.entries(dictionaries[lang]).flatMap(([ns, dict]) =>
    Object.entries(dict as Dict).map(([key, text]) => ({ ns: ns as Namespace, key, text })),
  );

afterEach(() => {
  setPlatform("windows");
  setLang("it");
});

describe("platform variants (`key@platform`)", () => {
  it("name a known non-Windows platform and an existing base key", () => {
    for (const lang of ["it", "en"] as const) {
      for (const { ns, key } of entries(lang).filter((e) => e.key.includes("@"))) {
        const [base, platform] = key.split("@");
        expect(PLATFORMS, `${lang}/${ns}.${key}`).toContain(platform);
        expect(Object.keys(dictionaries[lang][ns]), `${lang}/${ns}.${key}`).toContain(base);
      }
    }
  });

  it("use only placeholders the base key's callers pass", () => {
    for (const lang of ["it", "en"] as const) {
      for (const { ns, key, text } of entries(lang).filter((e) => e.key.includes("@"))) {
        const base = (dictionaries[lang][ns] as Dict)[key.split("@")[0] ?? ""] ?? "";
        for (const p of placeholders(text)) expect(placeholders(base), `${lang}/${ns}.${key} {${p}}`).toContain(p);
      }
    }
  });

  it("replace the base text on macOS and leave Windows wording unchanged", () => {
    setPlatform("windows");
    expect(tr("settings", "languageHint")).toBe("«Sistema» segue la lingua di Windows.");
    expect(tr("settings", "keySavedDetail")).toBe("Conservata in Gestione credenziali di Windows.");
    setPlatform("macos");
    expect(tr("settings", "languageHint")).toBe("«Sistema» segue la lingua di macOS.");
    expect(tr("settings", "keySavedDetail")).toBe("Conservata nel Portachiavi di macOS.");
    expect(tr("settings", "assistantPointKey")).toContain("Portachiavi");
    setLang("en");
    expect(tr("settings", "languageHint")).toBe("“System” follows the macOS language.");
    expect(tr("settings", "assistantPointKey")).toContain("macOS Keychain");
  });

  it("apply to plural forms and to the non-React helpers", () => {
    setPlatform("macos");
    expect(simpleError("permissionDenied").explanation).toBe(
      "macOS non permette all'app di leggere questo dato con i permessi attuali.",
    );
    expect(simpleError("notSupported").title).toBe("Non disponibile su questo Mac");
    expect(unavailabilitySimpleText({ kind: "notSupported" })).toBe("Questo Mac non fornisce questo dato");
    // A variant-free key resolves to its base text.
    expect(translatorFor("diagnostics").plural("simpleImportant", 2)).toBe("2 cose importanti");
  });

  it("follow a platform change on the next lookup (no stale translator)", () => {
    const before = translatorFor("errors")("simpleTitle_platform");
    setPlatform("macos");
    const after = translatorFor("errors")("simpleTitle_platform");
    expect(before).toBe("Windows non ha risposto come previsto");
    expect(after).toBe("macOS non ha risposto come previsto");
  });
});

/**
 * Texts that still name Windows on macOS on purpose: the Windows-only assistant actions say
 * they exist only on Windows, a Windows Settings page name, and the online-checks texts (the
 * hint belongs to a toggle that macOS does not show; the note explains why it is absent).
 */
const WINDOWS_ON_PURPOSE = new Set([
  "assistant.settingsPage_windowsUpdate",
  "assistant.action_openSettingsPage",
  "assistant.action_openDeviceManager",
  "assistant.action_openDiskCleanup",
  "assistant.action_runSystemFileCheck",
  "assistant.windowsOnly",
  "settings.onlineChecksHint",
  "settings.onlineChecksWindowsOnly",
]);

describe("texts shown on macOS", () => {
  it("never present a Windows tool, service or path as the user's own (Drivers page excluded: hidden)", () => {
    setPlatform("macos");
    const wrong = /Windows|WebView2|working set|Gestione credenziali|Credential Manager|Esegui come amministratore|Run as administrator|Documenti\\|Documents\\/;
    for (const lang of ["it", "en"] as const) {
      for (const ns of Object.keys(dictionaries.it) as Namespace[]) {
        if (ns === "drivers") continue;
        const t = translatorFor(ns, lang) as unknown as (key: string) => string;
        for (const key of Object.keys(dictionaries.it[ns]).filter((k) => !k.includes("@"))) {
          if (WINDOWS_ON_PURPOSE.has(`${ns}.${key}`)) continue;
          expect(t(key), `${lang}/${ns}.${key}`).not.toMatch(wrong);
        }
      }
    }
  });
});
