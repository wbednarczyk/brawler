import type { CredentialStatus, UserSettings } from "../../api/types";

type AiSettingsProps = {
  geminiCredentialStatus: CredentialStatus | null;
  settings: UserSettings | null;
  onYoutubeTranscriptionModelChange: (model: string) => void;
  onYoutubeTranscriptionTimeoutChange: (timeoutSeconds: number) => void;
  formatAiProvider: (value: string | null | undefined) => string;
  formatGeminiModel: (value: string | null | undefined) => string;
  formatCredentialConfigured: (status: CredentialStatus | null) => string;
  formatCredentialStorage: (value: string | null | undefined) => string;
  formatCredentialKind: (value: string | null | undefined) => string;
};

export function AiSettings({
  geminiCredentialStatus,
  settings,
  onYoutubeTranscriptionModelChange,
  onYoutubeTranscriptionTimeoutChange,
  formatAiProvider,
  formatGeminiModel,
  formatCredentialConfigured,
  formatCredentialStorage,
  formatCredentialKind,
}: AiSettingsProps) {
  return (
    <section className="settings-group" aria-labelledby="settings-ai-title">
      <h2 id="settings-ai-title">AI</h2>
      <dl className="settings-grid">
        <div>
          <dt>YouTube transcription</dt>
          <dd>{formatAiProvider(settings?.aiProviders.youtubeTranscriptionProvider)}</dd>
        </div>
        <div>
          <dt>YouTube transcription provider ID</dt>
          <dd>{settings?.aiProviders.youtubeTranscriptionProvider ?? "provider_gemini"}</dd>
        </div>
        <div>
          <dt>YouTube transcription model</dt>
          <dd>{formatGeminiModel(settings?.aiProviders.youtubeTranscriptionModel)}</dd>
        </div>
        <div>
          <dt>YouTube transcription timeout</dt>
          <dd>{settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}s</dd>
        </div>
        <div>
          <dt>YouTube transcription credentials</dt>
          <dd>{formatCredentialConfigured(geminiCredentialStatus)}</dd>
        </div>
        <div>
          <dt>Credential storage</dt>
          <dd>{formatCredentialStorage(geminiCredentialStatus?.storage)}</dd>
        </div>
        <div>
          <dt>Credential kind</dt>
          <dd>{formatCredentialKind(geminiCredentialStatus?.secretKind)}</dd>
        </div>
        <div>
          <dt>YouTube transcription disclosure</dt>
          <dd>Starting a transcript job sends the YouTube URL and video content to Gemini.</dd>
        </div>
        <div>
          <dt>YouTube transcription scope</dt>
          <dd>Gemini is used only for YouTube transcription.</dd>
        </div>
        <div>
          <dt>General AI provider</dt>
          <dd>{formatAiProvider(settings?.aiProviders.generalAnalysisProvider)}</dd>
        </div>
        <div>
          <dt>AI analysis mode</dt>
          <dd>{settings?.aiAnalysisMode ?? "source_grounded"}</dd>
        </div>
      </dl>
      <div className="settings-row">
        <label>
          Gemini transcription model
          <select
            aria-label="Gemini transcription model"
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
          <span>Default</span>
          <strong>Cheapest supported</strong>
        </div>
      </div>
      <div className="settings-row">
        <label>
          Gemini transcription timeout
          <select
            aria-label="Gemini transcription timeout"
            value={settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}
            onChange={(event) => onYoutubeTranscriptionTimeoutChange(Number(event.target.value))}
          >
            <option value={45}>45 seconds</option>
            <option value={90}>90 seconds</option>
            <option value={180}>3 minutes</option>
            <option value={300}>5 minutes</option>
            <option value={600}>10 minutes</option>
          </select>
        </label>
        <div className="settings-summary">
          <span>Default</span>
          <strong>5 minutes</strong>
        </div>
      </div>
    </section>
  );
}
