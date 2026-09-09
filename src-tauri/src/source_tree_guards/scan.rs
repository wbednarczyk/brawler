//! Shared source-scan primitives (hard gates wave 2, G8/G9): the DFS `.rs`
//! walker that used to be copy-pasted per guard in `mod.rs` (still duplicated
//! outside this module in `storage/tests/schema.rs` and
//! `jobs/activity_awaited_paths.rs` — out of this slice's scope), the 4-clause
//! test-file predicate (`jobs/activity_awaited_paths.rs:129-170`), and
//! comment/string-safe span handling so a scan sees production code only.

use std::path::{Path, PathBuf};

/// Every `.rs` file under `root`, absolute paths, depth-first (order not
/// guaranteed). The walker previously copy-pasted per guard in this crate.
pub(super) fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable source dir") {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out
}

/// Whether `rel_path` (repo-relative, `/`-separated) is a test file — the
/// 4-clause predicate from `jobs::activity_awaited_paths::tests::all_source_files`
/// callers: a call inside one of these is deliberate, direct, unwrapped test
/// setup, not a production offender.
pub(super) fn is_test_file(rel_path: &str) -> bool {
    rel_path.ends_with("_tests.rs")
        || rel_path.ends_with("/tests.rs")
        || rel_path == "tests.rs"
        || rel_path.contains("/tests/")
}

/// Byte ranges of `content` that are inside a quoted string, raw string, or
/// `//`/`/* */` comment — shares the string/raw-string skip logic
/// `extract_fn_body` (in `mod.rs`) uses for brace-matching; comments are new
/// here since fn-body extraction never previously needed to look past one.
fn non_code_spans(content: &str) -> Vec<(usize, usize)> {
    let bytes = content.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i = (i + 1).min(bytes.len());
                spans.push((start, i));
            }
            b'r' if matches!(bytes.get(i + 1), Some(b'"') | Some(b'#')) => {
                let start = i;
                let mut j = i + 1;
                let mut hashes = 0usize;
                while bytes.get(j) == Some(&b'#') {
                    hashes += 1;
                    j += 1;
                }
                if bytes.get(j) == Some(&b'"') {
                    j += 1;
                    while j < bytes.len() {
                        if bytes[j] == b'"' {
                            let mut k = j + 1;
                            let mut seen = 0usize;
                            while seen < hashes && bytes.get(k) == Some(&b'#') {
                                k += 1;
                                seen += 1;
                            }
                            if seen == hashes {
                                j = k;
                                break;
                            }
                        }
                        j += 1;
                    }
                    spans.push((start, j));
                    i = j;
                    continue;
                }
                i += 1;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let start = i;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                spans.push((start, i));
            }
            // A char/byte-char literal (`'{'`, `'\''`, `'\u{7b}'`, `b'{'`)
            // must be blanked too — an unblanked `'{'` corrupts a caller's
            // brace-depth count — but a LIFETIME (`'a`, `impl<'a>`) must
            // survive untouched, so this only fires when the `'` is
            // actually followed by a closing `'` (escaped or one-char).
            b'\'' => {
                if let Some(end) = char_literal_end(bytes, i) {
                    spans.push((i, end));
                    i = end;
                    continue;
                }
                i += 1;
            }
            b'b' if bytes.get(i + 1) == Some(&b'\'') => {
                if let Some(end) = char_literal_end(bytes, i + 1) {
                    spans.push((i, end));
                    i = end;
                    continue;
                }
                i += 1;
            }
            // Rust block comments NEST (`/* outer /* inner */ still-comment */`
            // is one comment, not "outer" plus the code between the two `*/`s)
            // — a depth counter tracks that instead of stopping at the first
            // `*/`.
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let start = i;
                let mut depth = 1usize;
                i += 2;
                while depth > 0 && i < bytes.len() {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                spans.push((start, i));
            }
            _ => i += 1,
        }
    }
    spans
}

/// If `bytes[quote_pos]` (a `'`) opens a char/byte-char literal — an
/// escaped one (`'\n'`, `'\''`, `'\u{7b}'`) or a plain one-char literal
/// (`'{'`) — the exclusive end offset just past its closing `'`. `None` for
/// a lifetime (`'a`, `'static`), which the caller then leaves untouched.
fn char_literal_end(bytes: &[u8], quote_pos: usize) -> Option<usize> {
    if bytes.get(quote_pos + 1) == Some(&b'\\') {
        // Escaped: the same "skip 2 after `\`, else 1" scan the plain
        // string-literal branch above uses — it can't be fooled by an
        // escaped quote (`\'`, 2 bytes) into stopping early, regardless of
        // which escape kind (`\n`, `\xHH`, `\u{...}`, ...) follows.
        let mut j = quote_pos + 1;
        while j < bytes.len() && bytes[j] != b'\'' {
            j += if bytes[j] == b'\\' { 2 } else { 1 };
        }
        return Some((j + 1).min(bytes.len()));
    }
    if bytes.get(quote_pos + 2) == Some(&b'\'') {
        return Some(quote_pos + 3);
    }
    None
}

/// Blank (space out, preserving newlines) every quoted-string, raw-string,
/// and `//`/`/* */` comment span in `content`, so a plain substring/token
/// search only ever matches real code — the "outside comments/strings" rule
/// G8's own-connection/transaction check needs (a `// still transactional`
/// comment or a log string mentioning "transaction" must not exempt a real
/// offender). Byte-for-byte length-preserving, so callers may keep using
/// offsets/line numbers computed against the original `content`.
pub(super) fn strip_comments_and_strings(content: &str) -> String {
    let mut out = content.as_bytes().to_vec();
    for (start, end) in non_code_spans(content) {
        for byte in &mut out[start..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    String::from_utf8(out).expect("blanking only replaces bytes with ASCII spaces")
}

/// Whether `token` appears in `text` as a whole identifier — not as a
/// substring of a larger one (word boundaries: an ASCII alphanumeric-or-`_`
/// byte on either side means it's part of a bigger token, e.g.
/// `transaction_count` must not match the token `transaction`, and
/// `rename_all = "async_x"` must not match the token `async`). Shared by
/// G8's transaction-marker check and G9's `command(async)` attribute parse.
pub(super) fn contains_word_token(text: &str, token: &str) -> bool {
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let bytes = text.as_bytes();
    let mut start = 0;
    while let Some(rel) = text[start..].find(token) {
        let idx = start + rel;
        let before_ok = idx == 0 || !is_ident(bytes[idx - 1]);
        let after = idx + token.len();
        let after_ok = after >= bytes.len() || !is_ident(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        start = idx + 1;
    }
    false
}

/// The reason text of a `// <marker>` comment (e.g. `"// offload-ok:"`,
/// `"// multi-write-ok:"`) sat directly on the line above `lines[idx]`, if
/// any — `None` when the line above isn't that comment, `Some("")` when it
/// is but carries no reason (still fails whatever exemption check the caller
/// runs on it — a blank reason must not exempt anything; see G11,
/// `escape_hatch_reasons_are_non_empty`). Shared shape behind every guard's
/// reviewed-escape-hatch handling (command_shapes.rs's `// offload-ok:`,
/// storage_writes.rs's `// multi-write-ok:`).
pub(super) fn escape_hatch_reason_above(
    lines: &[&str],
    idx: usize,
    marker: &str,
) -> Option<String> {
    if idx == 0 {
        return None;
    }
    lines[idx - 1]
        .trim_start()
        .strip_prefix(marker)
        .map(|reason| reason.trim().to_string())
}

/// Byte offset just past the `)`/`}` matching the opening delimiter at
/// `open`, skipping string/comment content. `open_byte`/`close_byte` are
/// `b'('`/`b')'` or `b'{'`/`b'}'`.
fn matched_delimiter_end(
    content: &str,
    open: usize,
    open_byte: u8,
    close_byte: u8,
) -> Option<usize> {
    let stripped = strip_comments_and_strings(content);
    let bytes = stripped.as_bytes();
    if bytes.get(open) != Some(&open_byte) {
        return None;
    }
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        if bytes[i] == open_byte {
            depth += 1;
        } else if bytes[i] == close_byte {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

/// The text strictly between the `(` at `open` and its matching `)` (real
/// code only — the delimiter search skips comments/strings, but the returned
/// slice is the ORIGINAL content, so a SQL literal's own text is preserved
/// for keyword matching).
pub(super) fn paren_matched_text(content: &str, open: usize) -> Option<&str> {
    let end = matched_delimiter_end(content, open, b'(', b')')?;
    Some(&content[open + 1..end - 1])
}

/// Blank every brace-matched `#[cfg(test)] mod …` block (both the `mod name;`
/// external-file-declaration form and the inline `mod name { … }` form) and
/// every `#[test]`/`#[tokio::test]` fn (attributes through the matching `}`)
/// in `content`, so a production-code scan never mistakes test-only code for
/// a real offender — the same care G14's inline-test-span detection takes.
/// Length-preserving like `strip_comments_and_strings` (newlines kept).
pub(super) fn strip_test_spans(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut line_offsets = Vec::with_capacity(lines.len());
    let mut acc = 0usize;
    for line in &lines {
        line_offsets.push(acc);
        acc += line.len() + 1;
    }

    let mut blank_ranges: Vec<(usize, usize)> = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        let trimmed = lines[idx].trim_start();
        let is_cfg_test = trimmed == "#[cfg(test)]";
        let is_test_attr = trimmed == "#[test]" || trimmed.starts_with("#[tokio::test");
        if !is_cfg_test && !is_test_attr {
            idx += 1;
            continue;
        }
        let attr_start_line = idx;
        let mut item_line = idx;
        while item_line < lines.len() && lines[item_line].trim_start().starts_with("#[") {
            item_line += 1;
        }
        if item_line >= lines.len() {
            idx += 1;
            continue;
        }
        let item_trimmed = lines[item_line].trim_start();
        let is_mod_item = is_cfg_test
            && ["mod ", "pub mod ", "pub(crate) mod ", "pub(super) mod "]
                .iter()
                .any(|prefix| item_trimmed.starts_with(prefix));
        let is_fn_item = is_test_attr && super::is_fn_signature(lines[item_line]);
        if !is_mod_item && !is_fn_item {
            idx += 1;
            continue;
        }

        let item_start = line_offsets[item_line];
        let span_start = line_offsets[attr_start_line];
        let span_end = if is_mod_item && !item_trimmed.contains('{') {
            // `mod name;` — an external-file declaration; no body in THIS file.
            item_start + lines[item_line].len()
        } else {
            let Some(brace_open) = content[item_start..].find('{').map(|rel| item_start + rel)
            else {
                idx = item_line + 1;
                continue;
            };
            let Some(end) = matched_delimiter_end(content, brace_open, b'{', b'}') else {
                idx = item_line + 1;
                continue;
            };
            end
        };
        blank_ranges.push((span_start, span_end));
        let consumed_line = content[..span_end].matches('\n').count();
        idx = consumed_line.max(item_line) + 1;
    }

    let mut out = content.as_bytes().to_vec();
    for (start, end) in blank_ranges {
        for byte in &mut out[start..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    String::from_utf8(out).expect("blanking only replaces bytes with ASCII spaces")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_comments_and_strings_blanks_a_nested_block_comment_fully() {
        let content = "/* outer /* inner */\ncode_after_inner();\n*/\nreal_code();\n";
        let stripped = strip_comments_and_strings(content);
        assert!(
            !stripped.contains("code_after_inner"),
            "text between the inner and outer `*/` is still inside the outer \
             comment (Rust block comments nest): {stripped:?}"
        );
        assert!(
            stripped.contains("real_code();"),
            "code after the (fully closed) nested comment must survive: {stripped:?}"
        );
        assert_eq!(
            stripped.len(),
            content.len(),
            "blanking must be byte-length-preserving"
        );
    }

    #[test]
    fn strip_comments_and_strings_blanks_char_and_byte_char_literals() {
        for literal in ["'{'", "'\\''", "'\\u{7b}'", "b'{'"] {
            let content = format!("const OPEN: char = {literal};\nmod tests;\n");
            let stripped = strip_comments_and_strings(&content);
            assert!(
                !stripped.contains('{') && !stripped.contains('}'),
                "the char/byte-char literal {literal} must be blanked so it \
                 cannot corrupt a caller's brace count: {stripped:?}"
            );
        }
    }

    #[test]
    fn strip_comments_and_strings_leaves_lifetimes_alone() {
        for content in ["fn f(x: &'a str) {}", "impl<'a> Widget<'a> {}"] {
            let stripped = strip_comments_and_strings(content);
            assert_eq!(
                stripped, content,
                "a lifetime must never be mistaken for a char literal: {stripped:?}"
            );
        }
    }
}
