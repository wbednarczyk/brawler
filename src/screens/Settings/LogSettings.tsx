import type { UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { pluralNoun, FILE_FORMS } from "../../shared/locale/plural";
import { Figure, FieldRow, InfoGrid, SelectField } from "../../ui";

type LogSettingsProps = {
  settings: UserSettings | null;
  onLogLevelChange: (level: string) => void;
  onLogMaxFilesChange: (maxFiles: number) => void;
  onLogMaxFileBytesChange: (maxFileBytes: number) => void;
};

// Persisted values are the Rust-side enum tokens, unchanged — only the
// DISPLAY text is product language (docs/plans/f4c-contracts/s4-settings-pass-banner.md item 1).
const logLevels = ["off", "error", "warn", "info", "debug", "trace"];
const logLevelLabels: Record<string, string> = {
  off: "Off",
  error: "Errors only",
  warn: "Warnings",
  info: "Normal",
  debug: "Detailed",
  trace: "Everything",
};
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
  const { text, locale } = useLocale();
  const maxFiles = settings?.logs.maxFiles ?? 5;
  const maxFileMb = Math.round((settings?.logs.maxFileBytes ?? 5_242_880) / 1_048_576);

  return (
    <section className="settings-group" aria-labelledby="settings-logs-title">
      <h2 id="settings-logs-title">{text("Logs")}</h2>
      <p className="settings-note">{text("Activity records stay on this computer. Nothing is sent anywhere.")}</p>
      <FieldRow>
        <SelectField
          aria-label={text("Detail level")}
          label={text("Detail level")}
          value={settings?.logs.level ?? "info"}
          onChange={(event) => onLogLevelChange(event.target.value)}
        >
          {logLevels.map((level) => (
            <option key={level} value={level}>
              {text(logLevelLabels[level])}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("Log file count")}
          label={text("Log files")}
          value={maxFiles}
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
      <InfoGrid
        className="settings-grid"
        items={[
          {
            label: text("History kept"),
            value: (
              <>
                <Figure value={maxFiles} /> {pluralNoun(locale, maxFiles, FILE_FORMS)} ×{" "}
                <Figure value={maxFileMb} /> {text("MB")}
              </>
            ),
          },
        ]}
      />
    </section>
  );
}
