import type { UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { ActionButton, FieldRow, SelectField } from "../../ui";

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
      <h2 id="settings-database-title">{text("Data storage")}</h2>
      <p className="settings-note">
        {text("How hard the app works on your data at once. Applies after restart.")}
      </p>
      <FieldRow>
        <SelectField
          aria-label={text("Parallel work")}
          label={text("Parallel work")}
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
          aria-label={text("Wait when busy")}
          label={text("Wait when busy")}
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
          aria-label={text("Wait to start")}
          label={text("Wait to start")}
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
      <ActionButton kind="control" onClick={onResetDatabaseSettings}>
        {text("Reset to defaults")}
      </ActionButton>
    </section>
  );
}
