import { callCommand } from "./tauri";
import type { EmbeddingModelStatus } from "./generated/EmbeddingModelStatus";
import type { FindSimilarContentResult } from "./generated/FindSimilarContentResult";

// Interpretative AI layer — embedding-model strategy (ADR 0035, v0.45.0).
// Local-only: the model runs on-device, no content leaves the machine.
//
// Output DTOs are GENERATED from src-tauri/src/commands/interpretation.rs via
// ts-rs (ADR 0048); the weightsState/strategy unions are inline overrides on
// those fields. EmbeddingWeightsState and SimilarityStrategy are frontend-only
// union aliases (no backing Rust enum — the Rust fields are `String`).

export type EmbeddingWeightsState = "unsupported" | "absent" | "downloading" | "ready" | "error";
export type SimilarityStrategy = "static" | "embedding";

export type { EmbeddedTypeCount } from "./generated/EmbeddedTypeCount";
export type { EmbeddingModelStatus } from "./generated/EmbeddingModelStatus";
export type { ScoredContent } from "./generated/ScoredContent";
export type { FindSimilarContentResult } from "./generated/FindSimilarContentResult";

export function getEmbeddingModelStatus() {
  return callCommand<EmbeddingModelStatus>("get_embedding_model_status");
}

export function downloadEmbeddingModel() {
  return callCommand<EmbeddingModelStatus>("download_embedding_model");
}

export function setSimilarityStrategy(strategy: SimilarityStrategy) {
  return callCommand<EmbeddingModelStatus>("set_similarity_strategy", { input: { strategy } });
}

export function rebuildEmbeddingIndex() {
  return callCommand<EmbeddingModelStatus>("rebuild_embedding_index");
}

export function findSimilarContent(input: { contentType: string; contentId: string; k?: number }) {
  return callCommand<FindSimilarContentResult>("find_similar_content", { input });
}
