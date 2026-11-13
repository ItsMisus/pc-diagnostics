import { Fragment, createElement, type ReactNode } from "react";
import { create } from "zustand";
import { getPlatform, usePlatform } from "@/lib/platform";
import type { Platform } from "@/types/generated/Platform";
import { dictionaries, type Namespace } from "./locales";

/**
 * Minimal, dependency-free i18n.
 *
 * - Texts live in `src/i18n/locales/<lang>/<namespace>.ts`, one file per feature. The
 *   Italian file is the reference: every other language must provide exactly the same keys
 *   (checked by TypeScript via `Translation<typeof it>`).
 * - Placeholders: `{name}`. Plurals: keys `<base>_one` / `<base>_other`, chosen with
 *   `Intl.PluralRules`.
 * - Components: `const t = useT("overview")`, then `t("title")`, `t("count", { n })`,
 *   `t.plural("devices", n)`, `t.rich("hint", { link: <a>…</a> })`.
 * - Outside React (stores, formatters): `tr("errors", "notFound")`. Never compute translated
 *   strings at module scope: they would not follow a language or platform change.
 * - Platform variants: a key `<key>@<platform>` (e.g. `languageHint@macos`) replaces `<key>`
 *   when the app runs on that platform (`lib/platform.ts`), in every lookup (`t`, `t.plural`
 *   on the resolved `<base>_<form>` key, `t.rich`, `tr`). Callers always use the base key; the
 *   base text is the Windows wording. A variant needs a base key and may use only the base
 *   key's placeholders (unit test), and every language has the same variants.
 *
 * Adding a language: add it to `Lang`, `LOCALE_TAGS` and `locales/index.ts`, then copy
 * `locales/en` and translate.
 */
export type Lang = "it" | "en";

type Dict = Record<string, string>;
type Params = Record<string, string | number>;
type RichParams = Record<string, ReactNode>;

/** Base keys only: platform variants (`key@macos`) are picked by the lookup, never named by callers. */
type Keys<N extends Namespace> = Exclude<Extract<keyof (typeof dictionaries)["it"][N], string>, `${string}@${string}`>;
type PluralBase<K extends string> = K extends `${infer B}_one` ? B : never;

const LOCALE_TAGS: Record<Lang, string> = { it: "it-IT", en: "en-US" };

interface LangStore {
  lang: Lang;
  setLang: (lang: Lang) => void;
}

/**
 * The effective language. Seeded from the WebView language (which follows the OS), then
 * replaced by the backend's answer (`get_language`) so frontend and backend texts agree.
 */
export const useLangStore = create<LangStore>((set) => ({
  lang: initialLang(),
  setLang: (lang) => set({ lang }),
}));

function initialLang(): Lang {
  const nav = typeof navigator !== "undefined" ? navigator.language : "";
  return nav.toLowerCase().startsWith("it") ? "it" : "en";
}

export const getLang = (): Lang => useLangStore.getState().lang;
export const setLang = (lang: Lang): void => useLangStore.getState().setLang(lang);
export const useLang = (): Lang => useLangStore((s) => s.lang);

/** BCP 47 tag of the current language, for `Intl` and `toLocale*` calls. */
export const localeTag = (lang: Lang = getLang()): string => LOCALE_TAGS[lang];

function lookup(lang: Lang, platform: Platform, ns: Namespace, key: string): string {
  const own = dictionaries[lang][ns] as Dict;
  const it = dictionaries.it[ns] as Dict;
  // Windows texts are the base keys, so Windows never looks for a variant.
  if (platform !== "windows") {
    const variant = `${key}@${platform}`;
    const text = own[variant] ?? it[variant];
    if (text !== undefined) return text;
  }
  // Missing keys fall back to Italian, then to the key itself (visible, never blank).
  return own[key] ?? it[key] ?? `${ns}.${key}`;
}

function fill(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (m, name: string) => (name in params ? String(params[name]) : m));
}

function fillRich(text: string, params: RichParams): ReactNode {
  const parts = text.split(/(\{\w+\})/g);
  return createElement(
    Fragment,
    null,
    ...parts.map((part, i) => {
      const m = /^\{(\w+)\}$/.exec(part);
      const value = m?.[1] !== undefined && m[1] in params ? params[m[1]] : part;
      return createElement(Fragment, { key: i }, value);
    }),
  );
}

export interface Translator<N extends Namespace> {
  (key: Keys<N>, params?: Params): string;
  /** `key_one` / `key_other` by `count`; `{count}` is available as a placeholder. */
  plural: (base: PluralBase<Keys<N>>, count: number, params?: Params) => string;
  /** Placeholders may be React nodes (links, emphasis). */
  rich: (key: Keys<N>, params: RichParams) => ReactNode;
}

function makeTranslator<N extends Namespace>(lang: Lang, platform: Platform, ns: N): Translator<N> {
  const t = ((key: string, params?: Params) => fill(lookup(lang, platform, ns, key), params)) as unknown as Translator<N>;
  t.plural = (base, count, params) => {
    const form = new Intl.PluralRules(LOCALE_TAGS[lang]).select(count) === "one" ? "one" : "other";
    return fill(lookup(lang, platform, ns, `${base}_${form}`), { count, ...params });
  };
  t.rich = (key, params) => fillRich(lookup(lang, platform, ns, key), params);
  return t;
}

const cache = new Map<string, Translator<Namespace>>();

function translator<N extends Namespace>(lang: Lang, platform: Platform, ns: N): Translator<N> {
  const id = `${lang}:${platform}:${ns}`;
  let t = cache.get(id);
  if (!t) {
    t = makeTranslator(lang, platform, ns) as Translator<Namespace>;
    cache.set(id, t);
  }
  return t as unknown as Translator<N>;
}

/**
 * React hook: re-renders on a language or platform change. The returned function is stable
 * per language and platform.
 */
export function useT<N extends Namespace>(ns: N): Translator<N> {
  const lang = useLang();
  const platform = usePlatform();
  return translator(lang, platform, ns);
}

/** Non-React access with the current language and platform. */
export function tr<N extends Namespace>(ns: N, key: Keys<N>, params?: Params): string {
  return translator(getLang(), getPlatform(), ns)(key, params);
}

/** Non-React translator bound to the current language (e.g. inside a store action) and platform. */
export function translatorFor<N extends Namespace>(ns: N, lang: Lang = getLang(), platform: Platform = getPlatform()): Translator<N> {
  return translator(lang, platform, ns);
}

export type { Namespace };
