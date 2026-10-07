// ============================================================
// i18n/index.ts
// English-only i18n: single dictionary, no locale switching.
// Kept the same hook API (useT/useLang/useFormatters) so callers
// don't need changes.
// ============================================================
import {
  createContext,
  createElement,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
} from "react";
import en from "./en";
import type { I18nFormatters, MessageRuntime, MessageValue, Vars } from "./runtime";
import type { Dict, DictKey } from "./types";

export interface LocaleMeta {
  label: string;
  tag: string;
}

export const LOCALES = {
  en: { label: "English", tag: "en" },
} as const satisfies Record<string, LocaleMeta>;

export type Lang = keyof typeof LOCALES;

const DEFAULT_LANG: Lang = "en";

// ---- translate -----------------------------------------------

function interpolate(template: string, vars?: Vars): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (_, k) => (k in vars ? String(vars[k]) : `{${k}}`));
}

function createFormatters(): I18nFormatters {
  const locale = LOCALES[DEFAULT_LANG].tag;

  return {
    formatDateTime: (value, options) =>
      new Intl.DateTimeFormat(locale, {
        dateStyle: "medium",
        timeStyle: "short",
        ...options,
      }).format(value instanceof Date ? value : new Date(value)),
    formatList: (values, options) =>
      new Intl.ListFormat(locale, {
        style: "short",
        type: "conjunction",
        ...options,
      }).format(Array.from(values)),
    formatNumber: (value, options) => new Intl.NumberFormat(locale, options).format(value),
  };
}

function createRuntime(): MessageRuntime {
  const formatters = createFormatters();
  return {
    formatters,
    interpolate,
    locale: LOCALES[DEFAULT_LANG].tag,
    t: (key, vars) => translate(DEFAULT_LANG, key as DictKey, vars),
  };
}

function renderMessage(
  message: MessageValue,
  vars: Vars | undefined,
  runtime: MessageRuntime,
): string {
  return typeof message === "function"
    ? message(vars, runtime)
    : runtime.interpolate(message, vars);
}

export function translate(_lang: Lang, key: DictKey, vars?: Vars): string {
  const raw = (en as Dict)[key] ?? key;
  if (typeof raw === "string" && raw === key && !vars) return raw;
  return renderMessage(raw as MessageValue, vars, createRuntime());
}

export function translateCurrent(key: DictKey, vars?: Vars): string {
  return translate(DEFAULT_LANG, key, vars);
}

export async function loadLocale(_lang: Lang): Promise<Partial<Dict>> {
  return en;
}

export function resolvePreferredLang(_candidates: readonly string[]): Lang | null {
  return DEFAULT_LANG;
}

// ---- context -------------------------------------------------

interface I18nCtx {
  formatters: I18nFormatters;
  lang: Lang;
  setLang: (l: Lang) => void;
  t: (key: DictKey, vars?: Vars) => string;
}

export type Translate = I18nCtx["t"];

const I18nContext = createContext<I18nCtx>({
  formatters: createFormatters(),
  lang: DEFAULT_LANG,
  setLang: () => {},
  t: (key) => key,
});

// ---- provider ------------------------------------------------

export function I18nProvider({ children }: { children: ReactNode }) {
  const t = useCallback((key: DictKey, vars?: Vars) => translate(DEFAULT_LANG, key, vars), []);
  const formatters = useMemo(() => createFormatters(), []);
  const setLang = useCallback((_l: Lang) => {}, []);

  const ctx = useMemo<I18nCtx>(
    () => ({ formatters, lang: DEFAULT_LANG, setLang, t }),
    [formatters, setLang, t],
  );

  return createElement(I18nContext.Provider, { value: ctx }, children);
}

// ---- hook ----------------------------------------------------

export function useT(): (key: DictKey, vars?: Vars) => string {
  return useContext(I18nContext).t;
}

export function useLang(): { lang: Lang; setLang: (l: Lang) => void } {
  const { lang, setLang } = useContext(I18nContext);
  return { lang, setLang };
}

export function useFormatters(): I18nFormatters {
  return useContext(I18nContext).formatters;
}

export function useI18n(): I18nCtx {
  return useContext(I18nContext);
}

export type { I18nFormatters, MessageRuntime, MessageValue, Vars } from "./runtime";
export type { Dict, DictKey } from "./types";
