import type { FormEvent } from "react";
import type { CredentialStatus, FeedPruneResult, Theme, UserSettings } from "../../api/types";

export type SettingsScreenProps = {
  theme: Theme;
  settings: UserSettings | null;
  settingsError: string | null;
  feedPruneRetentionDays: number;
  feedPruneResult: FeedPruneResult | null;
  geminiCredentialStatus: CredentialStatus | null;
  geminiCredentialError: string | null;
  geminiCredentialInFlight: boolean;
  geminiApiKeyDraft: string;
  onThemeChange: (theme: Theme) => void;
  onPollIntervalChange: (pollIntervalSeconds: number) => void;
  onYoutubeTranscriptionModelChange: (model: string) => void;
  onYoutubeTranscriptionTimeoutChange: (timeoutSeconds: number) => void;
  onGeminiApiKeyDraftChange: (apiKey: string) => void;
  onSaveGeminiApiKey: (event: FormEvent<HTMLFormElement>) => void;
  onClearGeminiApiKey: () => void;
  onOpenGeminiApiKeyPage: () => void;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  formatPollInterval: (seconds: number) => string;
  formatAiProvider: (value: string | null | undefined) => string;
  formatGeminiModel: (value: string | null | undefined) => string;
  formatCredentialConfigured: (status: CredentialStatus | null) => string;
  formatCredentialStorage: (value: string | null | undefined) => string;
  formatCredentialKind: (value: string | null | undefined) => string;
};
