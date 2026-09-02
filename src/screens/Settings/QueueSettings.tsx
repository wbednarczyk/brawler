import type { UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { ActionButton, FieldRow, SelectField } from "../../ui";

type QueueSettingsProps = {
  settings: UserSettings | null;
  onSourcesWorkersChange: (workers: number) => void;
  onAutopilotWorkersChange: (workers: number) => void;
  onResetQueueSettings: () => void;
};

const workerOptions = [1, 2, 3, 4, 6, 8];

export function QueueSettings({
  settings,
  onSourcesWorkersChange,
  onAutopilotWorkersChange,
  onResetQueueSettings,
}: QueueSettingsProps) {
  const { text } = useLocale();
  const queue = settings?.queue;

  return (
    <section className="settings-group" aria-labelledby="settings-queue-title">
      <h2 id="settings-queue-title">{text("Background work")}</h2>
      <p className="settings-note">
        {text(
          "How many things run at once in the background. Applies after restart.",
        )}
      </p>
      <FieldRow>
        <SelectField
          aria-label={text("Source refreshes at once")}
          label={text("Source refreshes at once")}
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
          aria-label={text("Autopilot tasks at once")}
          label={text("Autopilot tasks at once")}
          value={queue?.autopilotWorkers ?? 3}
          onChange={(event) => onAutopilotWorkersChange(Number(event.target.value))}
        >
          {workerOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </SelectField>
      </FieldRow>
      <ActionButton kind="control" onClick={onResetQueueSettings}>
        {text("Reset to defaults")}
      </ActionButton>
    </section>
  );
}
