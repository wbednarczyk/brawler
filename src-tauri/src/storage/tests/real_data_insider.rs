//! Real-data recall/precision harness for the MAR art. 19 insider cover-note
//! parser (v0.57 T4, ADR 0083 D6; docs/testing.md real-data-first rule). Mirrors
//! [`super::real_data_ownership`]: **inert in CI**, it measures accuracy over the
//! hand-labeled ground-truth set (`private/realdata/insider_ground_truth.json`,
//! 22 filings / 30 transactions) by resolving each labeled filing from a
//! throwaway copy of the maintainer's real DB, reading `feed_items.title` +
//! `body_text`, running the pure parser, and reporting person/role/direction
//! recall+precision against the labels.
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/worktest.sqlite3
//! BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
//!   cargo test -p brawler --lib real_data_insider -- --ignored --nocapture
//! ```
//!
//! `BRAWLER_INSIDER_GROUND_TRUTH` overrides the ground-truth path.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;

use super::*;
use crate::fundamentals::insider::{
    parse_insider_notification, InsiderNotificationParse, InsiderUnit,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroundTruth {
    notifications: Vec<GtFiling>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GtFiling {
    ticker: String,
    feed_item_id: String,
    transactions: Vec<GtTransaction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GtTransaction {
    person_raw: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    volume: Option<String>,
    #[serde(default)]
    price: Option<String>,
    #[serde(default)]
    tx_date: Option<String>,
}

/// Fold Polish diacritics and lowercase.
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ą' | 'Ą' => 'a',
            'ć' | 'Ć' => 'c',
            'ę' | 'Ę' => 'e',
            'ł' | 'Ł' => 'l',
            'ń' | 'Ń' => 'n',
            'ó' | 'Ó' => 'o',
            'ś' | 'Ś' => 's',
            'ź' | 'Ź' | 'ż' | 'Ż' => 'z',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

/// A lenient person key: the set of 4-char folded token prefixes. Two names match
/// when the smaller key is a subset of the larger (tolerates honorifics, residual
/// declension, and entity-suffix variants without crediting distinct people).
fn person_key(name: &str) -> BTreeSet<String> {
    fold(name)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(|t| t.chars().take(4).collect::<String>())
        .collect()
}

fn names_match(a: &BTreeSet<String>, b: &BTreeSet<String>) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a.is_subset(b) || b.is_subset(a)
}

fn pct(n: usize, d: usize) -> String {
    if d == 0 {
        "n/a".to_owned()
    } else {
        format!("{:.1}%", 100.0 * n as f64 / d as f64)
    }
}

#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB (a throwaway copy) + the insider ground truth"]
fn real_data_insider_recall_precision() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_insider_recall_precision: set BRAWLER_REAL_DB to a throwaway copy"
        );
        return;
    };
    let gt_path = std::env::var("BRAWLER_INSIDER_GROUND_TRUTH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("private/realdata/insider_ground_truth.json")
        });
    let Ok(raw) = std::fs::read_to_string(&gt_path) else {
        eprintln!(
            "SKIP real_data_insider_recall_precision: no ground truth at {}",
            gt_path.display()
        );
        return;
    };
    let gt: GroundTruth = serde_json::from_str(&raw).expect("insider ground-truth JSON parses");

    let connection = open_database(&db_path).expect("open real db copy");

    let (mut resolved, mut labeled_tx, mut parsed_units) = (0usize, 0usize, 0usize);
    let (mut person_matched, mut role_correct, mut role_labeled, mut role_emitted) =
        (0usize, 0usize, 0usize, 0usize);
    let (mut dir_correct, mut dir_emitted, mut dir_labeled) = (0usize, 0usize, 0usize);
    // Inline-figures accuracy over the labeled non-null subset (T4 point 4).
    let (mut vol_correct, mut vol_emitted, mut vol_labeled) = (0usize, 0usize, 0usize);
    let (mut price_correct, mut price_emitted, mut price_labeled) = (0usize, 0usize, 0usize);
    let (mut date_correct, mut date_emitted, mut date_labeled) = (0usize, 0usize, 0usize);

    eprintln!(
        "== real-data insider recall/precision: {} filing(s) ==",
        gt.notifications.len()
    );

    for filing in &gt.notifications {
        let row: Option<(String, Option<String>)> = connection
            .query_row(
                "SELECT title, body_text FROM feed_items WHERE id = ?1",
                [&filing.feed_item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .expect("query feed_items");
        let Some((title, Some(body))) = row else {
            eprintln!(
                "SKIP {} ({}): no feed_items row / body",
                filing.ticker, filing.feed_item_id
            );
            continue;
        };
        resolved += 1;

        let units = match parse_insider_notification(&title, &body) {
            InsiderNotificationParse::Units(units) => units
                .into_iter()
                .filter_map(|u| match u {
                    InsiderUnit::Clean(p) => Some(p),
                    InsiderUnit::Ambiguous { .. } => None,
                })
                .collect::<Vec<_>>(),
            InsiderNotificationParse::NotFound => Vec::new(),
        };
        labeled_tx += filing.transactions.len();
        parsed_units += units.len();

        // Greedy 1:1 person matching between labeled tx and parsed units.
        let mut consumed = vec![false; units.len()];
        let mut misses: Vec<&str> = Vec::new();
        for tx in &filing.transactions {
            let key = person_key(&tx.person_raw);
            let hit = units.iter().enumerate().find(|(i, u)| {
                !consumed[*i] && names_match(&key, &person_key(&u.person_normalized))
            });
            if let Some(role) = &tx.role {
                let _ = role;
                role_labeled += 1;
            }
            if tx.direction.is_some() {
                dir_labeled += 1;
            }
            if tx.volume.is_some() {
                vol_labeled += 1;
            }
            if tx.price.is_some() {
                price_labeled += 1;
            }
            if tx.tx_date.is_some() {
                date_labeled += 1;
            }
            match hit {
                Some((i, u)) => {
                    consumed[i] = true;
                    person_matched += 1;
                    // Role: correct when labeled role present and equals parsed role.
                    if let (Some(lbl), Some(got)) = (&tx.role, u.role) {
                        if lbl == got.as_str() {
                            role_correct += 1;
                        }
                    }
                    // Direction precision: parsed direction present and equals label.
                    if let Some(got) = u.direction {
                        if tx.direction.as_deref() == Some(got.as_str()) {
                            dir_correct += 1;
                        }
                    }
                    // Figures recall: parsed field present and equals the label.
                    if let (Some(lbl), Some(got)) = (&tx.volume, &u.volume) {
                        if lbl == got {
                            vol_correct += 1;
                        }
                    }
                    if let (Some(lbl), Some(got)) = (&tx.price, &u.price) {
                        if lbl == got {
                            price_correct += 1;
                        }
                    }
                    if let (Some(lbl), Some(got)) = (&tx.tx_date, &u.tx_date) {
                        if lbl == got {
                            date_correct += 1;
                        }
                    }
                }
                None => misses.push(tx.person_raw.as_str()),
            }
        }
        role_emitted += units.iter().filter(|u| u.role.is_some()).count();
        dir_emitted += units.iter().filter(|u| u.direction.is_some()).count();
        vol_emitted += units.iter().filter(|u| u.volume.is_some()).count();
        price_emitted += units.iter().filter(|u| u.price.is_some()).count();
        date_emitted += units.iter().filter(|u| u.tx_date.is_some()).count();

        eprintln!(
            "{:<5} {:<48} labeled={} parsed={} matched={}{}",
            filing.ticker,
            filing.feed_item_id,
            filing.transactions.len(),
            units.len(),
            filing
                .transactions
                .iter()
                .filter(|tx| {
                    let key = person_key(&tx.person_raw);
                    units
                        .iter()
                        .any(|u| names_match(&key, &person_key(&u.person_normalized)))
                })
                .count(),
            if misses.is_empty() {
                String::new()
            } else {
                format!("  MISS={misses:?}")
            },
        );
    }

    eprintln!("-- overall over {resolved} resolved filing(s) --");
    eprintln!("person   recall={} ({person_matched}/{labeled_tx})   precision={} ({person_matched}/{parsed_units})",
        pct(person_matched, labeled_tx), pct(person_matched, parsed_units));
    eprintln!("role     recall={} ({role_correct}/{role_labeled})   precision(vs emitted)={} ({role_correct}/{role_emitted})",
        pct(role_correct, role_labeled), pct(role_correct, role_emitted));
    eprintln!("direction precision={} ({dir_correct}/{dir_emitted})   recall={} ({dir_correct}/{dir_labeled})",
        pct(dir_correct, dir_emitted), pct(dir_correct, dir_labeled));
    eprintln!("-- inline figures (labeled non-null subset) --");
    eprintln!("volume   precision={} ({vol_correct}/{vol_emitted})   recall={} ({vol_correct}/{vol_labeled})",
        pct(vol_correct, vol_emitted), pct(vol_correct, vol_labeled));
    eprintln!("price    precision={} ({price_correct}/{price_emitted})   recall={} ({price_correct}/{price_labeled})",
        pct(price_correct, price_emitted), pct(price_correct, price_labeled));
    eprintln!("tx_date  precision={} ({date_correct}/{date_emitted})   recall={} ({date_correct}/{date_labeled})",
        pct(date_correct, date_emitted), pct(date_correct, date_labeled));

    assert!(
        resolved > 0,
        "at least one labeled filing must resolve (check BRAWLER_REAL_DB)"
    );
}

/// Optional attachment-tier (T4b) cross-check: if notification documents are
/// present locally, parse them and measure figure recall against the ground truth.
///
/// The live fetch over the maintainer's filings runs at T9 closure; until then this
/// consumes any pre-fetched files under `BRAWLER_INSIDER_ATTACH_DIR`, named
/// `<feedItemId>.pdf` / `<feedItemId>.xhtml` (or, when unset, the `report_documents/`
/// subtree of `BRAWLER_REAL_DATA_DIR`). Inert (SKIP) when no files are present — it
/// never fabricates evidence.
#[test]
#[ignore = "real-data validation; needs BRAWLER_INSIDER_ATTACH_DIR (or a real data dir) of fetched notification files"]
fn real_data_insider_attachment_figures() {
    use crate::fundamentals::insider::attachment::{parse_notification_text, AttachmentParse};
    use crate::report_diff::extraction::{extract_report, ExtractionState, SourceFormat};

    let gt_path = std::env::var("BRAWLER_INSIDER_GROUND_TRUTH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("private/realdata/insider_ground_truth.json")
        });
    let Ok(raw) = std::fs::read_to_string(&gt_path) else {
        eprintln!("SKIP real_data_insider_attachment_figures: no ground truth");
        return;
    };
    let gt: GroundTruth = serde_json::from_str(&raw).expect("ground-truth JSON parses");

    let attach_dir = std::env::var("BRAWLER_INSIDER_ATTACH_DIR")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::var("BRAWLER_REAL_DATA_DIR")
                .map(|d| PathBuf::from(d).join("report_documents"))
        });
    let Ok(attach_dir) = attach_dir else {
        eprintln!(
            "SKIP real_data_insider_attachment_figures: set BRAWLER_INSIDER_ATTACH_DIR (or \
             BRAWLER_REAL_DATA_DIR) to a directory of fetched notification files"
        );
        return;
    };

    let (mut found, mut parsed) = (0usize, 0usize);
    let (mut vol_hit, mut vol_lbl, mut date_hit, mut date_lbl) = (0usize, 0usize, 0usize, 0usize);

    for filing in &gt.notifications {
        // Accept either a per-feed-item file or a document keyed by feed id.
        let candidate = ["pdf", "xhtml", "html"]
            .iter()
            .map(|ext| attach_dir.join(format!("{}.{ext}", filing.feed_item_id)))
            .find(|p| p.exists());
        let Some(path) = candidate else { continue };
        found += 1;
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let format = SourceFormat::resolve(None, &path.to_string_lossy());
        let extracted = extract_report(&bytes, format);
        if extracted.state != ExtractionState::Extracted {
            eprintln!(
                "{} {}: {:?} (parked)",
                filing.ticker, filing.feed_item_id, extracted.state
            );
            continue;
        }
        let text: String = extracted
            .sections
            .iter()
            .flat_map(|s| [s.heading.as_str(), s.body.as_str()])
            .collect::<Vec<_>>()
            .join("\n");
        let AttachmentParse::Units(units) = parse_notification_text(&text) else {
            eprintln!("{} {}: NotFound", filing.ticker, filing.feed_item_id);
            continue;
        };
        parsed += 1;
        for tx in &filing.transactions {
            if tx.volume.is_some() {
                vol_lbl += 1;
                if units.iter().any(|u| u.volume == tx.volume) {
                    vol_hit += 1;
                }
            }
            if tx.tx_date.is_some() {
                date_lbl += 1;
                if units.iter().any(|u| u.tx_date == tx.tx_date) {
                    date_hit += 1;
                }
            }
        }
        eprintln!(
            "{} {}: {} unit(s)",
            filing.ticker,
            filing.feed_item_id,
            units.len()
        );
    }

    if found == 0 {
        eprintln!("SKIP real_data_insider_attachment_figures: no attachment files under the dir");
        return;
    }
    eprintln!("-- attachment tier over {found} present file(s), {parsed} parsed --");
    eprintln!(
        "volume  recall={} ({vol_hit}/{vol_lbl})",
        pct(vol_hit, vol_lbl)
    );
    eprintln!(
        "tx_date recall={} ({date_hit}/{date_lbl})",
        pct(date_hit, date_lbl)
    );
}
