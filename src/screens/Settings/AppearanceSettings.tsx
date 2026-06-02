import type { AppLocale, Theme, UserSettings } from "../../api/types";
import { localeDisplayName, supportedLocales, useLocale, type LocaleKey } from "../../shared/locale";

type AppearanceSettingsProps = {
  settings: UserSettings | null;
  theme: Theme;
  locale: AppLocale;
  onThemeChange: (theme: Theme) => void;
  onLocaleChange: (locale: AppLocale) => void;
  t: (key: LocaleKey) => string;
};

export function AppearanceSettings({
  settings,
  theme,
  locale,
  onThemeChange,
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
        <div className="settings-summary">
          <span>{t("settings.appearance.palette")}</span>
          <strong>{settings?.accentPalette ?? "night-neon"}</strong>
        </div>
      </div>
    </section>
  );
}
