import type { Theme, UserSettings } from "../../api/types";

type AppearanceSettingsProps = {
  settings: UserSettings | null;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
};

export function AppearanceSettings({
  settings,
  theme,
  onThemeChange,
}: AppearanceSettingsProps) {
  return (
    <section className="settings-group" aria-labelledby="settings-appearance-title">
      <h2 id="settings-appearance-title">Appearance</h2>
      <div className="settings-row">
        <label>
          Theme
          <select
            aria-label="Settings theme"
            value={theme}
            onChange={(event) => onThemeChange(event.target.value as Theme)}
          >
            <option value="dark">Dark</option>
            <option value="light">Light</option>
            <option value="system">System</option>
          </select>
        </label>
        <div className="settings-summary">
          <span>Palette</span>
          <strong>{settings?.accentPalette ?? "night-neon"}</strong>
        </div>
      </div>
    </section>
  );
}
