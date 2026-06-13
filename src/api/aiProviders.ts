import { callCommand } from "./tauri";
import type { AiProviderCatalogEntry } from "./types";

/** The selectable AI analysis providers and their curated models (ADR 0028). */
export function listAiProviderCatalog() {
  return callCommand<AiProviderCatalogEntry[]>("list_ai_provider_catalog");
}
