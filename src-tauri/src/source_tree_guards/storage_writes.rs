//! Guard (G8, hard gates wave 2): a storage fn that checks out its OWN
//! connection and issues two or more write statements with no transaction
//! wrapping them is not atomic — a crash/failure between the two writes
//! leaves the database in a state no caller ever intended (issue #404's
//! class, applied to Group A: fns that own their `checkout()` rather than
//! receiving a `&Connection`). Group B (`&Connection` writers, e.g.
//! `storage::notebooks::create_notebook_entry`, #461) is NOT scanned — the
//! caller-supplied connection's provenance is not decidable by name; that
//! rule is written, not enforced here (docs/testing.md § Source-tree guards).
//!
//! Fix: wrap the writes in
//! `rusqlite::Transaction::new_unchecked(&conn, rusqlite::TransactionBehavior::Immediate)`
//! (DoD §C — write transactions are IMMEDIATE, never DEFERRED, #404 H6).

use std::path::Path;

use super::scan::{is_test_file, paren_matched_text, strip_comments_and_strings, strip_test_spans};
use super::{extract_fn_body, extract_test_fn_name, is_fn_signature, source_files};

/// Today's own-connection multi-writers with no transaction (computed by this
/// guard on 2026-09-08). Per-fn ratchet: an entry that gains a transaction
/// must be deleted here (loud, never silently re-pinned); a fn newly matching
/// the pattern outside this list fails the gate.
const FROZEN_UNTRANSACTED_WRITERS: &[&str] = &[
    "storage/autopilot.rs:create_run_if_absent",
    "storage/jobs.rs:mark_failed",
    "storage/jobs.rs:reclaim_stale_running",
];

/// Whether `body` (a fn body from `extract_fn_body`) is a G8 offender: checks
/// out its own connection (`.checkout()`) and issues ≥2 write statements
/// (`.execute(`/`execute_batch(` whose parenthesized argument text contains
/// an INSERT/UPDATE/DELETE literal, case-insensitive) with no
/// `new_unchecked(`/`transaction` token in the real code (comments/strings
/// don't count — a `// still transactional` comment or a log string
/// mentioning "transaction" must not exempt a real offender).
fn is_untransacted_multi_writer(body: &str) -> bool {
    if !body.contains(".checkout()") {
        return false;
    }
    if write_statement_count(body) < 2 {
        return false;
    }
    let code_only = strip_comments_and_strings(body);
    !code_only.contains("new_unchecked(") && !code_only.contains("transaction")
}

/// Count `.execute(`/`execute_batch(` calls whose parenthesized argument text
/// contains an INSERT/UPDATE/DELETE SQL literal, case-insensitive. The
/// keyword search runs over the RAW (unstripped) argument text — the SQL
/// literal is inside quotes, so this must not blank strings first.
fn write_statement_count(body: &str) -> usize {
    let mut count = 0;
    for marker in [".execute(", "execute_batch("] {
        let mut search_from = 0;
        while let Some(rel) = body[search_from..].find(marker) {
            let open = search_from + rel + marker.len() - 1;
            match paren_matched_text(body, open) {
                Some(args) => {
                    let upper = args.to_uppercase();
                    if ["INSERT", "UPDATE", "DELETE"]
                        .iter()
                        .any(|keyword| upper.contains(keyword))
                    {
                        count += 1;
                    }
                    search_from = open + args.len() + 2;
                }
                None => search_from = open + 1,
            }
        }
    }
    count
}

/// G8 scan: every fn in `src/storage/**` (test files and test spans
/// excluded) checked against [`is_untransacted_multi_writer`], reported as
/// `"path:fn"` (path relative to `src-tauri/`).
fn untransacted_multi_writers() -> Vec<String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let storage_root = src_dir.join("storage");
    let mut offenders = Vec::new();
    for path in source_files(&storage_root) {
        let rel = path
            .strip_prefix(&src_dir)
            .expect("under src")
            .to_string_lossy()
            .replace('\\', "/");
        if is_test_file(&rel) {
            continue;
        }
        let raw = std::fs::read_to_string(&path).expect("readable source file");
        let content = strip_test_spans(&raw);
        let lines: Vec<&str> = content.lines().collect();
        let mut idx = 0;
        while idx < lines.len() {
            if !is_fn_signature(lines[idx].trim_start()) {
                idx += 1;
                continue;
            }
            let fn_line = idx;
            let body = extract_fn_body(&content, &lines, fn_line);
            if is_untransacted_multi_writer(&body) {
                let name = extract_test_fn_name(lines[fn_line]);
                offenders.push(format!("{rel}:{name}"));
            }
            idx = fn_line + 1;
        }
    }
    offenders.sort();
    offenders.dedup();
    offenders
}

/// Guard (G8): see module docs. Frozen-pin idiom (wave 1,
/// `FROZEN_NO_PROPTEST`): a NEW offender outside the pin fails loudly; a
/// pinned entry that no longer offends must be deleted from the pin (an
/// improvement can never silently re-regress).
#[test]
fn own_connection_multi_writes_are_transactional() {
    let offenders = untransacted_multi_writers();
    let mut violations = Vec::new();
    for offender in &offenders {
        if !FROZEN_UNTRANSACTED_WRITERS.contains(&offender.as_str()) {
            violations.push(format!(
                "{offender}: own-connection writer issues \u{2265}2 write statements with no \
                 transaction (G8) — wrap them in rusqlite::Transaction::new_unchecked(&conn, \
                 rusqlite::TransactionBehavior::Immediate) (DoD \u{a7}C)"
            ));
        }
    }
    for frozen in FROZEN_UNTRANSACTED_WRITERS {
        if !offenders.iter().any(|o| o == frozen) {
            violations.push(format!(
                "{frozen}: no longer an untransacted multi-writer — delete it from \
                 FROZEN_UNTRANSACTED_WRITERS so it can never regress back (per-fn ratchet)"
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "own-connection multi-write guard (G8):\n{}",
        violations.join("\n")
    );
}

#[cfg(test)]
mod predicate_tests {
    use super::is_untransacted_multi_writer;

    /// Needles are built via `format!` so this file's own fixtures never
    /// match `source_tree_guards`'s crate-wide scans (`no-assert-ok`-style
    /// self-matching hazard the wave-1 guards already guard against).
    fn checkout_line() -> String {
        format!("let {} = self.db.{}()?;", "connection", "checkout")
    }

    fn write_call(sql: &str) -> String {
        format!("connection.execute(\"{sql}\", params![id])?;")
    }

    #[test]
    fn own_checkout_with_two_writes_is_flagged() {
        let body = format!(
            "{}\n{}\n{}",
            checkout_line(),
            write_call("DELETE FROM t WHERE id = ?1"),
            write_call("INSERT INTO t (id) VALUES (?1)")
        );
        assert!(is_untransacted_multi_writer(&body));
    }

    #[test]
    fn own_checkout_with_one_write_is_not_flagged() {
        let body = format!("{}\n{}", checkout_line(), write_call("UPDATE t SET x = 1"));
        assert!(!is_untransacted_multi_writer(&body));
    }

    #[test]
    fn two_writes_inside_new_unchecked_transaction_are_not_flagged() {
        let body = format!(
            "{}\nlet tx = rusqlite::{}::{}(&connection, rusqlite::TransactionBehavior::Immediate)?;\n{}\n{}",
            checkout_line(),
            "Transaction",
            "new_unchecked",
            write_call("DELETE FROM t WHERE id = ?1").replace("connection.", "tx."),
            write_call("INSERT INTO t (id) VALUES (?1)").replace("connection.", "tx.")
        );
        assert!(!is_untransacted_multi_writer(&body));
    }

    #[test]
    fn transaction_word_only_in_a_comment_still_flags() {
        let body = format!(
            "// still transactional, honest\n{}\n{}\n{}",
            checkout_line(),
            write_call("DELETE FROM t WHERE id = ?1"),
            write_call("INSERT INTO t (id) VALUES (?1)")
        );
        assert!(
            is_untransacted_multi_writer(&body),
            "a comment-only mention of \"transaction\" must not exempt a real offender"
        );
    }

    #[test]
    fn transaction_word_only_in_a_string_literal_still_flags() {
        let body = format!(
            "let note = \"logging a transaction here\";\n{}\n{}\n{}",
            checkout_line(),
            write_call("DELETE FROM t WHERE id = ?1"),
            write_call("INSERT INTO t (id) VALUES (?1)")
        );
        assert!(
            is_untransacted_multi_writer(&body),
            "a string-literal-only mention of \"transaction\" must not exempt a real offender"
        );
    }

    #[test]
    fn fn_taking_borrowed_connection_is_not_scanned() {
        // No `.checkout()` at all — Group B (#461), deliberately out of scope.
        let body = format!(
            "{}\n{}",
            write_call("DELETE FROM t WHERE id = ?1"),
            write_call("INSERT INTO t (id) VALUES (?1)")
        );
        assert!(!is_untransacted_multi_writer(&body));
    }
}
