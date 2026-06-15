import type {
  ResearchEvidenceItem,
  ResearchEvidenceType,
  ResearchQuestionStatus,
  ResearchReminder,
  ResearchTrustCategory,
} from "../../api/researchTypes";
import { formatAiProvider, formatCompanyEventType } from "../../shared/formatting/labels";

export function formatQuestionStatus(status: ResearchQuestionStatus) {
  switch (status) {
    case "open":
      return "Status open";
    case "answered":
      return "Answered";
    case "closed":
      return "Closed";
  }
}

export function formatReminderKind(kind: ResearchReminder["reminderKind"]) {
  switch (kind) {
    case "claim_follow_up":
      return "Claim follow-up";
    case "event_review":
      return "Event review";
    case "question_review":
      return "Question review";
    case "manual_research":
      return "Manual research";
    case "digest_review":
      return "Digest review";
    case "signal_review":
      return "Signal review";
  }
}

export function formatReminderStatus(status: ResearchReminder["status"]) {
  switch (status) {
    case "open":
      return "Open";
    case "completed":
      return "Completed";
    case "dismissed":
      return "Dismissed";
  }
}

export function formatEvidenceTitle(item: ResearchEvidenceItem) {
  if (item.evidenceType === "ai_analysis" && item.title === "AI analysis") {
    return "AI analysis";
  }

  return item.title;
}

export function formatEvidenceSummary(item: ResearchEvidenceItem) {
  if (!item.summary) {
    return null;
  }

  if (item.evidenceType === "company_event") {
    return formatCompanyEventType(item.summary);
  }

  return item.summary;
}

export function formatEvidenceAttribution(item: ResearchEvidenceItem) {
  if (!item.attribution) {
    return null;
  }

  if (item.attribution === "provider_gemini") {
    return formatAiProvider(item.attribution);
  }

  return item.attribution;
}

export function formatEvidenceType(evidenceType: ResearchEvidenceType) {
  switch (evidenceType) {
    case "feed_item":
      return "Feed item";
    case "notebook_entry":
      return "Note";
    case "claim":
      return "Claim";
    case "transcript_segment":
      return "Transcript";
    case "company_event":
      return "Event";
    case "ai_analysis":
      return "AI analysis";
    case "research_question":
      return "Research question";
    case "company_signal":
      return "Signal";
    case "reminder":
      return "Reminder";
    case "ai_brief":
      return "AI brief";
    case "digest":
      return "Digest";
  }
}

export function formatTrustCategory(trustCategory: ResearchTrustCategory) {
  switch (trustCategory) {
    case "official_report":
      return "Official report";
    case "company_publication":
      return "Company publication";
    case "public_media":
      return "Market news";
    case "market_calendar":
      return "Calendar";
    case "transcript":
      return "Transcript";
    case "user_note":
      return "Personal note";
    case "ai_generated":
      return "AI analysis";
    case "unknown":
      return "Unknown";
  }
}
