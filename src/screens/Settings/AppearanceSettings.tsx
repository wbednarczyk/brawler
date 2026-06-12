import type { AccentPalette, AppLocale, Theme } from "../../api/types";
import { accentPaletteOptions } from "../../app/theme";
import { localeDisplayName, supportedLocales, useLocale, type LocaleKey } from "../../shared/locale";

type AppearanceSettingsProps = {
  theme: Theme;
  accentPalette: AccentPalette;
  locale: AppLocale;
  onThemeChange: (theme: Theme) => void;
  onAccentPaletteChange: (accentPalette: AccentPalette) => void;
  onLocaleChange: (locale: AppLocale) => void;
  t: (key: LocaleKey) => string;
};

export function AppearanceSettings({
  theme,
  accentPalette,
  locale,
  onThemeChange,
  onAccentPaletteChange,
  onLocaleChange,
  t,
}: AppearanceSettingsProps) {
  const { text } = useLocale();

  return (
    <section className="settings-group" aria-labelledby="settings-appearance-title">
      <h2 id="settings-appearance-title">{t("settings.appearance.title")}</h2>
      <div className="settings-row">
        <label>
          {t("settings.appearance.theme")}
          <select
            aria-label={text("Settings theme")}
            value={theme}
            onChange={(event) => onThemeChange(event.target.value as Theme)}
          >
            <option value="dark">{t("theme.dark")}</option>
            <option value="light">{t("theme.light")}</option>
            <option value="system">{t("theme.system")}</option>
          </select>
        </label>
        <label>
          {t("settings.appearance.palette")}
          <select
            aria-label={text("Settings palette")}
            value={accentPalette}
            onChange={(event) => onAccentPaletteChange(event.target.value as AccentPalette)}
          >
            {accentPaletteOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t("settings.appearance.locale")}
          <select
            aria-label={text("Settings locale")}
            value={locale}
            onChange={(event) => onLocaleChange(event.target.value as AppLocale)}
          >
            {supportedLocales.map((supportedLocale) => (
              <option key={supportedLocale} value={supportedLocale}>
                {localeDisplayName(supportedLocale, locale)}
              </option>
            ))}
          </select>
        </label>
      </div>
    </section>
  );
}
