//! Classifies a Bankier company feed item as **witnessing** a periodic-report
//! filing — the shared predicate behind `report_delay` and Dziś `nonArrival`
//! (ADR 0083 §8, issue #427; `storage::red_flags::witness`). Distinct from
//! [`super::bankier_company::is_periodic_report_item`] (attachment-download
//! gate only): a false positive here suppresses a genuine delay flag.
//! Decision order/markers: [`is_periodic_report_filing`] / `periodic_filing_markers.json`.

use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;

const MARKERS_JSON: &str = include_str!("periodic_filing_markers.json");

#[derive(Deserialize)]
struct RawMarkers {
    #[serde(rename = "bodyPeriodicForm")]
    body_periodic_form: Vec<String>,
    #[serde(rename = "bodyCurrentReport")]
    body_current_report: Vec<String>,
    #[serde(rename = "titleFormCode")]
    title_form_code: String,
    #[serde(rename = "titlePositive")]
    title_positive: Vec<String>,
    #[serde(rename = "titleNegative")]
    title_negative: Vec<String>,
}

struct Markers {
    body_periodic_form: Vec<String>,
    body_current_report: Vec<String>,
    title_form_code: Regex,
    title_positive: Vec<String>,
    title_negative: Vec<String>,
}

fn markers() -> &'static Markers {
    static MARKERS: OnceLock<Markers> = OnceLock::new();
    MARKERS.get_or_init(|| {
        let raw: RawMarkers =
            serde_json::from_str(MARKERS_JSON).expect("static periodic_filing_markers.json");
        Markers {
            body_periodic_form: lower_all(raw.body_periodic_form),
            body_current_report: lower_all(raw.body_current_report),
            title_form_code: Regex::new(&format!("(?i){}", raw.title_form_code))
                .expect("static titleFormCode pattern"),
            title_positive: lower_all(raw.title_positive),
            title_negative: lower_all(raw.title_negative),
        }
    })
}

fn lower_all(items: Vec<String>) -> Vec<String> {
    items.into_iter().map(|item| item.to_lowercase()).collect()
}

/// A table-of-contents marker matches only at an item boundary: `1. RAPORT
/// BIEŻĄCY` must not be found inside `11. RAPORT BIEŻĄCY` (the digit before
/// the match belongs to another item number). Both sides are lowercased.
fn contains_item_marker(body_lower: &str, marker_lower: &str) -> bool {
    body_lower.match_indices(marker_lower).any(|(idx, _)| {
        !body_lower[..idx]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_digit())
    })
}

/// Whether `title`/`body` witness a periodic-report filing (the classifier
/// behind `report_delay` / Dziś non-arrival). Decision order (markers file —
/// `"decisionOrder"` — is the single source of the lists/pattern):
///
/// 1. `body` present and contains a `bodyCurrentReport` marker → `false`.
/// 2. `body` present and contains ≥2 `bodyPeriodicForm` markers → `true`.
/// 3. `title` contains a `titleNegative` marker → `false`.
/// 4. `title` matches `titleFormCode` → `true`.
/// 5. `title` contains a `titlePositive` marker → `true`.
/// 6. else `false` (incl. empty/whitespace title with no usable body).
pub(crate) fn is_periodic_report_filing(title: &str, body: Option<&str>) -> bool {
    let m = markers();
    let body = body
        .map(|text| text.to_lowercase())
        .filter(|text| !text.trim().is_empty());
    if let Some(body) = &body {
        if m.body_current_report
            .iter()
            .any(|marker| contains_item_marker(body, marker))
        {
            return false;
        }
        let hits = m
            .body_periodic_form
            .iter()
            .filter(|marker| contains_item_marker(body, marker))
            .count();
        if hits >= 2 {
            return true;
        }
    }
    let title_lower = title.to_lowercase();
    if m.title_negative
        .iter()
        .any(|marker| title_lower.contains(marker.as_str()))
    {
        return false;
    }
    if m.title_form_code.is_match(title) {
        return true;
    }
    m.title_positive
        .iter()
        .any(|marker| title_lower.contains(marker.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const FORM_BODY: &str =
        "Spis treści:1. STRONA TYTUŁOWA2. WYBRANE DANE FINANSOWE3. ZAWARTOŚĆ RAPORTU4. PODPISY";
    const CURRENT_BODY: &str =
        "Spis treści:1. RAPORT BIEŻĄCY2. MESSAGE (ENGLISH VERSION)3. PODPISY";

    #[test]
    fn form_body_wins_regardless_of_title() {
        assert!(is_periodic_report_filing(
            "Wyniki finansowe PSr /2026",
            Some(FORM_BODY)
        ));
    }

    #[test]
    fn current_report_body_beats_a_periodic_looking_title() {
        // The body decides even when the title itself reads like a periodic
        // correction (real Bankier data: such corrections are filed as
        // current reports, not periodic forms).
        assert!(!is_periodic_report_filing(
            "Korekta raportu okresowego za I kwartał 2026 roku",
            Some(CURRENT_BODY),
        ));
    }

    /// A periodic form whose body merely MENTIONS "raport bieżący" in prose
    /// (not the numbered TOC heading "1. RAPORT BIEŻĄCY") must stay periodic:
    /// `bodyCurrentReport` matches only the actual heading, so a KOREKTA
    /// RAPORTU section that references "raport bieżący nr ..." in passing
    /// does not flip the verdict.
    #[test]
    fn body_mention_of_current_report_outside_the_toc_heading_stays_periodic() {
        let body = "Spis treści:1. STRONA TYTUŁOWA2. WYBRANE DANE FINANSOWE3. KOREKTA RAPORTU4. \
                     ZAWARTOŚĆ RAPORTUTreść korekty: dokument pozostaje raport bieżący nr 15/2026 \
                     w części opisowej.";
        assert!(is_periodic_report_filing(
            "Korekta raportu rocznego za 2025 rok",
            Some(body),
        ));
    }

    /// A corrected report republished as a periodic FORM still witnesses via
    /// its body — step 2 (`bodyPeriodicForm` ≥2) runs before the title
    /// negative check, so a "Korekta ..." title doesn't override a genuine
    /// periodic-form body.
    #[test]
    fn corrected_periodic_form_still_witnesses_via_body() {
        let body = "Spis treści:1. STRONA TYTUŁOWA2. WYBRANE DANE FINANSOWE3. PODPISY";
        assert!(is_periodic_report_filing(
            "Korekta raportu rocznego",
            Some(body)
        ));
    }

    #[test]
    fn title_only_form_codes() {
        assert!(is_periodic_report_filing("Wyniki finansowe PSr", None));
        assert!(is_periodic_report_filing(
            "Wyniki finansowe QSr 1/2026",
            None
        ));
        assert!(is_periodic_report_filing(
            "P /2026: formularz raportu półrocznego",
            None
        ));
        assert!(is_periodic_report_filing(
            "Wyniki finansowe SRR /2025",
            None
        ));
    }

    /// The form-code regex requires filing syntax — a bare form-code letter
    /// followed by an unrelated word (not `wyniki finansowe` and not a
    /// `/20YY` year) must not match, or two-letter abbreviations like `Q&A`
    /// and `R&D` would false-positive.
    #[test]
    fn title_only_bare_letters_are_not_form_codes() {
        assert!(!is_periodic_report_filing("Q&A z zarządem", None));
        assert!(!is_periodic_report_filing("R & D: nowa strategia", None));
        assert!(!is_periodic_report_filing("PS w sprawie dywidendy", None));
    }

    /// Decision order: title negatives are checked BEFORE the title form
    /// code, so a form-code-looking title that also reads as preliminary
    /// results is rejected rather than short-circuited to `true`.
    #[test]
    fn title_negative_wins_over_a_matching_form_code() {
        assert!(!is_periodic_report_filing(
            "QSr 1/2026 - wstępne wyniki",
            None
        ));
    }

    #[test]
    fn title_only_preliminary_results_is_not_periodic() {
        assert!(!is_periodic_report_filing(
            "Wstępne wyniki finansowe i operacyjne za I półrocze 2026 roku",
            None,
        ));
    }

    #[test]
    fn title_only_correction_with_no_body_is_not_periodic() {
        assert!(!is_periodic_report_filing(
            "Korekta raportu okresowego za I kwartał",
            None,
        ));
    }

    /// Corrections never count as the witnessing arrival (owner decision):
    /// the generic `korekta` marker catches a correction title even when the
    /// word doesn't sit next to "raportu" and the title otherwise reads as a
    /// periodic report (`skonsolidowany raport`).
    #[test]
    fn title_only_correction_anywhere_in_title_is_not_periodic() {
        assert!(!is_periodic_report_filing(
            "Skonsolidowany raport roczny za 2025 — korekta",
            None,
        ));
    }

    #[test]
    fn title_only_publication_date_change_is_not_periodic() {
        assert!(!is_periodic_report_filing(
            "Zmiana terminu publikacji skonsolidowanego raportu za I półrocze 2026 roku",
            None,
        ));
    }

    #[test]
    fn title_only_generic_periodic_wording_is_periodic() {
        assert!(is_periodic_report_filing(
            "Raport okresowy za I półrocze 2026",
            None
        ));
    }

    #[test]
    fn empty_or_whitespace_title_is_not_periodic() {
        assert!(!is_periodic_report_filing("", None));
        assert!(!is_periodic_report_filing("   ", None));
    }

    /// Every labeled row (`periodic_filing_labels.tsv`: public feed item id,
    /// label, title, and a reduced body head carrying just the form/
    /// current-report markers) must classify exactly as labeled — the
    /// real-data precision floor, pinned as a committed fixture so it runs
    /// in default CI (the full real-database probe, `witness_precision_probe`
    /// below, is `#[ignore]`d and needs the owner's data).
    #[test]
    fn matches_every_labeled_fixture_row() {
        let fixture = include_str!("periodic_filing_labels.tsv");
        let mut mismatches = Vec::new();
        for (line_no, line) in fixture.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let mut cols = line.splitn(4, '\t');
            let (Some(_id), Some(label), Some(title), Some(body_head)) =
                (cols.next(), cols.next(), cols.next(), cols.next())
            else {
                panic!("malformed fixture line {}: {line}", line_no + 1);
            };
            let expected = label == "periodic";
            let actual = is_periodic_report_filing(title, Some(body_head));
            if actual != expected {
                mismatches.push(format!(
                    "line {}: {title} — expected {label}, got {actual}",
                    line_no + 1
                ));
            }
        }
        assert!(mismatches.is_empty(), "{mismatches:#?}");
    }

    /// Real-data precision probe (deliberately `#[ignore]`d — needs the
    /// owner's data): reads the label set
    /// `feed_item_id\tlabel\ttitle\tbody_head` — the committed fixture by default,
    /// or the same format from `BRAWLER_WITNESS_LABELS` —
    /// fetches the FULL title/body for each `feed_item_id` from
    /// `BRAWLER_WITNESS_PROBE_DB` (a throwaway copy), and prints a confusion
    /// matrix + every mismatch. Target: precision = recall = 1.0 — a
    /// mismatch means the markers need adjusting, never the labels.
    ///
    /// ```text
    /// BRAWLER_WITNESS_PROBE_DB=$SCRATCH/owner-db-copy.sqlite3 \
    ///   cargo test -p brawler --lib periodic_filing::tests::witness_precision_probe \
    ///   -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "witness classifier precision probe; needs BRAWLER_WITNESS_PROBE_DB (a throwaway db copy)"]
    fn witness_precision_probe() {
        let Ok(db_path) = std::env::var("BRAWLER_WITNESS_PROBE_DB") else {
            eprintln!(
                "SKIP witness_precision_probe: set BRAWLER_WITNESS_PROBE_DB (a throwaway db copy)"
            );
            return;
        };
        let labels = match std::env::var("BRAWLER_WITNESS_LABELS") {
            Ok(path) => std::fs::read_to_string(&path).expect("read labels tsv"),
            Err(_) => include_str!("periodic_filing_labels.tsv").to_owned(),
        };
        let file_name = std::path::Path::new(&db_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        assert!(
            file_name != "brawler.sqlite3" && !db_path.starts_with("/mnt/d/"),
            "refusing to run: {db_path} is the master snapshot or the live application database"
        );

        let connection = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open probe db read-only");

        let (mut tp, mut tn, mut fp, mut fn_) = (0u32, 0u32, 0u32, 0u32);
        let mut mismatches = Vec::new();
        for line in labels.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let mut cols = line.splitn(4, '\t');
            let (Some(feed_item_id), Some(label), ..) =
                (cols.next(), cols.next(), cols.next(), cols.next())
            else {
                continue;
            };
            let expected = label == "periodic";
            let (title, body): (String, Option<String>) = connection
                .query_row(
                    "SELECT title, body_text FROM feed_items WHERE id = ?1",
                    [feed_item_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap_or_else(|error| panic!("row {feed_item_id} missing: {error}"));
            let actual = is_periodic_report_filing(&title, body.as_deref());
            match (expected, actual) {
                (true, true) => tp += 1,
                (false, false) => tn += 1,
                (false, true) => {
                    fp += 1;
                    mismatches.push(format!("FP {feed_item_id} {title}"));
                }
                (true, false) => {
                    fn_ += 1;
                    mismatches.push(format!("FN {feed_item_id} {title}"));
                }
            }
        }

        let precision = if tp + fp == 0 {
            1.0
        } else {
            tp as f64 / (tp + fp) as f64
        };
        let recall = if tp + fn_ == 0 {
            1.0
        } else {
            tp as f64 / (tp + fn_) as f64
        };
        eprintln!("== witness classifier precision probe ==");
        eprintln!("db={db_path}");
        eprintln!("tp={tp} tn={tn} fp={fp} fn={fn_} precision={precision:.3} recall={recall:.3}");
        for mismatch in &mismatches {
            eprintln!("{mismatch}");
        }
        assert!(
            mismatches.is_empty(),
            "{} mismatches, see stderr above",
            mismatches.len()
        );
    }

    /// Golden verdicts over the labeled fixture (ADR 0049 golden snapshot):
    /// any marker change that flips a labeled title shows up as a snapshot diff.
    #[test]
    fn toc_markers_match_only_at_item_boundaries() {
        // A later "11." item must not be read as the "1." current-report heading.
        let periodic_with_late_item = "Spis treści:1. STRONA TYTUŁOWA2. WYBRANE DANE FINANSOWE3. ZAWARTOŚĆ RAPORTU11. RAPORT BIEŻĄCY";
        assert!(is_periodic_report_filing(
            "cokolwiek",
            Some(periodic_with_late_item)
        ));
        // "11. STRONA TYTUŁOWA" is not the "1. STRONA TYTUŁOWA" heading.
        assert!(!is_periodic_report_filing(
            "cokolwiek",
            Some("Spis treści:11. STRONA TYTUŁOWA2. PODPISY")
        ));
        assert!(contains_item_marker(
            "x 1. raport bieżący",
            "1. raport bieżący"
        ));
        assert!(!contains_item_marker(
            "x 11. raport bieżący",
            "1. raport bieżący"
        ));
    }

    #[test]
    fn golden_periodic_filing_verdicts_over_the_labeled_fixture() {
        let rows: Vec<(String, String, bool)> = include_str!("periodic_filing_labels.tsv")
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .map(|line| {
                let cols: Vec<&str> = line.split('\t').collect();
                let (title, body) = (cols[2], cols.get(3).copied().unwrap_or(""));
                let body_head: String = body.chars().take(36).collect();
                (
                    title.to_owned(),
                    body_head,
                    is_periodic_report_filing(title, Some(body)),
                )
            })
            .collect();
        insta::assert_debug_snapshot!("golden_periodic_filing_verdicts", rows);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// A body that carries the current-report form marker is never a
        /// periodic filing, whatever the title says — the invariant that
        /// keeps "Wstępne wyniki finansowe…" from witnessing an arrival.
        #[test]
        fn a_current_report_body_never_witnesses(
            title in ".{0,80}",
            prefix in ".{0,40}",
            suffix in ".{0,40}",
        ) {
            let body = format!("{prefix}1. RAPORT BIEŻĄCY{suffix}");
            prop_assert!(!is_periodic_report_filing(&title, Some(&body)));
        }

        /// Total, deterministic and case-insensitive over the Polish/ASCII
        /// charset the markers are written in (the frontend mock compares the
        /// same lowercased lists).
        #[test]
        fn verdict_is_total_deterministic_and_case_insensitive(
            title in "[A-Za-zĄĆĘŁŃÓŚŹŻąćęłńóśźż0-9 /:._-]{0,80}",
            body in proptest::option::of("[A-Za-zĄĆĘŁŃÓŚŹŻąćęłńóśźż0-9 /:._-]{0,120}"),
        ) {
            let verdict = is_periodic_report_filing(&title, body.as_deref());
            prop_assert_eq!(is_periodic_report_filing(&title, body.as_deref()), verdict);
            let upper_body = body.as_ref().map(|text| text.to_uppercase());
            prop_assert_eq!(
                is_periodic_report_filing(&title.to_uppercase(), upper_body.as_deref()),
                verdict
            );
        }
    }
}
