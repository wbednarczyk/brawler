import type { FeedPruneResult, UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { FieldRow, InfoGrid, SelectField } from "../../ui";

type SourceSettingsProps = {
  feedPruneRetentionDays: number;
  feedPruneResult: FeedPruneResult | null;
  settings: UserSettings | null;
  onPollIntervalChange: (pollIntervalSeconds: number) => void;
  formatPollInterval: (seconds: number) => string;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
};

export function SourceSettings({
  feedPruneRetentionDays,
  feedPruneResult,
  settings,
  onPollIntervalChange,
  formatPollInterval,
  formatTimestamp,
}: SourceSettingsProps) {
  const { t, text } = useLocale();

  return (
    <>
      <section className="settings-group" aria-labelledby="settings-sources-title">
        <h2 id="settings-sources-title">{t("settings.sources.title")}</h2>
        <FieldRow>
          <SelectField
            aria-label={text("Settings source poll interval")}
            label={text("Poll interval")}
            value={settings?.pollIntervalSeconds ?? 900}
            onChange={(event) => onPollIntervalChange(Number(event.target.value))}
          >
            <option value={300}>5 min</option>
            <option value={900}>15 min</option>
            <option value={1800}>30 min</option>
            <option value={3600}>1 hour</option>
          </SelectField>
        </FieldRow>
        <InfoGrid
          className="settings-grid"
          items={[
            {
              label: text("Poll interval"),
              value: formatPollInterval(settings?.pollIntervalSeconds ?? 900),
            },
          ]}
        />
      </section>

      <section className="settings-group" aria-labelledby="settings-cleanup-title">
        <h2 id="settings-cleanup-title">{t("settings.feedCleanup.title")}</h2>
        <InfoGrid
          className="settings-grid"
          items={[
            { label: text("Feed cleanup"), value: text("On") },
            { label: text("Feed retention"), value: `${feedPruneRetentionDays} ${text("days")}` },
            { label: text("Cleanup interval"), value: text("Daily") },
            {
              label: text("Last cleanup"),
              value: formatTimestamp(feedPruneResult?.prunedAt, text("Not run this session")),
            },
            {
              label: text("Last cleanup deleted"),
              value: feedPruneResult ? feedPruneResult.itemsDeleted : text("Not run this session"),
            },
            { label: text("Protected feed items"), value: text("Saved") },
          ]}
        />
      </section>

    </>
  );
}
