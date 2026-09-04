import type { ActivityFamily } from "../../../api/generated/ActivityFamily";
import type { ActivityItem } from "../../../api/generated/ActivityItem";

type TextFn = (value: string) => string;

/**
 * Exhaustive family -> English source string (ADR 0109 D1, plan § D4 item 4).
 * A `never` fallthrough makes a family the generated union gains but this
 * switch doesn't handle a TYPE error, not a silent English leak — the same
 * shape `activity_identity.rs`'s Rust resolver enforces on the backend. The
 * word "autopilot" never appears here (owner decision 2026-09-03).
 */
function familyCopy(family: ActivityFamily): string {
  switch (family) {
    case "sourceRefresh":
      return "Source refresh";
    case "companyRefresh":
      return "Company refresh";
    case "registryRefresh":
      return "Company registry";
    case "fxPull":
      return "Currency rates";
    case "fundamentalsPull":
      return "Aggregator financial data";
    case "briefing":
      return "Morning briefing";
    case "historyFetch":
      return "Report history retrieval";
    case "reportSweep":
      return "Bulk report reading";
    case "reextraction":
      return "Report re-reading";
    case "reportReading":
      return "Report reading";
    case "ownershipReading":
      return "Ownership reading";
    case "managementReading":
      return "Management holdings reading";
    case "priceHistory":
      return "Price quote history";
    case "kpiIngest":
      return "KPI collection (agent)";
    case "transcript":
      return "Transcript";
    case "corrupted":
      return "Unrecognized task";
    default: {
      const exhaustive: never = family;
      throw new Error(`activityLabels: unhandled ActivityFamily ${String(exhaustive)}`);
    }
  }
}

export function familyLabel(family: ActivityFamily, text: TextFn): string {
  return text(familyCopy(family));
}

type ActivityStatus = ActivityItem["status"];

// Every status renders the ledger's exact outcome (sol diff R2 finding 6),
// not a vague paraphrase ("Completed"/"Did not finish" used to collapse
// "failed" toward "interrupted"). "Stalled"/"Interrupted" use the bare
// outcome word; the other five can't — "Queued"/"Running"/"Succeeded"/
// "Failed"/"Partial" already carry a DIFFERENT PL value elsewhere
// (attentionEventLabels.ts, QualityPanel.tsx, CompanyCoveragePanel.tsx), so
// each uses a distinct phrase that still names the outcome exactly.
function statusCopy(status: ActivityStatus): string {
  switch (status) {
    case "queued":
      return "Queued to run";
    case "running":
      return "Currently running";
    case "stalled":
      return "Stalled";
    case "succeeded":
      return "Finished successfully";
    case "failed":
      return "Finished with an error";
    case "partial":
      return "Partially finished";
    case "interrupted":
      return "Interrupted";
    default: {
      const exhaustive: never = status;
      throw new Error(`activityLabels: unhandled ActivityItem status ${String(exhaustive)}`);
    }
  }
}

export function statusLabel(status: ActivityStatus, text: TextFn): string {
  return text(statusCopy(status));
}
