import type { UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { FieldRow, SelectField } from "../../ui";

type DatabaseSettingsProps = {
  settings: UserSettings | null;
  onDbMaxConnectionsChange: (maxConnections: number) => void;
  onDbBusyTimeoutMsChange: (busyTimeoutMs: number) => void;
  onDbAcquireTimeoutMsChange: (acquireTimeoutMs: number) => void;
  onResetDatabaseSettings: () => void;
};

const maxConnectionOptions = [1, 2, 4, 8, 16];
const busyTimeoutOptions = [1000, 5000, 10000, 30000, 60000];
const acquireTimeoutOptions = [5000, 10000, 30000, 60000];

export function DatabaseSettings({
  settings,
  onDbMaxConnectionsChange,
  onDbBusyTimeoutMsChange,
  onDbAcquireTimeoutMsChange,
  onResetDatabaseSettings,
}: DatabaseSettingsProps) {
  const { text } = useLocale();
  const database = settings?.database;

  return (
    <section className="settings-group" aria-labelledby="settings-database-title">
      <h2 id="settings-database-title">{text("Database")}</h2>
      <p className="settings-note">
        {text("Advanced connection-pool tuning. Changes apply on the next app launch.")}
      </p>
      <FieldRow>
        <SelectField
          aria-label={text("Max connections")}
          label={text("Max connections")}
          value={database?.maxConnections ?? 4}
          onChange={(event) => onDbMaxConnectionsChange(Number(event.target.value))}
        >
          {maxConnectionOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("Busy timeout (ms)")}
          label={text("Busy timeout (ms)")}
          value={database?.busyTimeoutMs ?? 5000}
          onChange={(event) => onDbBusyTimeoutMsChange(Number(event.target.value))}
        >
          {busyTimeoutOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("Acquire timeout (ms)")}
          label={text("Acquire timeout (ms)")}
          value={database?.acquireTimeoutMs ?? 10000}
          onChange={(event) => onDbAcquireTimeoutMsChange(Number(event.target.value))}
        >
          {acquireTimeoutOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectField>
      </FieldRow>
      <button type="button" className="secondary-button" onClick={onResetDatabaseSettings}>
        {text("Reset to defaults")}
      </button>
    </section>
  );
}
