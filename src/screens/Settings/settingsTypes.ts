import type {
  AccentPalette,
  AppLocale,
  ShortcutBindingSetting,
  Theme,
  UserSettings,
} from "../../api/types";
import type { AppShortcutReferenceItem } from "../../app/shortcuts";

export type SettingsScreenProps = {
  theme: Theme;
  accentPalette: AccentPalette;
  locale: AppLocale;
  settings: UserSettings | null;
  settingsError: string | null;
  shortcutBindings: Record<string, ShortcutBindingSetting>;
  shortcutReferences: AppShortcutReferenceItem[];
  onThemeChange: (theme: Theme) => void;
  onAccentPaletteChange: (accentPalette: AccentPalette) => void;
  onLocaleChange: (locale: AppLocale) => void;
  onPollIntervalChange: (pollIntervalSeconds: number) => void;
  onBackfillYearsChange: (backfillYears: number) => void;
  onMcpPortChange: (port: number) => void;
  onMcpWritesEnabledChange: (enabled: boolean) => void;
  onKpiAcquisitionEnabledChange: (enabled: boolean) => void;
  onShortcutBindingsChange: (
    shortcutBindings: Record<string, ShortcutBindingSetting>,
  ) => void;
  onLogLevelChange: (level: string) => void;
  onLogMaxFilesChange: (maxFiles: number) => void;
  onLogMaxFileBytesChange: (maxFileBytes: number) => void;
  onDbMaxConnectionsChange: (maxConnections: number) => void;
  onDbBusyTimeoutMsChange: (busyTimeoutMs: number) => void;
  onDbAcquireTimeoutMsChange: (acquireTimeoutMs: number) => void;
  onResetDatabaseSettings: () => void;
  onSourcesWorkersChange: (workers: number) => void;
  onAutopilotWorkersChange: (workers: number) => void;
  onResetQueueSettings: () => void;
  onImportApplied: () => void;
  formatPollInterval: (seconds: number) => string;
};
