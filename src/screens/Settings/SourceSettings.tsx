import type { FeedPruneResult, UserSettings } from "../../api/types";

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
  return (
    <>
      <section className="settings-group" aria-labelledby="settings-sources-title">
        <h2 id="settings-sources-title">Sources</h2>
        <div className="settings-row">
          <label>
            Poll interval
            <select
              aria-label="Settings source poll interval"
              value={settings?.pollIntervalSeconds ?? 900}
              onChange={(event) => onPollIntervalChange(Number(event.target.value))}
            >
              <option value={300}>5 min</option>
              <option value={900}>15 min</option>
              <option value={1800}>30 min</option>
              <option value={3600}>1 hour</option>
            </select>
          </label>
        </div>
        <dl className="settings-grid">
          <div>
            <dt>Settings source</dt>
            <dd>{settings?.settingsSource ?? "sqlite"}</dd>
          </div>
          <div>
            <dt>Poll interval</dt>
            <dd>{formatPollInterval(settings?.pollIntervalSeconds ?? 900)}</dd>
          </div>
        </dl>
      </section>

      <section className="settings-group" aria-labelledby="settings-cleanup-title">
        <h2 id="settings-cleanup-title">Feed Cleanup</h2>
        <dl className="settings-grid">
          <div>
            <dt>Feed cleanup</dt>
            <dd>On</dd>
          </div>
          <div>
            <dt>Feed retention</dt>
            <dd>{feedPruneRetentionDays} days</dd>
          </div>
          <div>
            <dt>Cleanup interval</dt>
            <dd>Daily</dd>
          </div>
          <div>
            <dt>Last cleanup</dt>
            <dd>{formatTimestamp(feedPruneResult?.prunedAt, "Not run this session")}</dd>
          </div>
          <div>
            <dt>Last cleanup deleted</dt>
            <dd>{feedPruneResult ? feedPruneResult.itemsDeleted : "Not run this session"}</dd>
          </div>
          <div>
            <dt>Protected feed items</dt>
            <dd>Saved</dd>
          </div>
        </dl>
      </section>

      <section className="settings-group" aria-labelledby="settings-import-title">
        <h2 id="settings-import-title">Import And Export</h2>
        <dl className="settings-grid">
          <div>
            <dt>YAML import/export</dt>
            <dd>{settings?.yamlImportExportStatus ?? "accepted_deferred"}</dd>
          </div>
          <div>
            <dt>Settings format</dt>
            <dd>{settings?.settingsImportExportFormat ?? "yaml"}</dd>
          </div>
        </dl>
      </section>
    </>
  );
}
