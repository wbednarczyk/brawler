import { createContext, useContext } from "react";

import { en, type LocaleKey, type LocaleResources } from "./resources/en";
import { pl } from "./resources/pl";
import { plText } from "./resources/plText";

export type { LocaleKey, LocaleResources } from "./resources/en";

export const defaultLocale = "en";
export const supportedLocales = ["en", "pl"] as const;
export type LocaleCode = (typeof supportedLocales)[number];
export const localeResources: Record<LocaleCode, LocaleResources> = {
  en,
  pl,
};

export function isSupportedLocale(locale: string): locale is LocaleCode {
  return supportedLocales.includes(locale as LocaleCode);
}

export function normalizeLocale(locale: string | null | undefined): LocaleCode {
  return locale && isSupportedLocale(locale) ? locale : defaultLocale;
}

export function localeDisplayName(locale: LocaleCode, displayLocale: LocaleCode = locale) {
  return translate(displayLocale, `settings.locale.${locale}`);
}

export function translate(locale: LocaleCode, key: LocaleKey): string {
  return translateKey(locale, key);
}

export function translateKey(locale: LocaleCode, key: string): string {
  return localeResources[locale][key as LocaleKey] ?? localeResources[defaultLocale][key as LocaleKey] ?? key;
}

export function makeTranslator(locale: LocaleCode) {
  return (key: LocaleKey) => translate(locale, key);
}

export function translateText(locale: LocaleCode, text: string): string {
  return locale === "pl" ? plText[text] ?? text : text;
}

export function makeTextTranslator(locale: LocaleCode) {
  return (text: string) => translateText(locale, text);
}

export type LocaleContextValue = {
  locale: LocaleCode;
  t: (key: LocaleKey) => string;
  text: (value: string) => string;
};

export const LocaleContext = createContext<LocaleContextValue>({
  locale: defaultLocale,
  t: makeTranslator(defaultLocale),
  text: makeTextTranslator(defaultLocale),
});

export function useLocale() {
  return useContext(LocaleContext);
}
