import { useEffect, useState } from "react";

import { listAiProviderCatalog } from "../../api/aiProviders";
import { OPENAI_COMPATIBLE_PROVIDER_ID } from "../../api/credentials";
import type { AiProviderCatalogEntry, UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import {
  FieldRow,
  Hint,
  RangeField,
  SegmentedControl,
  SegmentedControlOption,
  SelectField,
  TextField,
} from "../../ui";

// History-sweep AI budget presets (tier-4 call units per sweep). Visual-first
// config UX (docs/ui-authoring.md): clickable presets + a slider + numeric
// input, all bound to the same value. The backend clamps to [0, 500]; `0`
// means unlimited (ADR 0077 §6).
const AI_CALL_LIMIT_PRESETS = [0, 10, 30, 100];
const AI_CALL_LIMIT_DEFAULT = 30;
const clampAiCallLimit = (value: number): number =>
  Number.isNaN(value) ? AI_CALL_LIMIT_DEFAULT : Math.min(500, Math.max(0, Math.round(value)));

type AiSettingsProps = {
  settings: UserSettings | null;
  onYoutubeTranscriptionModelChange: (model: string) => void;
  onYoutubeTranscriptionTimeoutChange: (timeoutSeconds: number) => void;
  onGeneralAnalysisProviderChange: (provider: string) => void;
  onGeneralAnalysisModelChange: (model: string) => void;
  onGeneralAnalysisTimeoutChange: (timeoutSeconds: number) => void;
  onEspiAiFallbackChange: (enabled: boolean) => void;
  onOpenAiCompatibleBaseUrlChange: (baseUrl: string) => void;
  onHistorySweepAiCallLimitChange: (historySweepAiCallLimit: number) => void;
};

export function AiSettings({
  settings,
  onYoutubeTranscriptionModelChange,
  onYoutubeTranscriptionTimeoutChange,
  onGeneralAnalysisProviderChange,
  onGeneralAnalysisModelChange,
  onGeneralAnalysisTimeoutChange,
  onEspiAiFallbackChange,
  onOpenAiCompatibleBaseUrlChange,
  onHistorySweepAiCallLimitChange,
}: AiSettingsProps) {
  const { t, text } = useLocale();
  const [catalog, setCatalog] = useState<AiProviderCatalogEntry[]>([]);

  useEffect(() => {
    let active = true;
    listAiProviderCatalog()
      .then((entries) => {
        if (active) {
          setCatalog(entries);
        }
      })
      .catch(() => {
        if (active) {
          setCatalog([]);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  const selectedProvider = settings?.aiProviders.generalAnalysisProvider ?? "";
  const selectedProviderEntry = catalog.find((entry) => entry.providerId === selectedProvider);
  const analysisModels = selectedProviderEntry?.models ?? [];
  const isGeneralProviderCompatible = selectedProvider === OPENAI_COMPATIBLE_PROVIDER_ID;

  // Free-text settings fields edit a local draft and commit on blur, rather
  // than calling `update_settings` per keystroke: a controlled field bound
  // directly to the round-tripped settings value can't be typed into, because
  // the async save reverts each keystroke and backend validation rejects
  // every partial value on the way (docs/ui-authoring.md).
  const savedGeneralModel = settings?.aiProviders.generalAnalysisModel ?? "";
  const [generalModelDraft, setGeneralModelDraft] = useState(savedGeneralModel);
  useEffect(() => {
    setGeneralModelDraft(savedGeneralModel);
  }, [savedGeneralModel]);

  const savedBaseUrl = settings?.aiProviders.openaiCompatibleBaseUrl ?? "";
  const [baseUrlDraft, setBaseUrlDraft] = useState(savedBaseUrl);
  useEffect(() => {
    setBaseUrlDraft(savedBaseUrl);
  }, [savedBaseUrl]);

  const historySweepAiCallLimit = settings?.historySweepAiCallLimit ?? AI_CALL_LIMIT_DEFAULT;

  return (
    <section className="settings-group" aria-labelledby="settings-ai-title">
      <h2 id="settings-ai-title">{t("settings.ai.title")}</h2>
      <FieldRow>
        <SelectField
          aria-label={text("Gemini transcription model")}
          label={text("Gemini transcription model")}
          value={settings?.aiProviders.youtubeTranscriptionModel ?? "gemini-3.5-flash"}
          onChange={(event) => onYoutubeTranscriptionModelChange(event.target.value)}
        >
          <option value="gemini-2.5-flash-lite">Gemini 2.5 Flash-Lite</option>
          <option value="gemini-2.5-flash">Gemini 2.5 Flash</option>
          <option value="gemini-3.1-flash-lite">Gemini 3.1 Flash-Lite</option>
          <option value="gemini-3.5-flash">Gemini 3.5 Flash</option>
        </SelectField>
        <SelectField
          aria-label={text("Gemini transcription timeout")}
          label={text("Gemini transcription timeout")}
          value={settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}
          onChange={(event) => onYoutubeTranscriptionTimeoutChange(Number(event.target.value))}
        >
          <option value={45}>45 {text("seconds")}</option>
          <option value={90}>90 {text("seconds")}</option>
          <option value={180}>3 {text("minutes")}</option>
          <option value={300}>5 {text("minutes")}</option>
          <option value={600}>10 {text("minutes")}</option>
        </SelectField>
        <SelectField
          aria-label={text("General AI provider")}
          label={text("General AI provider")}
          value={selectedProvider}
          onChange={(event) => onGeneralAnalysisProviderChange(event.target.value)}
        >
          <option value="">{text("Not configured")}</option>
          {catalog.map((entry) => (
            <option key={entry.providerId} value={entry.providerId}>
              {entry.label}
            </option>
          ))}
        </SelectField>
        {isGeneralProviderCompatible ? (
          <TextField
            aria-label={text("General AI model")}
            label={text("General AI model")}
            value={generalModelDraft}
            onChange={(event) => setGeneralModelDraft(event.target.value)}
            onBlur={() => {
              if (generalModelDraft !== savedGeneralModel) {
                onGeneralAnalysisModelChange(generalModelDraft);
              }
            }}
          />
        ) : (
          <SelectField
            aria-label={text("General AI model")}
            label={text("General AI model")}
            value={settings?.aiProviders.generalAnalysisModel ?? ""}
            disabled={analysisModels.length === 0}
            onChange={(event) => onGeneralAnalysisModelChange(event.target.value)}
          >
            {analysisModels.length === 0 ? (
              <option value="">{text("Select a provider first")}</option>
            ) : null}
            {analysisModels.map((model) => (
              <option key={model} value={model}>
                {model}
              </option>
            ))}
          </SelectField>
        )}
        <SelectField
          aria-label={text("General AI timeout")}
          label={text("General AI timeout")}
          value={settings?.aiProviders.generalAnalysisTimeoutSeconds ?? 90}
          onChange={(event) => onGeneralAnalysisTimeoutChange(Number(event.target.value))}
        >
          <option value={45}>45 {text("seconds")}</option>
          <option value={90}>90 {text("seconds")}</option>
          <option value={180}>3 {text("minutes")}</option>
          <option value={300}>5 {text("minutes")}</option>
          <option value={600}>10 {text("minutes")}</option>
        </SelectField>
        <SelectField
          aria-label={text("ESPI AI classification fallback")}
          label={text("ESPI AI classification fallback")}
          value={settings?.espiAiFallbackEnabled ? "enabled" : "disabled"}
          onChange={(event) => onEspiAiFallbackChange(event.target.value === "enabled")}
        >
          <option value="disabled">{text("Disabled")}</option>
          <option value="enabled">{text("Enabled")}</option>
        </SelectField>
      </FieldRow>
      <FieldRow>
        <TextField
          aria-label={text("OpenAI-compatible base URL")}
          label={text("OpenAI-compatible base URL")}
          placeholder="https://api.example.com/v1"
          value={baseUrlDraft}
          onChange={(event) => setBaseUrlDraft(event.target.value)}
          onBlur={() => {
            if (baseUrlDraft !== savedBaseUrl) {
              onOpenAiCompatibleBaseUrlChange(baseUrlDraft);
            }
          }}
        />
      </FieldRow>
      <Hint>
        {text(
          "Presets for common OpenAI-compatible endpoints live in the wiki. Set the API key in Credentials below.",
        )}
      </Hint>

      {/* History-sweep AI budget (ADR 0077 §6, T5.3): visual-first — clickable
          presets + a slider two-way bound to a numeric input. The backend
          clamps to [0, 500]; 0 means unlimited. */}
      <FieldRow>
        <div
          className="settings-range-control"
          role="group"
          aria-label={text("History sweep AI budget")}
        >
          <span className="settings-range-control-label">{text("History sweep AI budget")}</span>
          <SegmentedControl ariaLabel={text("History sweep AI budget presets")}>
            {AI_CALL_LIMIT_PRESETS.map((preset) => (
              <SegmentedControlOption
                key={preset}
                active={historySweepAiCallLimit === preset}
                onClick={() => onHistorySweepAiCallLimitChange(preset)}
              >
                {preset}
              </SegmentedControlOption>
            ))}
          </SegmentedControl>
          <div className="settings-range-control-custom">
            <RangeField
              aria-label={text("History sweep AI budget (slider)")}
              min={0}
              max={500}
              step={5}
              value={historySweepAiCallLimit}
              onChange={(event) =>
                onHistorySweepAiCallLimitChange(clampAiCallLimit(Number(event.target.value)))
              }
            />
            <TextField
              aria-label={text("History sweep AI budget in calls")}
              type="number"
              min={0}
              max={500}
              value={String(historySweepAiCallLimit)}
              onChange={(event) =>
                onHistorySweepAiCallLimitChange(clampAiCallLimit(Number(event.target.value)))
              }
              className="settings-range-control-input"
            />
          </div>
          <Hint>{text("AI calls per history sweep (0 = unlimited).")}</Hint>
        </div>
      </FieldRow>
    </section>
  );
}
