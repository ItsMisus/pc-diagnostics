import { describe, expect, it } from "vitest";
import { dictionaries } from "./locales";
import { setLang, translatorFor } from "./index";

const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe("dictionaries", () => {
  const langs = Object.keys(dictionaries) as (keyof typeof dictionaries)[];

  it("every language has the same keys and placeholders as Italian", () => {
    for (const lang of langs) {
      for (const [ns, ref] of Object.entries(dictionaries.it)) {
        const other = (dictionaries[lang] as Record<string, Record<string, string>>)[ns] ?? {};
        expect(Object.keys(other).sort(), `${lang}/${ns} keys`).toEqual(Object.keys(ref).sort());
        for (const [key, text] of Object.entries(ref as Record<string, string>)) {
          expect(placeholders(other[key] ?? ""), `${lang}/${ns}.${key} placeholders`).toEqual(placeholders(text));
        }
      }
    }
  });

  it("plural keys come in pairs", () => {
    for (const lang of langs) {
      for (const [ns, dict] of Object.entries(dictionaries[lang])) {
        const keys = Object.keys(dict);
        for (const k of keys.filter((k) => k.endsWith("_one"))) {
          expect(keys, `${lang}/${ns}.${k}`).toContain(k.replace(/_one$/, "_other"));
        }
      }
    }
  });

  it("no text is empty", () => {
    for (const lang of langs) {
      for (const [ns, dict] of Object.entries(dictionaries[lang])) {
        for (const [key, text] of Object.entries(dict as Record<string, string>)) {
          expect(text.trim(), `${lang}/${ns}.${key}`).not.toBe("");
        }
      }
    }
  });
});

describe("translator", () => {
  it("falls back to the namespaced key when a text is missing", () => {
    setLang("en");
    const t = translatorFor("common") as unknown as (k: string) => string;
    expect(t("__missing__")).toBe("common.__missing__");
    setLang("it");
  });
});
