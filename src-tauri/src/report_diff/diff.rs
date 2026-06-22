//! Deterministic section-level diff between two financial statements (v0.47.0,
//! ADR 0052). Pure, total, and reproducible: the same two section lists always
//! produce the same diff, and a statement diffed against itself yields an
//! all-`Unchanged`, zero-delta result (the hard self-diff = empty invariant).

use serde::Serialize;
use similar::{ChangeTag, TextDiff};

use super::extraction::Section;

/// Per-section diff status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum SectionDiffStatus {
    Unchanged,
    Changed,
    OnlyOlder,
    OnlyNewer,
}

/// One aligned (or unmatched) section in the diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SectionDiff {
    pub status: SectionDiffStatus,
    pub heading: String,
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub older_ordinal: Option<i64>,
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub newer_ordinal: Option<i64>,
    pub added_lines: i64,
    pub removed_lines: i64,
}

/// The full section diff read model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SectionsDiff {
    /// Older sections that matched a newer section by heading + positional consumption.
    pub aligned_count: i64,
    pub sections: Vec<SectionDiff>,
}

/// Diff two ordered section lists. Alignment is by heading with **positional
/// consumption**: each newer section is matched at most once, so duplicate
/// headings never cross-match — the property that makes self-diff = empty hold
/// (ADR 0052). `<preamble>` sections participate like any other.
pub fn diff_sections(older: &[Section], newer: &[Section]) -> SectionsDiff {
    let mut used = vec![false; newer.len()];
    let mut sections: Vec<SectionDiff> = Vec::new();
    let mut aligned = 0i64;

    for o in older {
        match newer
            .iter()
            .enumerate()
            .find(|(i, n)| !used[*i] && n.heading == o.heading)
        {
            Some((idx, n)) => {
                used[idx] = true;
                aligned += 1;
                if o.body.trim() == n.body.trim() {
                    sections.push(SectionDiff {
                        status: SectionDiffStatus::Unchanged,
                        heading: o.heading.clone(),
                        older_ordinal: Some(o.ordinal),
                        newer_ordinal: Some(n.ordinal),
                        added_lines: 0,
                        removed_lines: 0,
                    });
                } else {
                    let (added, removed) = line_delta(o.body.trim(), n.body.trim());
                    sections.push(SectionDiff {
                        status: SectionDiffStatus::Changed,
                        heading: o.heading.clone(),
                        older_ordinal: Some(o.ordinal),
                        newer_ordinal: Some(n.ordinal),
                        added_lines: added,
                        removed_lines: removed,
                    });
                }
            }
            None => sections.push(SectionDiff {
                status: SectionDiffStatus::OnlyOlder,
                heading: o.heading.clone(),
                older_ordinal: Some(o.ordinal),
                newer_ordinal: None,
                added_lines: 0,
                removed_lines: 0,
            }),
        }
    }

    for (i, n) in newer.iter().enumerate() {
        if !used[i] {
            sections.push(SectionDiff {
                status: SectionDiffStatus::OnlyNewer,
                heading: n.heading.clone(),
                older_ordinal: None,
                newer_ordinal: Some(n.ordinal),
                added_lines: 0,
                removed_lines: 0,
            });
        }
    }

    SectionsDiff {
        aligned_count: aligned,
        sections,
    }
}

fn line_delta(older: &str, newer: &str) -> (i64, i64) {
    let diff = TextDiff::from_lines(older, newer);
    let mut added = 0i64;
    let mut removed = 0i64;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sections(items: &[(&str, &str)]) -> Vec<Section> {
        items
            .iter()
            .enumerate()
            .map(|(i, (h, b))| Section {
                ordinal: i as i64,
                heading: (*h).to_owned(),
                body: (*b).to_owned(),
            })
            .collect()
    }

    #[test]
    fn self_diff_is_empty() {
        // A statement diffed against itself: every section Unchanged, zero delta.
        let s = sections(&[
            ("bilans", "aktywa 100\npasywa 100"),
            ("bilans", "duplicate heading different body"),
            ("rachunek zysków", "przychody 50"),
        ]);
        let d = diff_sections(&s, &s);
        assert_eq!(d.aligned_count, 3);
        assert!(d
            .sections
            .iter()
            .all(|x| x.status == SectionDiffStatus::Unchanged));
        assert!(d
            .sections
            .iter()
            .all(|x| x.added_lines == 0 && x.removed_lines == 0));
    }

    #[test]
    fn detects_changed_added_removed() {
        let older = sections(&[("bilans", "aktywa 100"), ("noty", "nota A")]);
        let newer = sections(&[("bilans", "aktywa 120"), ("ryzyka", "nowe ryzyko")]);
        let d = diff_sections(&older, &newer);
        let changed = &d.sections[0];
        assert_eq!(changed.status, SectionDiffStatus::Changed);
        assert!(changed.added_lines > 0 && changed.removed_lines > 0);
        assert!(d
            .sections
            .iter()
            .any(|x| x.status == SectionDiffStatus::OnlyOlder && x.heading == "noty"));
        assert!(d
            .sections
            .iter()
            .any(|x| x.status == SectionDiffStatus::OnlyNewer && x.heading == "ryzyka"));
    }

    #[test]
    fn deterministic_and_idempotent() {
        let older = sections(&[("a", "1"), ("b", "2"), ("a", "3")]);
        let newer = sections(&[("a", "1x"), ("a", "3"), ("b", "2")]);
        let first = diff_sections(&older, &newer);
        let second = diff_sections(&older, &newer);
        assert_eq!(first, second);
    }
}
