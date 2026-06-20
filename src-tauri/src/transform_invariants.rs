//! Reusable invariant assertions for data-transform property tests (ADR 0049).
//!
//! Brawler's roadmap munches a lot of structured data from many sources into one
//! unified set; that correctness risk lives in **data transforms** — dedup,
//! normalization, entity matching, merge — which fail on the long tail and at
//! volume, not on the happy path. Those transforms are tested by the **algebraic
//! properties** they must satisfy, not only by examples. These helpers express
//! each property once, so every transform — and every future data epic — plugs
//! into the same harness instead of re-deriving it.
//!
//! Test-only (`#[cfg(test)]`); never compiled into the shipped binary.

use std::fmt::Debug;

/// **Idempotence** for a `&str -> String` transform: `f(f(x)) == f(x)`. Applying
/// a normalization/slug transform to its own output is a no-op — the canonical
/// property for normalization.
pub fn assert_idempotent_str<F: Fn(&str) -> String>(f: F, input: &str) {
    let once = f(input);
    let twice = f(&once);
    assert_eq!(
        once, twice,
        "not idempotent: f(f(x)) != f(x) on input {input:?}"
    );
}

/// **Idempotence** for an owned `Vec<T> -> Vec<T>` transform: `f(f(x)) == f(x)`.
/// The dedup/normalize-collection counterpart of [`assert_idempotent_str`].
pub fn assert_idempotent_vec<T, F>(f: F, input: Vec<T>)
where
    T: Clone + PartialEq + Debug,
    F: Fn(Vec<T>) -> Vec<T>,
{
    let once = f(input);
    let twice = f(once.clone());
    assert_eq!(once, twice, "not idempotent: f(f(x)) != f(x)");
}

/// **Determinism** for a `&str -> String` transform: repeated calls agree, so
/// there is no wall-clock/random/iteration-order leakage. On an id-producing
/// transform this is *stable identity* (the same input always yields the same id).
pub fn assert_deterministic_str<F: Fn(&str) -> String>(f: F, input: &str) {
    assert_eq!(
        f(input),
        f(input),
        "not deterministic across calls on input {input:?}"
    );
}

/// **Order-independence (commutativity)** for a `Vec<T> -> U` transform:
/// `f(xs) == f(perm(xs))`. The core property for "the same items, arriving from
/// sources in any order, reconcile to the same canonical set." Checks reversal
/// and (for 3+ items) a rotation.
pub fn assert_order_independent<T, U, F>(f: F, items: Vec<T>)
where
    T: Clone,
    U: PartialEq + Debug,
    F: Fn(Vec<T>) -> U,
{
    let original = f(items.clone());

    let mut reversed = items.clone();
    reversed.reverse();
    assert_eq!(
        original,
        f(reversed),
        "transform depends on input order (reverse)"
    );

    if items.len() > 2 {
        let mut rotated = items;
        rotated.rotate_left(1);
        assert_eq!(
            original,
            f(rotated),
            "transform depends on input order (rotate)"
        );
    }
}

/// **Charset boundedness**: every character of the output is in the allowed set.
/// Slug/id transforms must collapse arbitrary input into a known alphabet rather
/// than leaking raw characters downstream.
pub fn assert_charset(output: &str, allowed: impl Fn(char) -> bool, label: &str) {
    for character in output.chars() {
        assert!(
            allowed(character),
            "{label}: disallowed char {character:?} leaked into output {output:?}"
        );
    }
}
