import { describe, expect, it } from "vitest";
import {
  defaultLocale,
  isSupportedLocale,
  localeDisplayName,
  makeTranslator,
  normalizeLocale,
  supportedLocales,
  translate,
  translateKey,
} from "./index";

describe("locale resources", () => {
  it("defines English as the default locale and Polish as an initial supported locale", () => {
    expect(defaultLocale).toBe("en");
    expect(supportedLocales).toEqual(["en", "pl"]);
    expect(isSupportedLocale("en")).toBe(true);
    expect(isSupportedLocale("pl")).toBe(true);
    expect(isSupportedLocale("de")).toBe(false);
  });

  it("normalizes unsupported locales to English", () => {
    expect(normalizeLocale("pl")).toBe("pl");
    expect(normalizeLocale("de")).toBe("en");
    expect(normalizeLocale(null)).toBe("en");
    expect(normalizeLocale(undefined)).toBe("en");
  });

  it("looks up typed app-owned strings for English and Polish", () => {
    expect(translate("en", "settings.title")).toBe("Settings");
    expect(translate("pl", "settings.title")).toBe("Ustawienia");
    expect(translate("pl", "companies.title")).toBe("Spółki");

    const t = makeTranslator("pl");
    expect(t("nav.companies")).toBe("Spółki");
  });

  it("has deterministic missing-key behavior", () => {
    expect(translateKey("pl", "missing.key")).toBe("missing.key");
  });

  it("formats locale names through locale resources", () => {
    expect(localeDisplayName("en", "en")).toBe("English");
    expect(localeDisplayName("pl", "en")).toBe("Polish");
    expect(localeDisplayName("en", "pl")).toBe("Angielski");
    expect(localeDisplayName("pl", "pl")).toBe("Polski");
  });
});
