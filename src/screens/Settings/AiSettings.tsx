import type { UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";

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

  return (
    <section className="settings-group" aria-labelledby="settings-ai-title">
      <h2 id="settings-ai-title">{t("settings.ai.title")}</h2>
      <div className="settings-row">
        <label>
          {text("Gemini transcription model")}
          <select
            aria-label={text("Gemini transcription model")}
            value={settings?.aiProviders.youtubeTranscriptionModel ?? "gemini-2.5-flash"}
            onChange={(event) => onYoutubeTranscriptionModelChange(event.target.value)}
          >
            <option value="gemini-2.5-flash-lite">Gemini 2.5 Flash-Lite</option>
            <option value="gemini-2.5-flash">Gemini 2.5 Flash</option>
            <option value="gemini-3.1-flash-lite">Gemini 3.1 Flash-Lite</option>
            <option value="gemini-3.5-flash">Gemini 3.5 Flash</option>
          </select>
        </label>
      </div>
      <div className="settings-row">
        <label>
          {text("Gemini transcription timeout")}
          <select
            aria-label={text("Gemini transcription timeout")}
            value={settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}
            onChange={(event) => onYoutubeTranscriptionTimeoutChange(Number(event.target.value))}
          >
            <option value={45}>45 {text("seconds")}</option>
            <option value={90}>90 {text("seconds")}</option>
            <option value={180}>3 {text("minutes")}</option>
            <option value={300}>5 {text("minutes")}</option>
            <option value={600}>10 {text("minutes")}</option>
          </select>
        </label>
      </div>
      <div className="settings-row">
        <label>
          {text("General AI provider")}
          <select
            aria-label={text("General AI provider")}
            value={settings?.aiProviders.generalAnalysisProvider ?? ""}
            onChange={(event) => onGeneralAnalysisProviderChange(event.target.value)}
          >
            <option value="">{text("Not configured")}</option>
            <option value="provider_gemini">{text("Gemini")}</option>
          </select>
        </label>
      </div>
      <div className="settings-row">
        <label>
          {text("General AI model")}
          <select
            aria-label={text("General AI model")}
            value={settings?.aiProviders.generalAnalysisModel ?? "gemini-2.5-flash"}
            onChange={(event) => onGeneralAnalysisModelChange(event.target.value)}
          >
            <option value="gemini-2.5-flash-lite">Gemini 2.5 Flash-Lite</option>
            <option value="gemini-2.5-flash">Gemini 2.5 Flash</option>
            <option value="gemini-3.1-flash-lite">Gemini 3.1 Flash-Lite</option>
            <option value="gemini-3.5-flash">Gemini 3.5 Flash</option>
          </select>
        </label>
      </div>
      <div className="settings-row">
        <label>
          {text("General AI timeout")}
          <select
            aria-label={text("General AI timeout")}
            value={settings?.aiProviders.generalAnalysisTimeoutSeconds ?? 90}
            onChange={(event) => onGeneralAnalysisTimeoutChange(Number(event.target.value))}
          >
            <option value={45}>45 {text("seconds")}</option>
            <option value={90}>90 {text("seconds")}</option>
            <option value={180}>3 {text("minutes")}</option>
            <option value={300}>5 {text("minutes")}</option>
            <option value={600}>10 {text("minutes")}</option>
          </select>
        </label>
      </div>
    </section>
  );
}
