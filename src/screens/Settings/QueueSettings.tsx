import type { UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { FieldRow, SelectField } from "../../ui";

type QueueSettingsProps = {
  settings: UserSettings | null;
  onSourcesWorkersChange: (workers: number) => void;
  onAutopilotWorkersChange: (workers: number) => void;
  onAiWorkersChange: (workers: number) => void;
  onAiProviderConcurrencyChange: (concurrency: number) => void;
  onResetQueueSettings: () => void;
};

const workerOptions = [1, 2, 3, 4, 6, 8];
const providerConcurrencyOptions = [1, 2, 3, 4, 6];

export function QueueSettings({
  settings,
  onSourcesWorkersChange,
  onAutopilotWorkersChange,
  onAiWorkersChange,
  onAiProviderConcurrencyChange,
  onResetQueueSettings,
}: QueueSettingsProps) {
  const { text } = useLocale();
  const queue = settings?.queue;

  return (
    <section className="settings-group" aria-labelledby="settings-queue-title">
      <h2 id="settings-queue-title">{text("Background work")}</h2>
      <p className="settings-note">
        {text(
          "Worker threads per lane and the per-AI-provider concurrency limit. Worker counts apply on the next app launch.",
        )}
      </p>
      <FieldRow>
        <SelectField
          aria-label={text("Source workers")}
          label={text("Source workers")}
          value={queue?.sourcesWorkers ?? 2}
          onChange={(event) => onSourcesWorkersChange(Number(event.target.value))}
        >
          {workerOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("Autopilot workers")}
          label={text("Autopilot workers")}
          value={queue?.autopilotWorkers ?? 3}
          onChange={(event) => onAutopilotWorkersChange(Number(event.target.value))}
        >
          {workerOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("AI workers")}
          label={text("AI workers")}
          value={queue?.aiWorkers ?? 2}
          onChange={(event) => onAiWorkersChange(Number(event.target.value))}
        >
          {workerOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectField>
        <SelectField
          aria-label={text("Max concurrent calls per AI provider")}
          label={text("Max concurrent calls per AI provider")}
          value={queue?.aiProviderConcurrency ?? 2}
          onChange={(event) => onAiProviderConcurrencyChange(Number(event.target.value))}
        >
          {providerConcurrencyOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectField>
      </FieldRow>
      <button type="button" className="secondary-button" onClick={onResetQueueSettings}>
        {text("Reset to defaults")}
      </button>
    </section>
  );
}
