import type { CredentialStatus, UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";

type TranscriptRuntimeStripProps = {
  geminiCredentialStatus: CredentialStatus | null;
  settings: UserSettings | null;
  formatAiProvider: (value: string | null | undefined) => string;
  formatCredentialConfigured: (status: CredentialStatus | null) => string;
  formatCredentialStorage: (value: string | null | undefined) => string;
};

export function TranscriptRuntimeStrip({
  geminiCredentialStatus,
  settings,
  formatAiProvider,
  formatCredentialConfigured,
  formatCredentialStorage,
}: TranscriptRuntimeStripProps) {
  const { text } = useLocale();

  return (
    <dl className="transcript-runtime-strip" aria-label={text("Transcript runtime settings")}>
      <div>
        <dt>{text("Provider")}</dt>
        <dd>{formatAiProvider(settings?.aiProviders.youtubeTranscriptionProvider)}</dd>
      </div>
      <div>
        <dt>{text("Credentials")}</dt>
        <dd>{formatCredentialConfigured(geminiCredentialStatus)}</dd>
      </div>
      <div>
        <dt>{text("Storage")}</dt>
        <dd>{formatCredentialStorage(geminiCredentialStatus?.storage)}</dd>
      </div>
      <div>
        <dt>{text("Timeout")}</dt>
        <dd>{settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}s</dd>
      </div>
    </dl>
  );
}
