import type { FeedPruneResult, UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { FieldRow, SelectField } from "../../ui";

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
        <dl className="settings-grid">
          <div>
            <dt>{text("Poll interval")}</dt>
            <dd>{formatPollInterval(settings?.pollIntervalSeconds ?? 900)}</dd>
          </div>
        </dl>
      </section>

      <section className="settings-group" aria-labelledby="settings-cleanup-title">
        <h2 id="settings-cleanup-title">{t("settings.feedCleanup.title")}</h2>
        <dl className="settings-grid">
          <div>
            <dt>{text("Feed cleanup")}</dt>
            <dd>{text("On")}</dd>
          </div>
          <div>
            <dt>{text("Feed retention")}</dt>
            <dd>{feedPruneRetentionDays} {text("days")}</dd>
          </div>
          <div>
            <dt>{text("Cleanup interval")}</dt>
            <dd>{text("Daily")}</dd>
          </div>
          <div>
            <dt>{text("Last cleanup")}</dt>
            <dd>{formatTimestamp(feedPruneResult?.prunedAt, text("Not run this session"))}</dd>
          </div>
          <div>
            <dt>{text("Last cleanup deleted")}</dt>
            <dd>{feedPruneResult ? feedPruneResult.itemsDeleted : text("Not run this session")}</dd>
          </div>
          <div>
            <dt>{text("Protected feed items")}</dt>
            <dd>{text("Saved")}</dd>
          </div>
        </dl>
      </section>

    </>
  );
}
