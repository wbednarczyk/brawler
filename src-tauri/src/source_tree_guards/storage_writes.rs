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
//!
//! ponytail: known ceiling — the write-statement count is textual, not
//! branch-aware, so two writes in mutually exclusive `if`/`else` arms (no
//! real path issues both) still flag. Escape hatch: a comment reading
//! `// multi-write-ok: <reason>` on the line directly above the fn signature
//! exempts it (mirrors command_shapes.rs's `// offload-ok:`) — a reviewed
//! reason, never a silent skip (ADR 0045); a blank reason still fails (G11,
//! `escape_hatch_reasons_are_non_empty`). Upgrade path is branch-aware
//! counting (walk the fn body's actual control-flow tree instead of a flat
//! token scan) if a real offender's fix ever gets expensive enough that the
//! IMMEDIATE-transaction wrap isn't the cheaper move.

use std::path::Path;

use super::scan::{
    contains_word_token, escape_hatch_reason_above, is_test_file, paren_matched_text,
    strip_comments_and_strings, strip_test_spans,
};
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
/// an INSERT/UPDATE/DELETE literal, case-insensitive) with no transaction
/// marker in the real code (comments/strings don't count — a `// still
/// transactional` comment or a log string mentioning "transaction" must not
/// exempt a real offender).
fn is_untransacted_multi_writer(body: &str) -> bool {
    if !body.contains(".checkout()") {
        return false;
    }
    if write_statement_count(body) < 2 {
        return false;
    }
    let code_only = strip_comments_and_strings(body);
    !has_transaction_marker(&code_only)
}

/// Whether `code_only` (already comment/string-stripped) carries a
/// transaction marker: `new_unchecked(`/`TransactionBehavior`/`.commit()`
/// literally, or a `transaction` IDENTIFIER token — word boundaries, so a
/// merely-similarly-named identifier like `transaction_count` does not
/// exempt an otherwise-untransacted writer.
fn has_transaction_marker(code_only: &str) -> bool {
    code_only.contains("new_unchecked(")
        || code_only.contains("TransactionBehavior")
        || code_only.contains(".commit()")
        || contains_word_token(code_only, "transaction")
}

/// Count `.execute(`/`execute_batch(` calls whose parenthesized argument text
/// contains an INSERT/UPDATE/DELETE SQL literal, case-insensitive.
///
/// `.execute(` takes exactly one statement, so a matching call counts once;
/// its keyword search runs over the RAW (unstripped) argument text — the SQL
/// literal is inside quotes, so this must not blank strings first.
/// `execute_batch(` takes a semicolon-separated SQL string that can pack
/// several writes into one call — counted per-statement via
/// [`count_batch_write_statements`], which decodes the Rust string literal
/// FIRST (so a raw string's `r#"` wrapper can't swallow a keyword, and a `;`
/// inside a SQL string value isn't mistaken for a statement boundary) so
/// e.g. one `execute_batch("INSERT ...; UPDATE ...;")` counts 2, not 1.
fn write_statement_count(body: &str) -> usize {
    let mut count = 0;
    for marker in [".execute(", "execute_batch("] {
        let mut search_from = 0;
        while let Some(rel) = body[search_from..].find(marker) {
            let open = search_from + rel + marker.len() - 1;
            match paren_matched_text(body, open) {
                Some(args) => {
                    count += if marker == "execute_batch(" {
                        count_batch_write_statements(args)
                    } else {
                        let upper = args.to_uppercase();
                        usize::from(
                            ["INSERT", "UPDATE", "DELETE"]
                                .iter()
                                .any(|keyword| upper.contains(keyword)),
                        )
                    };
                    search_from = open + args.len() + 2;
                }
                None => search_from = open + 1,
            }
        }
    }
    count
}

/// Count write statements packed into one `execute_batch(...)` call. `arg` is
/// the RAW Rust argument text (the literal's quotes, escapes, and `r#"`
/// wrapper included) — it is decoded to the actual SQL string first via
/// [`decode_rust_string_literal`], THEN split into statements, so neither a
/// raw-string's `r#"` prefix nor an escape sequence corrupts the keyword
/// match. A non-literal argument (a variable, `&format!(...)`, ...) can't be
/// decoded — its statement count is undecidable, so it counts as ONE write
/// (never zero, never a guess at how many).
fn count_batch_write_statements(arg: &str) -> usize {
    match decode_rust_string_literal(arg) {
        Some(sql) => count_sql_write_statements(&sql),
        None => 1,
    }
}

/// Decode a Rust string-literal's textual content into the string it
/// represents: a plain `"..."` (processing `\n`/`\t`/`\r`/`\0`/`\\`/`\"`/`\'`
/// escapes — SQL text realistically uses no others) or a raw `r"..."`/
/// `r#"..."#`/`r##"..."##`/... (no escapes at all). `arg` may carry
/// surrounding whitespace or a leading `&`. Returns `None` when `arg` isn't a
/// string literal at all (a bare identifier, `&format!(...)`, ...) or the
/// literal is malformed (unterminated) — the caller then can't inspect
/// statement boundaries.
fn decode_rust_string_literal(arg: &str) -> Option<String> {
    let arg = arg.trim().strip_prefix('&').unwrap_or(arg.trim()).trim();
    if let Some(rest) = arg.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(out),
                '\\' => out.push(match chars.next()? {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '0' => '\0',
                    other => other, // \\, \", \' and any other escape: literal char
                }),
                other => out.push(other),
            }
        }
        None
    } else if let Some(rest) = arg.strip_prefix('r') {
        let hashes = rest.chars().take_while(|&c| c == '#').count();
        let body = rest.strip_prefix(&"#".repeat(hashes))?.strip_prefix('"')?;
        let closer = format!("\"{}", "#".repeat(hashes));
        body.find(&closer).map(|end| body[..end].to_string())
    } else {
        None
    }
}

/// Count write-statement starts (immediately after `;`, or at the start of
/// `sql`, once leading punctuation/whitespace chars are skipped) that begin
/// with INSERT/UPDATE/DELETE, case-insensitive. `sql` is the DECODED SQL
/// text (no Rust literal quoting left) — split single-quote aware (`''`
/// escapes a literal quote inside a SQL string, and a `;` inside an open
/// `'...'` is data, not a statement boundary — e.g. `INSERT ... VALUES ('a;
/// UPDATE bogus')` is one statement, not two).
fn count_sql_write_statements(sql: &str) -> usize {
    let mut count = 0;
    let mut segment_start = 0;
    let mut in_string = false;
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' if in_string && bytes.get(i + 1) == Some(&b'\'') => i += 2, // '' escape
            b'\'' => {
                in_string = !in_string;
                i += 1;
            }
            b';' if !in_string => {
                count += usize::from(segment_is_write_statement(&sql[segment_start..i]));
                segment_start = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    count += usize::from(segment_is_write_statement(&sql[segment_start..]));
    count
}

/// Whether `segment` (one `;`-delimited SQL statement) starts with
/// INSERT/UPDATE/DELETE, case-insensitive, once leading non-alphabetic
/// punctuation/whitespace is skipped.
fn segment_is_write_statement(segment: &str) -> bool {
    let trimmed = segment
        .trim_start_matches(|c: char| !c.is_ascii_alphabetic())
        .to_uppercase();
    ["INSERT", "UPDATE", "DELETE"]
        .iter()
        .any(|keyword| trimmed.starts_with(keyword))
}

/// Whether a `// multi-write-ok: <non-empty reason>` comment sits directly
/// on the line above `lines[fn_line]` (the fn signature) — a reviewed
/// exception for the documented if/else ceiling (module docs): two writes in
/// mutually exclusive branches that still flag textually despite no real
/// path issuing both. Mirrors command_shapes.rs's `// offload-ok:` handling
/// via the shared [`escape_hatch_reason_above`]. A blank reason does NOT
/// exempt — same rule G11 (`escape_hatch_reasons_are_non_empty`) enforces
/// crate-wide.
fn multi_write_ok(lines: &[&str], fn_line: usize) -> bool {
    escape_hatch_reason_above(lines, fn_line, "// multi-write-ok:").is_some_and(|r| !r.is_empty())
}

/// Every untransacted multi-writer fn in `content` (one file's source, test
/// spans already excluded by the caller — `rel` labels the resulting
/// `"<rel>:<fn>"` ids). Standalone from disk I/O so synthetic snippets can
/// exercise it directly.
fn scan_untransacted_multi_writers_in(rel: &str, content: &str) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut offenders = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        if !is_fn_signature(lines[idx].trim_start()) {
            idx += 1;
            continue;
        }
        let fn_line = idx;
        let body = extract_fn_body(content, &lines, fn_line);
        if is_untransacted_multi_writer(&body) && !multi_write_ok(&lines, fn_line) {
            let name = extract_test_fn_name(lines[fn_line]);
            offenders.push(format!("{rel}:{name}"));
        }
        idx = fn_line + 1;
    }
    offenders
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
        offenders.extend(scan_untransacted_multi_writers_in(&rel, &content));
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
    use super::{is_untransacted_multi_writer, scan_untransacted_multi_writers_in};

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
    fn similarly_named_identifier_does_not_exempt_a_real_offender() {
        // "transaction_count" contains "transaction" as a substring but is a
        // DIFFERENT identifier — must not be mistaken for a real transaction
        // marker (word-boundary fix).
        let body = format!(
            "{}\nlet transaction_count = 2;\n{}\n{}",
            checkout_line(),
            write_call("DELETE FROM t WHERE id = ?1"),
            write_call("INSERT INTO t (id) VALUES (?1)")
        );
        assert!(
            is_untransacted_multi_writer(&body),
            "a `transaction_count` identifier must not exempt an untransacted offender"
        );
    }

    #[test]
    fn execute_batch_with_two_writes_counts_as_two() {
        let body = format!(
            "{}\nconnection.execute_batch(\"INSERT INTO t (id) VALUES (1); UPDATE t SET x = 1 WHERE id = 1;\")?;",
            checkout_line()
        );
        assert!(
            is_untransacted_multi_writer(&body),
            "one execute_batch call packing two writes must count as 2, not 1"
        );
    }

    #[test]
    fn execute_batch_with_one_write_is_not_flagged() {
        let body = format!(
            "{}\nconnection.execute_batch(\"INSERT INTO t (id) VALUES (1);\")?;",
            checkout_line()
        );
        assert!(!is_untransacted_multi_writer(&body));
    }

    /// Regression (a): a `r#"..."#` raw-string batch's leading `r#"` must not
    /// swallow the first INSERT keyword — the old textual split blanked this
    /// to 1, not 2, because `trim_start_matches(!is_alphabetic)` stopped
    /// immediately at the `r` in `r#"`.
    #[test]
    fn execute_batch_raw_string_with_two_writes_counts_as_two() {
        let body = format!(
            "{}\nconnection.execute_batch(r#\"INSERT INTO t (id) VALUES (1); UPDATE t SET x = 1 WHERE id = 1;\"#)?;",
            checkout_line()
        );
        assert!(
            is_untransacted_multi_writer(&body),
            "a raw-string execute_batch call packing two writes must count as 2, not 1"
        );
    }

    /// Regression (b): a `;` inside a single-quoted SQL string VALUE is data,
    /// not a statement boundary — the old blind `.split(';')` over raw text
    /// miscounted this as two writes; it is one INSERT.
    #[test]
    fn execute_batch_semicolon_inside_sql_string_literal_is_not_a_statement_boundary() {
        let body = format!(
            "{}\nconnection.execute_batch(\"INSERT INTO t (x) VALUES ('a; UPDATE bogus')\")?;",
            checkout_line()
        );
        assert!(
            !is_untransacted_multi_writer(&body),
            "a `;` inside a single-quoted SQL string value must not be mistaken for a statement \
             boundary — this is one INSERT, not two writes"
        );
    }

    /// Regression (c): a non-literal execute_batch argument (a variable, a
    /// `format!(...)` call) can't be decoded into SQL text — undecidable
    /// statement count must count as ONE write, never zero (a silent
    /// under-count) and never a guess at how many. Paired here with a second
    /// `.execute(` write so the fn totals \u{2265}2 and is correctly flagged.
    #[test]
    fn execute_batch_with_a_non_literal_argument_counts_as_one_write() {
        let body = format!(
            "{}\nconnection.execute_batch(&format!(\"INSERT INTO t (id) VALUES ({{id}})\"))?;\n{}",
            checkout_line(),
            write_call("UPDATE t SET x = 1")
        );
        assert!(
            is_untransacted_multi_writer(&body),
            "a non-literal execute_batch argument must count as one write, combining with the \
             second .execute( write to total \u{2265}2 and flag"
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

    #[test]
    fn multi_write_ok_comment_with_a_reason_exempts_the_fn() {
        let source = format!(
            "// multi-write-ok: mutually exclusive if/else arms, see module docs\nfn offender() {{\n    {}\n    {}\n    {}\n}}\n",
            checkout_line(),
            write_call("DELETE FROM t WHERE id = ?1"),
            write_call("INSERT INTO t (id) VALUES (?1)")
        );
        let offenders = scan_untransacted_multi_writers_in("storage/x.rs", &source);
        assert!(
            offenders.is_empty(),
            "a non-empty multi-write-ok reason above the fn signature must exempt it"
        );
    }

    #[test]
    fn multi_write_ok_comment_with_an_empty_reason_does_not_exempt() {
        let source = format!(
            "// multi-write-ok:\nfn offender() {{\n    {}\n    {}\n    {}\n}}\n",
            checkout_line(),
            write_call("DELETE FROM t WHERE id = ?1"),
            write_call("INSERT INTO t (id) VALUES (?1)")
        );
        let offenders = scan_untransacted_multi_writers_in("storage/x.rs", &source);
        assert_eq!(
            offenders,
            vec!["storage/x.rs:offender".to_string()],
            "a blank multi-write-ok reason must not exempt the fn (mirrors G11)"
        );
    }
}
