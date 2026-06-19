//! Pure-Rust on-device embedder via `candle` (ADR 0035 amendment, v0.45.0).
//!
//! Loads a BERT-family encoder (default `intfloat/multilingual-e5-small`) from
//! local weights and produces sentence embeddings by mean-pooling token states
//! under the attention mask, then L2-normalizing — the standard e5 recipe. This
//! whole module is behind the `embedding-model` cargo feature so the default
//! build never compiles `candle`; it is validated by the periodic model eval
//! built with the feature on (ADR 0035, engineering-workflow.md).
//!
//! The exact encoder is confirmed by the eval (ADR 0035 section 7); because
//! consumers bind to the [`Embedder`] boundary, swapping the model never touches
//! them.
#![cfg(feature = "embedding-model")]

use std::path::Path;

use async_trait::async_trait;
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

use super::embedder::Embedder;
use super::InterpretationError;

/// e5 models are trained with an instruction prefix; for symmetric similarity we
/// use the same prefix on every text.
const E5_PREFIX: &str = "query: ";

pub struct CandleEmbedder {
    model_id: String,
    dim: usize,
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl CandleEmbedder {
    /// Load the model from a directory containing `config.json`,
    /// `tokenizer.json`, and `model.safetensors`.
    pub fn load(dir: &Path, model_id: &str, dim: usize) -> Result<Self, String> {
        let device = Device::Cpu;

        let config_json = std::fs::read_to_string(dir.join("config.json"))
            .map_err(|error| format!("reading config.json: {error}"))?;
        let config: Config = serde_json::from_str(&config_json)
            .map_err(|error| format!("parsing config.json: {error}"))?;

        let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|error| format!("loading tokenizer.json: {error}"))?;
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            ..Default::default()
        }));
        // Truncate to the model's positional limit (e5-small = 512). Without this,
        // a long document yields position ids beyond the position-embedding table
        // and the forward pass fails with an index-select out-of-range error.
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: config.max_position_embeddings,
                ..Default::default()
            }))
            .map_err(|error| format!("configuring truncation: {error}"))?;

        let weights_path = dir.join("model.safetensors");
        let var_builder = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)
                .map_err(|error| format!("mapping model.safetensors: {error}"))?
        };
        let model = BertModel::load(var_builder, &config)
            .map_err(|error| format!("loading BERT weights: {error}"))?;

        Ok(Self {
            model_id: model_id.to_string(),
            dim,
            model,
            tokenizer,
            device,
        })
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let prefixed: Vec<String> = texts.iter().map(|t| format!("{E5_PREFIX}{t}")).collect();
        let encodings = self
            .tokenizer
            .encode_batch(prefixed, true)
            .map_err(|error| format!("tokenizing: {error}"))?;

        let batch = encodings.len();
        let seq_len = encodings.first().map(|e| e.get_ids().len()).unwrap_or(0);

        let mut ids = Vec::with_capacity(batch * seq_len);
        let mut mask = Vec::with_capacity(batch * seq_len);
        for encoding in &encodings {
            ids.extend(encoding.get_ids().iter().copied());
            mask.extend(encoding.get_attention_mask().iter().copied());
        }

        let input_ids = Tensor::from_vec(ids, (batch, seq_len), &self.device)
            .map_err(|error| format!("input_ids tensor: {error}"))?;
        let attention_mask = Tensor::from_vec(mask, (batch, seq_len), &self.device)
            .map_err(|error| format!("attention_mask tensor: {error}"))?;
        let token_type_ids = input_ids
            .zeros_like()
            .map_err(|error| format!("token_type_ids tensor: {error}"))?;

        let token_embeddings = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))
            .map_err(|error| format!("model forward: {error}"))?;

        // Mean-pool over tokens, weighting by the attention mask, then normalize.
        let mask_f = attention_mask
            .to_dtype(DType::F32)
            .and_then(|m| m.unsqueeze(2))
            .map_err(|error| format!("mask cast: {error}"))?;
        let pooled = (|| {
            let masked = token_embeddings.broadcast_mul(&mask_f)?;
            let summed = masked.sum(1)?;
            let counts = mask_f.sum(1)?;
            let mean = summed.broadcast_div(&counts)?;
            let norm = mean.sqr()?.sum_keepdim(1)?.sqrt()?;
            mean.broadcast_div(&norm)
        })()
        .map_err(|error| format!("pooling: {error}"))?;

        pooled
            .to_vec2::<f32>()
            .map_err(|error| format!("reading vectors: {error}"))
    }
}

#[async_trait]
impl Embedder for CandleEmbedder {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn dim(&self) -> usize {
        self.dim
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InterpretationError> {
        self.embed_batch(texts)
            .map_err(InterpretationError::Backend)
    }
}

#[cfg(test)]
mod tests {
    //! Runtime tests against the REAL model (ADR 0035 section 7). They only run
    //! when `BRAWLER_EMBEDDING_MODEL_DIR` points at a directory holding the
    //! e5-small weights (`config.json` + `tokenizer.json` + `model.safetensors`);
    //! otherwise they skip, so default/offline `cargo test --features
    //! embedding-model` stays green. Run locally with the downloaded model, e.g.:
    //! `BRAWLER_EMBEDDING_MODEL_DIR=<app-data>/models/intfloat__multilingual-e5-small`.

    use std::path::PathBuf;
    use std::sync::Arc;

    use super::*;
    use crate::interpretation::{
        evaluate_similarity, Embedder, EmbeddingSimilarity, LexicalSimilarity, SimilarityProvider,
        SimilarityRankingSample, TextItem,
    };

    fn model_dir_from_env() -> Option<PathBuf> {
        std::env::var_os("BRAWLER_EMBEDDING_MODEL_DIR")
            .map(PathBuf::from)
            .filter(|dir| dir.join("model.safetensors").exists())
    }

    fn load() -> Option<CandleEmbedder> {
        let dir = model_dir_from_env()?;
        Some(CandleEmbedder::load(&dir, "intfloat/multilingual-e5-small", 384).expect("load model"))
    }

    #[test]
    fn embeds_long_input_and_batch_without_panicking() {
        let Some(embedder) = load() else {
            eprintln!("skipping: BRAWLER_EMBEDDING_MODEL_DIR not set or weights absent");
            return;
        };

        // A >512-token document must be truncated, not overflow the position table
        // (the v0.45.0 regression). Then a mixed batch must keep its shape.
        let long = "raport okresowy ".repeat(4000);
        let batch = embedder
            .embed_batch(&[
                "zarząd rekomenduje wypłatę dywidendy".to_string(),
                "raport okresowy za trzeci kwartał".to_string(),
                long,
            ])
            .expect("batch embed");
        assert_eq!(batch.len(), 3);
        assert!(batch.iter().all(|vector| vector.len() == 384));
    }

    /// A small Polish ESPI/EBI corpus — each item phrased as a media/alternate
    /// source might phrase it (the "other coverage" in a story cluster).
    fn corpus() -> Vec<(&'static str, &'static str)> {
        vec![
            ("div", "Spółka chce podzielić się zyskiem z inwestorami i wypłacić im część wypracowanych środków"),
            ("warn", "Spółka ostrzega, że zarobi mniej niż wcześniej zakładała — korekta szacunków w dół"),
            ("contract", "Podpisano duży kontrakt handlowy; nowy odbiorca zamówił produkty o istotnej wartości"),
            ("buyback", "Spółka odkupiła część swoich akcji z rynku w ramach programu buy-back"),
            ("ceo", "Rada nadzorcza wybrała nową osobę na stanowisko szefa firmy"),
            ("agm", "Spółka zaprasza właścicieli akcji na doroczne zgromadzenie i podaje porządek obrad"),
            ("insider", "Zawiadomienie w trybie MAR o nabyciu akcji przez członka zarządu"),
            ("delay", "Spółka przesuwa datę przekazania sprawozdania za pierwsze trzy miesiące roku"),
            ("capital", "Sąd rejestrowy zatwierdził zwiększenie kapitału spółki w rejestrze"),
            ("shareholders", "Lista akcjonariuszy z co najmniej 5% liczby głosów na walnym zgromadzeniu"),
            ("merger", "Spółka ujawnia, że rozmawiała o kupnie innego podmiotu i wstrzymała się z podaniem tego do wiadomości"),
            ("audit", "Wybrano firmę audytorską do zbadania rocznego sprawozdania finansowego"),
            ("resolutions", "Uchwały podjęte na zwyczajnym walnym zgromadzeniu akcjonariuszy"),
        ]
    }

    /// (group, query phrased as the official/other source, id of the matching corpus item).
    /// `paraphrase` = low word overlap with its match (the case embeddings must win);
    /// `keyword` = high overlap (the case lexical already handles — a regression guard).
    fn queries() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            (
                "paraphrase",
                "Rekomendacja zarządu w sprawie wypłaty dywidendy za rok obrotowy",
                "div",
            ),
            (
                "paraphrase",
                "Korekta prognozy finansowej — obniżenie oczekiwanych wyników (profit warning)",
                "warn",
            ),
            (
                "paraphrase",
                "Zawarcie znaczącej umowy z kontrahentem na dostawę towarów",
                "contract",
            ),
            (
                "paraphrase",
                "Nabycie akcji własnych w ramach programu skupu",
                "buyback",
            ),
            (
                "paraphrase",
                "Powołanie nowego prezesa zarządu spółki",
                "ceo",
            ),
            (
                "paraphrase",
                "Ogłoszenie o zwołaniu zwyczajnego walnego zgromadzenia akcjonariuszy",
                "agm",
            ),
            (
                "paraphrase",
                "Insider transaction — transakcja osoby zarządzającej na akcjach spółki",
                "insider",
            ),
            (
                "paraphrase",
                "Zmiana terminu publikacji raportu okresowego za pierwszy kwartał",
                "delay",
            ),
            (
                "paraphrase",
                "Rejestracja przez sąd podwyższenia kapitału zakładowego",
                "capital",
            ),
            (
                "paraphrase",
                "Ujawnienie opóźnionej informacji poufnej dotyczącej negocjacji przejęcia",
                "merger",
            ),
            (
                "keyword",
                "Wykaz akcjonariuszy posiadających co najmniej 5% głosów na walnym zgromadzeniu",
                "shareholders",
            ),
            (
                "keyword",
                "Treść uchwał podjętych przez zwyczajne walne zgromadzenie",
                "resolutions",
            ),
        ]
    }

    fn ranking_samples() -> Vec<SimilarityRankingSample> {
        let corpus = corpus();
        queries()
            .into_iter()
            .map(|(_group, query, expected)| SimilarityRankingSample {
                query: query.to_string(),
                candidates: corpus
                    .iter()
                    .map(|(id, text)| TextItem {
                        id: id.to_string(),
                        text: text.to_string(),
                    })
                    .collect(),
                expected_id: expected.to_string(),
            })
            .collect()
    }

    /// Rank of `expected` (1-based) in `provider.most_similar` over the corpus, or
    /// 0 if absent.
    fn rank_of(provider: &dyn SimilarityProvider, sample: &SimilarityRankingSample) -> usize {
        let ranked = tauri::async_runtime::block_on(provider.most_similar(
            &sample.query,
            sample.candidates.clone(),
            sample.candidates.len(),
        ))
        .expect("rank");
        ranked
            .iter()
            .position(|item| item.id == sample.expected_id)
            .map(|position| position + 1)
            .unwrap_or(0)
    }

    #[test]
    fn embedding_eval_runs_and_compares_to_lexical() {
        let Some(embedder) = load() else {
            eprintln!("skipping: BRAWLER_EMBEDDING_MODEL_DIR not set or weights absent");
            return;
        };

        let model = EmbeddingSimilarity::new(Arc::new(embedder) as Arc<dyn Embedder>);
        let lexical = LexicalSimilarity::new();
        let groups = queries();
        let samples = ranking_samples();

        eprintln!("\n=== model vs lexical: rank of the correct match (1 = best) ===");
        eprintln!("{:<11}  {:>5}  {:>7}  query", "group", "model", "lexical");
        for ((group, query, _), sample) in groups.iter().zip(samples.iter()) {
            let model_rank = rank_of(&model, sample);
            let lexical_rank = rank_of(&lexical, sample);
            eprintln!(
                "{:<11}  {:>5}  {:>7}  {}",
                group,
                model_rank,
                lexical_rank,
                &query[..query.len().min(60)]
            );
        }

        let model_report = tauri::async_runtime::block_on(evaluate_similarity(&model, &samples));
        let lexical_report =
            tauri::async_runtime::block_on(evaluate_similarity(&lexical, &samples));
        eprintln!(
            "\nOVERALL  model: top1={:.0}% mrr={:.2}   lexical: top1={:.0}% mrr={:.2}\n",
            model_report.top1_accuracy() * 100.0,
            model_report.mean_reciprocal_rank(),
            lexical_report.top1_accuracy() * 100.0,
            lexical_report.mean_reciprocal_rank(),
        );

        assert_eq!(model_report.total, samples.len());
        // Regression guard (only meaningful with the real model present): the model
        // must not rank correct matches worse than the lexical baseline overall. On
        // this Polish ESPI/EBI paraphrase set the margin is large (≈0.88 vs ≈0.48).
        assert!(model_report.mean_reciprocal_rank() >= lexical_report.mean_reciprocal_rank());
    }
}
