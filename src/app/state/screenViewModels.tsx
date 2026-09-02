import type { CompaniesScreenProps } from "../../screens/Companies/CompaniesScreen";
import type { EventsScreenProps } from "../../screens/Events/eventTypes";
import type { InboxScreenProps } from "../../screens/Inbox/inboxTypes";
import type { ReportSeasonScreenProps } from "../../screens/ReportSeason/ReportSeasonScreen";
import type { ResearchScreenProps } from "../../screens/Research/ResearchScreen";
import type { SettingsScreenProps } from "../../screens/Settings/settingsTypes";
import type { TranscriptsScreenProps } from "../../screens/Transcripts/transcriptTypes";
import type { WatchlistsScreenProps } from "../../screens/Watchlists/WatchlistsScreen";
import { createScreenContext } from "./createScreenContext";

/**
 * Per-screen view-model contexts (Architecture v2 / ADR 0050). Each screen's
 * bundle is assembled once in `AppStateRoot`, provided here, and read by the
 * screen — replacing prop-drilling of large prop bundles from the root.
 * (The Sources screen has its own module; Settings/Diagnostics also read the
 * settings-domain `SettingsContext` for cross-cutting flags.)
 */
export const [InboxProvider, useInboxViewModel] =
  createScreenContext<InboxScreenProps>("Inbox");
export const [CompaniesProvider, useCompaniesViewModel] =
  createScreenContext<CompaniesScreenProps>("Companies");
export const [WatchlistsProvider, useWatchlistsViewModel] =
  createScreenContext<WatchlistsScreenProps>("Watchlists");
export const [ResearchProvider, useResearchViewModel] =
  createScreenContext<ResearchScreenProps>("Research");
export const [ReportSeasonProvider, useReportSeasonViewModel] =
  createScreenContext<ReportSeasonScreenProps>("ReportSeason");
export const [EventsProvider, useEventsViewModel] =
  createScreenContext<EventsScreenProps>("Events");
export const [TranscriptsProvider, useTranscriptsViewModel] =
  createScreenContext<TranscriptsScreenProps>("Transcripts");
export const [SettingsScreenProvider, useSettingsScreenViewModel] =
  createScreenContext<SettingsScreenProps>("Settings");
