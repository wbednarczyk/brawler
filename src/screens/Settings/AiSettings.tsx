import type { CredentialStatus, UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";

type AiSettingsProps = {
  geminiCredentialStatus: CredentialStatus | null;
  settings: UserSettings | null;
  onYoutubeTranscriptionModelChange: (model: string) => void;
  onYoutubeTranscriptionTimeoutChange: (timeoutSeconds: number) => void;
  onGeneralAnalysisProviderChange: (provider: string) => void;
  onGeneralAnalysisModelChange: (model: string) => void;
  onGeneralAnalysisTimeoutChange: (timeoutSeconds: number) => void;
  formatAiProvider: (value: string | null | undefined) => string;
  formatGeminiModel: (value: string | null | undefined) => string;
  formatCredentialConfigured: (status: CredentialStatus | null) => string;
  formatCredentialKind: (value: string | null | undefined) => string;
};

export function AiSettings({
  geminiCredentialStatus,
  settings,
  onYoutubeTranscriptionModelChange,
  onYoutubeTranscriptionTimeoutChange,
  onGeneralAnalysisProviderChange,
  onGeneralAnalysisModelChange,
  onGeneralAnalysisTimeoutChange,
  formatAiProvider,
  formatGeminiModel,
  formatCredentialConfigured,
  formatCredentialKind,
}: AiSettingsProps) {
  const { t, text } = useLocale();

  return (
    <section className="settings-group" aria-labelledby="settings-ai-title">
      <h2 id="settings-ai-title">{t("settings.ai.title")}</h2>
      <dl className="settings-grid">
        <div>
          <dt>{text("YouTube transcription")}</dt>
          <dd>{text(formatAiProvider(settings?.aiProviders.youtubeTranscriptionProvider))}</dd>
        </div>
        <div>
          <dt>{text("YouTube transcription provider ID")}</dt>
          <dd>{settings?.aiProviders.youtubeTranscriptionProvider ?? "provider_gemini"}</dd>
        </div>
        <div>
          <dt>{text("YouTube transcription model")}</dt>
          <dd>{text(formatGeminiModel(settings?.aiProviders.youtubeTranscriptionModel))}</dd>
        </div>
        <div>
          <dt>{text("YouTube transcription timeout")}</dt>
          <dd>{settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}s</dd>
        </div>
        <div>
          <dt>{text("YouTube transcription credentials")}</dt>
          <dd>{text(formatCredentialConfigured(geminiCredentialStatus))}</dd>
        </div>
        <div>
          <dt>{text("Credential kind")}</dt>
          <dd>{text(formatCredentialKind(geminiCredentialStatus?.secretKind))}</dd>
        </div>
        <div>
          <dt>{text("YouTube transcription disclosure")}</dt>
          <dd>{text("Starting a transcript job sends the YouTube URL and video content to Gemini.")}</dd>
        </div>
        <div>
          <dt>{text("YouTube transcription scope")}</dt>
          <dd>{text("Gemini is used only for YouTube transcription.")}</dd>
        </div>
        <div>
          <dt>{text("General AI provider")}</dt>
          <dd>{text(formatAiProvider(settings?.aiProviders.generalAnalysisProvider))}</dd>
        </div>
        <div>
          <dt>{text("General AI model")}</dt>
          <dd>{text(formatGeminiModel(settings?.aiProviders.generalAnalysisModel))}</dd>
        </div>
        <div>
          <dt>{text("General AI timeout")}</dt>
          <dd>{settings?.aiProviders.generalAnalysisTimeoutSeconds ?? 90}s</dd>
        </div>
        <div>
          <dt>{text("General AI disclosure")}</dt>
          <dd>
            {text(
              "Starting feed analysis sends the selected source text and metadata to the configured AI provider.",
            )}
          </dd>
        </div>
        <div>
          <dt>{text("AI analysis mode")}</dt>
          <dd>{settings?.aiAnalysisMode ?? "source_grounded"}</dd>
        </div>
      </dl>
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
        <div className="settings-summary">
          <span>{text("Default")}</span>
          <strong>{text("Cheapest supported")}</strong>
        </div>
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
        <div className="settings-summary">
          <span>{text("Default")}</span>
          <strong>5 {text("minutes")}</strong>
        </div>
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
        <div className="settings-summary">
          <span>{text("Scope")}</span>
          <strong>{text("Source-grounded feed analysis")}</strong>
        </div>
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
        <div className="settings-summary">
          <span>{text("Default")}</span>
          <strong>Gemini 2.5 Flash</strong>
        </div>
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
        <div className="settings-summary">
          <span>{text("Default")}</span>
          <strong>90 {text("seconds")}</strong>
        </div>
      </div>
    </section>
  );
}
