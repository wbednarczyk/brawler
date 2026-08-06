import type { UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import {
  FieldRow,
  Hint,
  InfoGrid,
  RangeField,
  SegmentedControl,
  SegmentedControlOption,
  SelectField,
  TextField,
} from "../../ui";

// Backfill-depth presets (years). Visual-first config UX (docs/ui-authoring.md):
// clickable presets paired with a slider + numeric input, all bound to the same
// value. The backend clamps to [1, 10] (ADR 0077 §3).
const BACKFILL_YEAR_PRESETS = [1, 3, 5, 10];
const BACKFILL_YEARS_DEFAULT = 3;
const clampBackfillYears = (value: number): number =>
  Number.isNaN(value) ? BACKFILL_YEARS_DEFAULT : Math.min(10, Math.max(1, Math.round(value)));

type SourceSettingsProps = {
  settings: UserSettings | null;
  onPollIntervalChange: (pollIntervalSeconds: number) => void;
  onBackfillYearsChange: (backfillYears: number) => void;
  formatPollInterval: (seconds: number) => string;
};

export function SourceSettings({
  settings,
  onPollIntervalChange,
  onBackfillYearsChange,
  formatPollInterval,
}: SourceSettingsProps) {
  const { t, text } = useLocale();
  const backfillYears = settings?.backfillYears ?? BACKFILL_YEARS_DEFAULT;

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

        {/* Backfill depth (ADR 0077 §3): visual-first — clickable presets + a
            slider two-way bound to a numeric input. The backend clamps to 1–10. */}
        <FieldRow>
          <div
            className="settings-range-control"
            role="group"
            aria-label={text("Backfill history depth")}
          >
            <span className="settings-range-control-label">{text("Backfill history depth")}</span>
            <SegmentedControl ariaLabel={text("Backfill depth presets")}>
              {BACKFILL_YEAR_PRESETS.map((preset) => (
                <SegmentedControlOption
                  key={preset}
                  active={backfillYears === preset}
                  onClick={() => onBackfillYearsChange(preset)}
                >
                  {preset}
                </SegmentedControlOption>
              ))}
            </SegmentedControl>
            <div className="settings-range-control-custom">
              <RangeField
                aria-label={text("Backfill history depth (slider)")}
                min={1}
                max={10}
                step={1}
                value={backfillYears}
                onChange={(event) => onBackfillYearsChange(clampBackfillYears(Number(event.target.value)))}
              />
              <TextField
                aria-label={text("Backfill history depth in years")}
                type="number"
                min={1}
                max={10}
                value={String(backfillYears)}
                onChange={(event) => onBackfillYearsChange(clampBackfillYears(Number(event.target.value)))}
                className="settings-range-control-input"
              />
            </div>
            <Hint>{text("Years of company history to fetch (1–10).")}</Hint>
          </div>
        </FieldRow>
      </section>
    </>
  );
}
