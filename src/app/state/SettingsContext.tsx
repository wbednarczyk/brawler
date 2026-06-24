import { createContext, useContext, type ReactNode } from "react";

import type { UserSettings } from "../../api/generated/UserSettings";

/**
 * Settings-domain feature-scoped state (Architecture v2 / ADR 0050).
 *
 * The first of the per-domain contexts that decompose the `AppStateRoot`
 * mega-view-model: instead of prop-drilling individual settings flags through
 * `AppShell` and every screen, the loaded {@link UserSettings} is provided once
 * here and read where needed via selector hooks (e.g. {@link useDeveloperMode}).
 * `null` means settings have not loaded yet — selectors return safe defaults
 * rather than throwing, matching the prior `Boolean(settings?.developerMode)`
 * call sites. Add a selector here as each further settings read migrates off
 * prop-drilling.
 */
const SettingsContext = createContext<UserSettings | null>(null);

export function SettingsProvider({
  value,
  children,
}: {
  value: UserSettings | null;
  children: ReactNode;
}) {
  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

/** Whether developer mode is on (defaults to `false` before settings load). */
export function useDeveloperMode(): boolean {
  return Boolean(useContext(SettingsContext)?.developerMode);
}

/** Whether the opt-in ESPI AI signal-classification fallback is enabled (ADR 0036). */
export function useAiFallbackEnabled(): boolean {
  return Boolean(useContext(SettingsContext)?.espiAiFallbackEnabled);
}

/** Whether a general-analysis AI provider is configured (gates AI analysis actions). */
export function useAiAnalysisProviderConfigured(): boolean {
  return Boolean(useContext(SettingsContext)?.aiProviders.generalAnalysisProvider);
}

/** Company IDs pinned to the sidebar spine (ADR 0054); empty before settings load. */
export function usePinnedCompanyIds(): string[] {
  return useContext(SettingsContext)?.pinnedCompanyIds ?? [];
}
