import type { UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { FieldRow, Hint, SelectField } from "../../ui";

type TranscriptSettingsProps = {
  settings: UserSettings | null;
  onYoutubeTranscriptionModelChange: (model: string) => void;
  onYoutubeTranscriptionTimeoutChange: (timeoutSeconds: number) => void;
};

// The transcript provider is the only remaining model-backed capability (ADR
// 0084 decision 3): transcription is data acquisition, not interpretation —
// intelligence now arrives through the MCP port.
export function TranscriptSettings({
  settings,
  onYoutubeTranscriptionModelChange,
  onYoutubeTranscriptionTimeoutChange,
}: TranscriptSettingsProps) {
  const { text } = useLocale();

  return (
    <section className="settings-group" aria-labelledby="settings-transcripts-title">
      <h2 id="settings-transcripts-title">{text("Transcripts")}</h2>
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
      </FieldRow>
      <Hint>
        {text("Speech-to-text for saved video sources. Set the Gemini API key in Credentials.")}
      </Hint>
    </section>
  );
}
