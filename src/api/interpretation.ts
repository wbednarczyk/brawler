import { callCommand } from "./tauri";

// Interpretative AI layer — embedding-model strategy (ADR 0035, v0.45.0).
// Local-only: the model runs on-device, no content leaves the machine.

export type EmbeddingWeightsState = "unsupported" | "absent" | "downloading" | "ready" | "error";

export type EmbeddedTypeCount = {
  contentType: string;
  count: number;
};

export type EmbeddingModelStatus = {
  modelId: string;
  dim: number;
  weightsState: EmbeddingWeightsState;
  downloadProgress: number | null;
  downloadError: string | null;
  activeSimilarityStrategy: "static" | "embedding";
  embeddedCounts: EmbeddedTypeCount[];
  indexModelId: string | null;
  featureCompiled: boolean;
};

export type SimilarityStrategy = "static" | "embedding";

export type ScoredContent = {
  contentType: string;
  contentId: string;
  score: number;
};

export type FindSimilarContentResult = {
  strategyId: string;
  items: ScoredContent[];
};

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
