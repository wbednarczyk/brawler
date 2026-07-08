import type { CredentialStatus, UserSettings } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { InfoGrid } from "../../ui";

type TranscriptRuntimeStripProps = {
  geminiCredentialStatus: CredentialStatus | null;
  settings: UserSettings | null;
  formatAiProvider: (value: string | null | undefined) => string;
  formatCredentialConfigured: (status: CredentialStatus | null) => string;
};

export function TranscriptRuntimeStrip({
  geminiCredentialStatus,
  settings,
  formatAiProvider,
  formatCredentialConfigured,
}: TranscriptRuntimeStripProps) {
  const { text } = useLocale();

  return (
    <InfoGrid
      ariaLabel={text("Transcript engine settings")}
      className="transcript-runtime-strip"
      items={[
        { label: text("Provider"), value: formatAiProvider(settings?.aiProviders.youtubeTranscriptionProvider) },
        { label: text("Credentials"), value: formatCredentialConfigured(geminiCredentialStatus) },
        { label: text("Timeout"), value: `${settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}s` },
      ]}
    />
  );
}
