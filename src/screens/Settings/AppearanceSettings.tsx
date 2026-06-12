import type { AccentPalette, AppLocale, Theme } from "../../api/types";
import { accentPaletteOptions } from "../../app/theme";
import { localeDisplayName, supportedLocales, useLocale, type LocaleKey } from "../../shared/locale";
import { FieldRow, SelectField } from "../../ui";

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
      <FieldRow>
        <SelectField
          aria-label={text("Settings theme")}
          label={t("settings.appearance.theme")}
          value={theme}
          onChange={(event) => onThemeChange(event.target.value as Theme)}
        >
          <option value="dark">{t("theme.dark")}</option>
          <option value="light">{t("theme.light")}</option>
          <option value="system">{t("theme.system")}</option>
        </SelectField>
        <SelectField
          aria-label={text("Settings palette")}
          label={t("settings.appearance.palette")}
          value={accentPalette}
          onChange={(event) => onAccentPaletteChange(event.target.value as AccentPalette)}
        >
          {accentPaletteOptions.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("Settings locale")}
          label={t("settings.appearance.locale")}
          value={locale}
          onChange={(event) => onLocaleChange(event.target.value as AppLocale)}
        >
          {supportedLocales.map((supportedLocale) => (
            <option key={supportedLocale} value={supportedLocale}>
              {localeDisplayName(supportedLocale, locale)}
            </option>
          ))}
        </SelectField>
      </FieldRow>
    </section>
  );
}
