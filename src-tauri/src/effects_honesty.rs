//! Effects honesty (epic #40 S5; [ADR 0091](../../docs/adr/0091-failure-path-and-real-state-testing.md)).
//!
//! The defect class this module exists to kill: **a run that reports success,
//! produced nothing, and cannot say why.** The owner kept catching it by hand
//! after v0.50 — a toast said "done", the numbers did not move, and nothing in
//! the app named the cause.
//!
//! The rule is a shape obligation, not a style preference: every run summary
//! must be able to answer "you had inputs and produced nothing — why?" with a
//! CONCRETE, typed reason it already carries (a counter that names the skip, a
//! typed reason code, an error list), never prose invented at render time
//! (ADR 0087 dec. 4 / ADR 0084 dec. 6). [`EffectVerdict::Unexplained`] is the
//! dishonest state; the per-shape invariant tests beside each summary assert it
//! is unreachable for every zero-effect state the producing code can build.
//!
//! Summaries implement [`ExplainsEffect`] **in their own module**, beside the
//! type, so the reason mapping cannot drift away from the counters it reads.

/// What a run summary says about its own effect.
///
/// The distinction that matters is between the last two: "nothing happened, and
/// here is the named reason" is honest; "nothing happened" alone is the bug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectVerdict {
    /// The run produced something — there is nothing to explain.
    Produced,
    /// The run had no inputs at all: nothing was asked of it, so producing
    /// nothing is the complete and honest answer.
    NoInputs,
    /// The run had inputs, produced nothing, and NAMES why. The reason is a
    /// stable machine token (a counter name or a typed reason code), which the
    /// UI translates — never English prose composed here.
    NothingProduced { reason: &'static str },
    /// The run had inputs, produced nothing, and names no reason. **The
    /// dishonest state.** Any summary that can reach it is a shape defect.
    Unexplained,
}

impl EffectVerdict {
    /// The named reason, when there is one.
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            EffectVerdict::NothingProduced { reason } => Some(reason),
            _ => None,
        }
    }

    /// Whether the summary is dishonest about its own emptiness.
    pub fn is_unexplained(&self) -> bool {
        matches!(self, EffectVerdict::Unexplained)
    }
}

/// A run summary that can account for its own (lack of) effect.
///
/// Implement beside the summary type. The implementation reads only fields the
/// summary already carries: this trait is a *reading* of the shape, so a shape
/// that cannot explain itself fails the invariant test instead of being papered
/// over with a fabricated reason.
pub trait ExplainsEffect {
    fn effect_verdict(&self) -> EffectVerdict;
}

/// Helper for the common shape: pick the first named reason whose counter is
/// non-zero, in the order the implementation considers most informative.
/// Returns `None` when every candidate counter is zero.
pub(crate) fn first_named<const N: usize>(
    candidates: [(&'static str, bool); N],
) -> Option<&'static str> {
    candidates
        .into_iter()
        .find_map(|(reason, present)| present.then_some(reason))
}

/// The single verdict assembly every implementation routes through, so the
/// four-way classification can never drift between shapes.
///
/// - `produced` — did the run establish anything at all?
/// - `had_inputs` — was there anything to act on?
/// - `reason` — the named explanation, when the summary carries one.
pub(crate) fn verdict(
    produced: bool,
    had_inputs: bool,
    reason: Option<&'static str>,
) -> EffectVerdict {
    if produced {
        return EffectVerdict::Produced;
    }
    if !had_inputs {
        return EffectVerdict::NoInputs;
    }
    match reason {
        Some(reason) => EffectVerdict::NothingProduced { reason },
        None => EffectVerdict::Unexplained,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_classifies_the_four_states() {
        assert_eq!(verdict(true, true, None), EffectVerdict::Produced);
        assert_eq!(verdict(false, false, None), EffectVerdict::NoInputs);
        assert_eq!(
            verdict(false, true, Some("no_period")),
            EffectVerdict::NothingProduced {
                reason: "no_period"
            }
        );
        // The whole point of the type: inputs in, nothing out, nothing said.
        assert!(verdict(false, true, None).is_unexplained());
    }

    #[test]
    fn first_named_picks_the_first_present_reason_and_nothing_when_all_absent() {
        assert_eq!(
            first_named([("a", false), ("b", true), ("c", true)]),
            Some("b")
        );
        assert_eq!(first_named([("a", false), ("b", false)]), None);
    }
}
