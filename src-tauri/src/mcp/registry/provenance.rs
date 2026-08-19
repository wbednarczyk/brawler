//! Provenance-carrier validation for `act` writes (ADR 0088 dec. 3), invoked
//! from [`super::call`]'s write gate.

use serde_json::Value;

use super::ProvenanceRequirement;
use crate::commands::error::{CommandError, CommandErrorCode};

// ============================================================================
// Provenance validation (scaffold for M3 `act` dispatch, ADR 0088 dec. 3)
// ============================================================================

/// Reject an `act` write whose provenance carrier is absent or empty with a
/// typed `invalid_input` error — never an "empty default" (ADR 0088 dec. 3).
/// Wired into `act` dispatch in M3; unit-tested here now.
pub fn validate_provenance(
    requirement: ProvenanceRequirement,
    input: &Value,
) -> Result<(), CommandError> {
    let satisfied = match requirement {
        // A non-empty `origins` array (notebook create) OR a non-empty
        // `transcriptSegmentIds` selection (the transcript-note origin).
        ProvenanceRequirement::Origins => {
            nonempty_array(input, "origins") || nonempty_array(input, "transcriptSegmentIds")
        }
        ProvenanceRequirement::SourceEvidence => nonempty_str(input, "sourceEvidenceId"),
        // A top-level `citationsJson` (single-verdict shape) OR a non-empty
        // `results` array where every entry carries its own non-empty
        // `citationsJson` (the `set_qualitative_verdicts` batch shape).
        ProvenanceRequirement::CitationsJson => {
            citations_nonempty(input.get("citationsJson")) || results_all_cited(input)
        }
        // #285 T9: `attribution` is the fact's slot dimension (`total` |
        // `owners_of_parent` | `nci`), never an alternate citation carrier —
        // accepting it here let an agent satisfy the gate with prose that
        // minted a phantom uniqueness slot instead of citing a source.
        ProvenanceRequirement::FactCitation => nonempty_str(input, "sourceDocumentRef"),
        ProvenanceRequirement::DocumentAndPerFactCitations => {
            nonempty_str(input, "reportDocumentId") && facts_all_cited(input)
        }
    };
    if satisfied {
        Ok(())
    } else {
        Err(CommandError::new(
            CommandErrorCode::ProvenanceRequired,
            format!(
                "this write must carry provenance: {}",
                requirement.carrier_label()
            ),
        ))
    }
}

/// True when `input.results` is a non-empty array and every entry carries a
/// non-empty serialized `citationsJson` array (the batch-verdict provenance
/// shape used by `set_qualitative_verdicts`).
fn results_all_cited(input: &Value) -> bool {
    input
        .get("results")
        .and_then(Value::as_array)
        .map(|results| {
            !results.is_empty()
                && results
                    .iter()
                    .all(|result| citations_nonempty(result.get("citationsJson")))
        })
        .unwrap_or(false)
}

/// True when `input.facts` is a non-empty array and every entry carries a
/// non-blank `citation` (the `record_financial_facts` per-fact provenance
/// shape, ADR 0093 dec. 6) — a blank citation on ANY entry fails the WHOLE
/// batch before the handler runs (atomic refusal, never a partial write).
fn facts_all_cited(input: &Value) -> bool {
    input
        .get("facts")
        .and_then(Value::as_array)
        .map(|facts| !facts.is_empty() && facts.iter().all(|fact| nonempty_str(fact, "citation")))
        .unwrap_or(false)
}

/// True when `input[key]` is a present, non-empty array.
fn nonempty_array(input: &Value, key: &str) -> bool {
    input
        .get(key)
        .and_then(Value::as_array)
        .map(|array| !array.is_empty())
        .unwrap_or(false)
}

/// True when `input[key]` is a present, non-blank string.
fn nonempty_str(input: &Value, key: &str) -> bool {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

/// True when the value is a string holding a JSON array with at least one entry.
fn citations_nonempty(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|parsed| parsed.as_array().map(|array| !array.is_empty()))
        .unwrap_or(false)
}
