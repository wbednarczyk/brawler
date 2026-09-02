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

// Option labels read as seconds (sol fix1 item 2) — the persisted value stays
// the raw millisecond figure the API expects; only the DISPLAY text changes.
// A whole-second value renders bare ("1 s" … "60 s"); a sub-second value
// (e.g. 500 ms — not an offered option today, but a real input this
// formatter must handle correctly) renders with one decimal ("0.5 s")
// instead of truncating to "0 s".
function formatSecondsLabel(ms: number): string {
  const seconds = ms / 1000;
  const rounded = Number.isInteger(seconds) ? String(seconds) : seconds.toFixed(1);
  return `${rounded} s`;
}

export function DatabaseSettings({
  settings,
  onDbMaxConnectionsChange,
  onDbBusyTimeoutMsChange,
  onDbAcquireTimeoutMsChange,
  onResetDatabaseSettings,
}: DatabaseSettingsProps) {
  const { text } = useLocale();
  const database = settings?.database;
  const busyTimeoutMs = database?.busyTimeoutMs ?? 5000;
  const acquireTimeoutMs = database?.acquireTimeoutMs ?? 10000;

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
          value={busyTimeoutMs}
          onChange={(event) => onDbBusyTimeoutMsChange(Number(event.target.value))}
        >
          {busyTimeoutOptions.map((value) => (
            <option key={value} value={value}>
              {formatSecondsLabel(value)}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("Wait to start")}
          label={text("Wait to start")}
          value={acquireTimeoutMs}
          onChange={(event) => onDbAcquireTimeoutMsChange(Number(event.target.value))}
        >
          {acquireTimeoutOptions.map((value) => (
            <option key={value} value={value}>
              {formatSecondsLabel(value)}
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
