import { useEffect, useState } from "react";

import { listAiProviderCatalog } from "../../api/aiProviders";
import type { AiProviderCatalogEntry, UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { FieldRow, SelectField } from "../../ui";

type AiSettingsProps = {
  settings: UserSettings | null;
  onYoutubeTranscriptionModelChange: (model: string) => void;
  onYoutubeTranscriptionTimeoutChange: (timeoutSeconds: number) => void;
  onGeneralAnalysisProviderChange: (provider: string) => void;
  onGeneralAnalysisModelChange: (model: string) => void;
  onGeneralAnalysisTimeoutChange: (timeoutSeconds: number) => void;
};

export function AiSettings({
  settings,
  onYoutubeTranscriptionModelChange,
  onYoutubeTranscriptionTimeoutChange,
  onGeneralAnalysisProviderChange,
  onGeneralAnalysisModelChange,
  onGeneralAnalysisTimeoutChange,
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
      </FieldRow>
    </section>
  );
}
