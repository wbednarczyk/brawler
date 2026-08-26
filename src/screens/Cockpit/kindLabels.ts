// Human labels for the cockpit panel kinds (extracted from CockpitScreen under
// the file-size ratchet, ADR 0103).
import type { GlobalKind, LinkedKind, PinnedKind } from "./CockpitScreen";

export function globalKindLabel(kind: GlobalKind, text: (s: string) => string): string {
  switch (kind) {
    case "watchlists":
      return text("Watchlists");
    case "research":
      return text("Research");
    case "notebook":
      return text("Notebook");
    case "events":
      return text("Events");
    case "reportSeason":
      return text("Report Season");
    case "decisionJournalGlobal":
      return text("Journal (all companies)");
  }
}

export function pinnedKindLabel(kind: PinnedKind, text: (s: string) => string): string {
  switch (kind) {
    case "basicInfo":
      return text("Basic info");
    case "fundamentals":
      return text("Fundamentals");
    case "coverage":
      return text("Coverage");
    case "reportDiff":
      return text("Report comparison");
    case "claims":
      return text("Claims");
    case "quality":
      return text("Quality");
    case "documents":
      return text("Report documents");
    case "companyFeed":
      return text("Feed");
    case "companyNotebook":
      return text("Notebook");
    case "decisionJournal":
      return text("Decision journal");
    case "shortPositions":
      return text("Short selling (KNF)");
    case "redFlags":
      return text("Warning signals");
    case "analystRecommendations":
      return text("Analyst recommendations");
  }
}

export function linkedTitle(kind: LinkedKind, text: (s: string) => string): string {
  switch (kind) {
    case "feed":
      return text("Feed");
    case "inspector":
      return text("Inspector");
    case "claims-sel":
      return text("Claims");
    case "diff-sel":
      return text("Report comparison");
  }
}
