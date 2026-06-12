import type { UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { FieldRow, SelectField } from "../../ui";

type LogSettingsProps = {
  settings: UserSettings | null;
  onLogLevelChange: (level: string) => void;
  onLogMaxFilesChange: (maxFiles: number) => void;
  onLogMaxFileBytesChange: (maxFileBytes: number) => void;
};

const logLevels = ["off", "error", "warn", "info", "debug", "trace"];
const logFileCounts = [1, 3, 5, 10, 20];
const logFileSizes = [
  { label: "1 MiB", value: 1_048_576 },
  { label: "5 MiB", value: 5_242_880 },
  { label: "10 MiB", value: 10_485_760 },
  { label: "50 MiB", value: 52_428_800 },
  { label: "100 MiB", value: 104_857_600 },
];

export function LogSettings({
  settings,
  onLogLevelChange,
  onLogMaxFilesChange,
  onLogMaxFileBytesChange,
}: LogSettingsProps) {
  const { text } = useLocale();

  return (
    <section className="settings-group" aria-labelledby="settings-logs-title">
      <h2 id="settings-logs-title">{text("Logs")}</h2>
      <p className="settings-note">{text("Local JSON logs only. No telemetry or remote upload.")}</p>
      <FieldRow>
        <SelectField
          aria-label={text("Log level")}
          label={text("Log level")}
          value={settings?.logs.level ?? "info"}
          onChange={(event) => onLogLevelChange(event.target.value)}
        >
          {logLevels.map((level) => (
            <option key={level} value={level}>
              {level}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("Log file count")}
          label={text("Log files")}
          value={settings?.logs.maxFiles ?? 5}
          onChange={(event) => onLogMaxFilesChange(Number(event.target.value))}
        >
          {logFileCounts.map((count) => (
            <option key={count} value={count}>
              {count}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("Log file size")}
          label={text("Log file size")}
          value={settings?.logs.maxFileBytes ?? 5_242_880}
          onChange={(event) => onLogMaxFileBytesChange(Number(event.target.value))}
        >
          {logFileSizes.map((size) => (
            <option key={size.value} value={size.value}>
              {size.label}
            </option>
          ))}
        </SelectField>
      </FieldRow>
      <dl className="settings-grid">
        <div>
          <dt>{text("Current level")}</dt>
          <dd>{settings?.logs.level ?? "info"}</dd>
        </div>
        <div>
          <dt>{text("Rotation")}</dt>
          <dd>
            {settings?.logs.maxFiles ?? 5} x {formatBytes(settings?.logs.maxFileBytes ?? 5_242_880)}
          </dd>
        </div>
      </dl>
    </section>
  );
}

function formatBytes(value: number) {
  if (value >= 1_048_576) {
    return `${Math.round(value / 1_048_576)} MiB`;
  }

  return `${value} B`;
}
