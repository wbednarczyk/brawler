import { callCommand } from "./tauri";

// Types GENERATED from src-tauri/src/storage/morning_briefings.rs via ts-rs (ADR 0048).
import type { MorningBriefing } from "./generated/MorningBriefing";

export type { MorningBriefing } from "./generated/MorningBriefing";
export type { MorningBriefingItem } from "./generated/MorningBriefingItem";

// --- Morning briefing (ADR 0068 decision 4) ---

/**
 * Enqueue an on-demand morning-briefing compose (forces a fresh compose even if
 * today's briefing already exists). Resolves once queued; poll
 * {@link getLatestMorningBriefing} for the composed result.
 */
export function generateMorningBriefing() {
  return callCommand<void>("generate_morning_briefing");
}

/**
 * The most recently composed morning briefing (its structured item list plus the
 * AI narrative when one was produced), or `null` when none has been composed yet.
 */
export function getLatestMorningBriefing() {
  return callCommand<MorningBriefing | null>("get_latest_morning_briefing");
}
