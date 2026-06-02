import type { CredentialStatus, UserSettings } from "../../api/types";

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
  return (
    <dl className="transcript-runtime-strip" aria-label="Transcript runtime settings">
      <div>
        <dt>Provider</dt>
        <dd>{formatAiProvider(settings?.aiProviders.youtubeTranscriptionProvider)}</dd>
      </div>
      <div>
        <dt>Credentials</dt>
        <dd>{formatCredentialConfigured(geminiCredentialStatus)}</dd>
      </div>
      <div>
        <dt>Storage</dt>
        <dd>{formatCredentialStorage(geminiCredentialStatus?.storage)}</dd>
      </div>
      <div>
        <dt>Timeout</dt>
        <dd>{settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}s</dd>
      </div>
    </dl>
  );
}
