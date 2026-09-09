//! Transform-manifest guard helpers (G7, ADR 0049, issue #194 S0): resolving
//! a file module's declared test submodule into its content (`mod`/`#[path]`
//! declarations at brace depth 0 of the file), and validating a manifest
//! row's out-of-file `"proptest_in": "<file>::<fn>"` claim. That claim is a
//! TEXT-LEVEL contract, not Rust scope resolution: over comment/string-
//! stripped, brace-depth-tracked text (no parser), it verifies `<fn>` is a
//! `#[test]` fn sitting at brace depth exactly 1 — a direct child — inside a
//! `proptest! { ... }` block in `<file>`, that its body calls `<stem>::…`
//! (never a decoy nested fn), and that `<file>` itself imports the
//! module's `<parent>::<stem>` path. It cannot tell which of two modules
//! sharing a file stem a bare `<stem>::` call means — the manifest row's
//! full path + fn name is what a reviewer actually reads. Kept out of
//! `mod.rs` to stay under the file-size ratchet threshold (ADR 0103) — same
//! reasoning as `scan.rs`.

use std::path::Path;

use super::extract_fn_body;
use super::scan::{contains_word_token, strip_comments_and_strings};

/// The content of every module `path` (a `.rs` file) declares at top level
/// with `mod x;` (any visibility) — a preceding `#[path = "P"]` attribute
/// resolves relative to `path`'s directory; otherwise the target is
/// `<dir>/<stem>/x.rs`, falling back to `<dir>/<stem>/x/mod.rs`. One level
/// deep: this is how a file module's declared test submodule (`esef.rs`'s
/// `mod tests;` -> `esef/tests.rs`; `report_documents_capture.rs`'s
/// `#[path = "report_documents_capture_tests.rs"] mod tests;`) counts toward
/// the manifest guard's `proptest!`/`insta::` search without the guard
/// having to know every file's test-module layout by hand. Only a
/// **brace-depth-0** declaration counts — depth tracked over the whole file
/// (a running `{`/`}` counter, comment/string-blanked so a brace inside a
/// literal never perturbs it), not indentation: `mod tests;` nested inside
/// some other block (`#[cfg(any())]\nmod disabled {\nmod tests;\n}`, even
/// unindented) is at depth 1 and does not count, while an oddly-indented but
/// genuinely top-level declaration does. Comments/strings are blanked before
/// matching (via `strip_comments_and_strings`) so a `mod tests;` sitting
/// inside `/* … */` or a multi-line string fixture is never mistaken for a
/// real declaration either. The `#[path = "…"]` filename itself is read back
/// from the *unstripped* line, since the stripper blanks its own string
/// value too.
pub(super) fn declared_test_module_content(path: &Path, content: &str) -> String {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let mut extra = String::new();
    let mut pending_path_attr: Option<&str> = None;
    let stripped = strip_comments_and_strings(content);
    let mut depth = 0i32;
    for (line, stripped_line) in content.lines().zip(stripped.lines()) {
        // The depth THIS line starts at (before its own braces, if any, are
        // counted below) is what decides top-level-ness — a `mod`/`#[path]`
        // written anywhere but depth 0 is nested, regardless of indentation.
        let line_depth = depth;
        for byte in stripped_line.bytes() {
            match byte {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
        }
        if line_depth != 0 {
            pending_path_attr = None;
            continue;
        }
        let trimmed = stripped_line.trim_start();
        if trimmed.starts_with("#[path") {
            pending_path_attr = line
                .trim_start()
                .strip_prefix("#[path")
                .and_then(|rest| rest.trim_start().strip_prefix('='))
                .and_then(|rest| rest.trim().trim_start_matches('"').split('"').next());
            continue;
        }
        let mod_name = ["mod ", "pub mod ", "pub(crate) mod ", "pub(super) mod "]
            .iter()
            .find_map(|prefix| trimmed.strip_prefix(prefix))
            .and_then(|rest| rest.strip_suffix(';'))
            .map(str::trim);
        let Some(name) = mod_name else {
            pending_path_attr = None;
            continue;
        };
        let target = match pending_path_attr.take() {
            Some(p) => dir.join(p),
            None => {
                let by_file = dir.join(stem).join(format!("{name}.rs"));
                if by_file.exists() {
                    by_file
                } else {
                    dir.join(stem).join(name).join("mod.rs")
                }
            }
        };
        if let Ok(module_content) = std::fs::read_to_string(&target) {
            extra.push('\n');
            extra.push_str(&module_content);
        }
    }
    extra
}

/// The stem the manifest guard's `proptest_in` cross-reference matches
/// against: a file row's file stem (`bankier_rss.rs` -> `bankier_rss`) or a
/// directory row's directory name.
fn module_stem(module_path: &str) -> &str {
    let name = module_path.rsplit('/').next().unwrap_or(module_path);
    name.strip_suffix(".rs").unwrap_or(name)
}

/// The Rust module path segment a real `<parent>::<stem>` reference would
/// use — `module_path`'s directory segment immediately above the stem
/// (`src/source_adapters/bankier_rss.rs` -> `source_adapters`;
/// `src/storage/ingestion.rs` -> `storage`;
/// `src/fundamentals/extraction/html.rs` -> `extraction`). `None` for a
/// module directly under `src/` (`src/foo.rs`): `src` is a file-system
/// prefix, never written in a Rust `use`/path, so there is no parent to
/// require.
fn module_parent(module_path: &str) -> Option<&str> {
    let parts: Vec<&str> = module_path.split('/').collect();
    if parts.len() < 3 {
        return None;
    }
    Some(parts[parts.len() - 2])
}

/// Validates a manifest row's `"proptest_in": "<file>::<fn>"` claim (the
/// property test lives elsewhere, e.g. `tests/parser_fuzz.rs`, rather than in
/// the module's own file/declared test submodule): `<file>` (relative to
/// `manifest_dir`, i.e. `src-tauri/`) exists and `<fn>` names a genuine
/// top-level property — `find_property_fn_line` — a `#[test]` fn at brace
/// depth exactly 1 (a direct child) inside a `proptest! { ... }` block, not
/// merely present somewhere in the file (a helper nested deeper, or a
/// same-named fn outside any `proptest!` block, does not count; two or more
/// qualifying fns of that name is "ambiguous"). The fn's own brace-matched
/// body (via `extract_fn_body`) then makes a genuine call/path reference to
/// `<stem>::` (`body_references_stem`), where `<stem>` is `module_path`'s
/// (`module_stem`). Any failure is a plain reason string — the caller names
/// the manifest row.
///
/// Two more checks close the gap `<stem>::` alone leaves: the body must not
/// hide its `<stem>::` reference inside a never-called nested `fn` item
/// (`body_defines_nested_fn` — an uncalled decoy can wrap a real call while
/// the outer body does nothing), and — since `<stem>::` alone cannot
/// distinguish two different modules that happen to share a file stem
/// (`foo/bar.rs` and `baz/bar.rs` both stem to `bar`) — the FILE itself must
/// reference the module's full `<parent>::<stem>` path (`module_parent`) in
/// a `use` or path, not just name-match the stem.
pub(super) fn proptest_in_is_valid(
    manifest_dir: &Path,
    module_path: &str,
    spec: &str,
) -> Result<(), String> {
    let (file_rel, fn_name) = spec
        .rsplit_once("::")
        .ok_or_else(|| format!("{spec:?} is not \"<file>::<fn>\""))?;
    let content = std::fs::read_to_string(manifest_dir.join(file_rel))
        .map_err(|_| format!("file {file_rel} does not exist"))?;
    let lines: Vec<&str> = content.lines().collect();
    let stripped = strip_comments_and_strings(&content);

    let fn_line = find_property_fn_line(&stripped, fn_name)
        .map_err(|reason| format!("fn {fn_name} {reason} in {file_rel}"))?;

    let body = extract_fn_body(&content, &lines, fn_line);
    if body_defines_nested_fn(&body) {
        return Err(
            "nested fn inside a property body is unsupported — call the transform directly"
                .to_owned(),
        );
    }
    let stem = module_stem(module_path);
    if !body_references_stem(&body, stem) {
        return Err(format!("fn {fn_name} body never references {stem}::"));
    }
    if let Some(parent) = module_parent(module_path) {
        let parent_path = format!("{parent}::{stem}");
        if !contains_word_token(&stripped, &parent_path) {
            return Err(format!(
                "{file_rel} never references {parent_path} (use or path) — <stem>:: alone \
                 cannot tell two same-stem modules apart"
            ));
        }
    }
    Ok(())
}

/// Whether `body` (raw, unstripped — comments/strings are blanked here)
/// defines a nested `fn <ident>(` item, as opposed to a function-pointer
/// TYPE like `fn(Args) -> Ret` (no identifier before the parens, so never
/// matches). A nested fn lets an attacker wrap a genuine `<stem>::` call in
/// a helper that the outer body never actually calls.
fn body_defines_nested_fn(body: &str) -> bool {
    let stripped = strip_comments_and_strings(body);
    let bytes = stripped.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut search_from = 0;
    while let Some(relative) = stripped[search_from..].find("fn") {
        let start = search_from + relative;
        let word_boundary_ok = start == 0 || !is_ident(bytes[start - 1]);
        let mut after_kw = start + 2;
        let ws_start = after_kw;
        while after_kw < bytes.len() && bytes[after_kw].is_ascii_whitespace() {
            after_kw += 1;
        }
        let has_ws = after_kw > ws_start;
        let ident_start = after_kw;
        let mut end = ident_start;
        while end < bytes.len() && is_ident(bytes[end]) {
            end += 1;
        }
        let has_ident = end > ident_start;
        if word_boundary_ok && has_ws && has_ident && bytes.get(end) == Some(&b'(') {
            return true;
        }
        search_from = start + 2;
    }
    false
}

/// Whether `body` makes a genuine call/path reference to `<stem>::` — not
/// merely the text appearing inside a comment or a string literal (blanked
/// via `strip_comments_and_strings` before matching), and not a bare
/// mention with nothing after it: a `<stem>::` word-bounded on the left,
/// followed by an identifier, followed by `(` (a call) or `::` (a further
/// path segment, including turbofish). A bare `stem::SOME_CONST` path use
/// with no call/further segment does not count — it is not enough to prove
/// the property actually exercises the module.
fn body_references_stem(body: &str, stem: &str) -> bool {
    let stripped = strip_comments_and_strings(body);
    let bytes = stripped.as_bytes();
    let needle = format!("{stem}::");
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut search_from = 0;
    while let Some(relative) = stripped[search_from..].find(&needle) {
        let start = search_from + relative;
        let word_boundary_ok = start == 0 || !is_ident(bytes[start - 1]);
        let ident_start = start + needle.len();
        let mut end = ident_start;
        while end < bytes.len() && is_ident(bytes[end]) {
            end += 1;
        }
        let has_ident = end > ident_start;
        let followed_by_call_or_path = matches!(bytes.get(end), Some(b'('))
            || (bytes.get(end) == Some(&b':') && bytes.get(end + 1) == Some(&b':'));
        if word_boundary_ok && has_ident && followed_by_call_or_path {
            return true;
        }
        search_from = start + 1;
    }
    false
}

/// The line index (into `content.lines()` — identical line boundaries to
/// `stripped`, the comment/string-blanked text this scans) of the `fn
/// <fn_name>(` that is a genuine top-level property: a DIRECT child of a
/// `proptest! { ... }` block — a brace-depth stack, not indentation, so a
/// helper nested one level deeper inside that property's own body never
/// qualifies (`fn t(` sitting behind `proptest! { fn outer() { fn t() {}
/// } }` is at depth 2, not depth 1) — with the nearest preceding non-blank
/// line, skipping any other stacked attribute, being `#[test]` (a plain
/// helper carries no such attribute). Modifier keywords (`pub`, `async`, …)
/// before `fn` need no special handling: they are always separated from
/// `fn` by whitespace, which already satisfies the left word-boundary check.
/// `Err` names why: the name never appears in the file at all, or it
/// appears but never as a top-level `#[test]` property, or it qualifies
/// more than once (ambiguous).
fn find_property_fn_line(stripped: &str, fn_name: &str) -> Result<usize, String> {
    let bytes = stripped.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let target = format!("fn {fn_name}(");
    let stripped_lines: Vec<&str> = stripped.lines().collect();

    // Stack of whether each currently-open (unmatched) `{` is a `proptest!`
    // macro block's own opening brace — "direct child" means this stack's
    // TOP is `true` right when the fn signature is found (not merely that
    // some ancestor brace is a proptest! block).
    let mut brace_stack: Vec<bool> = Vec::new();
    let mut name_found_anywhere = false;
    let mut candidate_lines: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                let mut before = i;
                while before > 0 && bytes[before - 1].is_ascii_whitespace() {
                    before -= 1;
                }
                brace_stack.push(stripped[..before].ends_with("proptest!"));
                i += 1;
                continue;
            }
            b'}' => {
                brace_stack.pop();
                i += 1;
                continue;
            }
            _ => {}
        }
        if stripped[i..].starts_with(&target) && (i == 0 || !is_ident(bytes[i - 1])) {
            name_found_anywhere = true;
            let is_direct_child = brace_stack.last() == Some(&true);
            let line = stripped[..i].matches('\n').count();
            if is_direct_child && has_test_attr_above(&stripped_lines, line) {
                candidate_lines.push(line);
            }
        }
        i += 1;
    }

    if !name_found_anywhere {
        return Err("not found".to_owned());
    }
    match candidate_lines.len() {
        0 => Err("is not a #[test] property directly inside a proptest! block".to_owned()),
        1 => Ok(candidate_lines[0]),
        n => Err(format!("is ambiguous: {n} qualifying #[test] properties")),
    }
}

/// Whether the nearest preceding non-blank line above `stripped_lines[line]`
/// is `#[test]` (or `#[tokio::test...]`) — blank lines and any OTHER
/// stacked attribute (`#[should_panic]` can sit above or below `#[test]`)
/// are skipped, but the walk stops (returning `false`) at the first real
/// code line.
fn has_test_attr_above(stripped_lines: &[&str], line: usize) -> bool {
    let mut idx = line;
    while idx > 0 {
        idx -= 1;
        let trimmed = stripped_lines[idx].trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "#[test]" || trimmed.starts_with("#[tokio::test") {
            return true;
        }
        if trimmed.starts_with("#[") {
            continue;
        }
        return false;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A fresh temp directory under the OS temp dir, cleaned up by the
    /// caller via `TempScratch::drop`.
    struct TempScratch(std::path::PathBuf);

    impl TempScratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "brawler-transform-manifest-test-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create temp scratch dir");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempScratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn read_module_content_follows_declared_mod_tests() {
        let scratch = TempScratch::new("mod-tests");
        let file = scratch.path().join("widget.rs");
        fs::write(&file, "pub fn widget() {}\n\n#[cfg(test)]\nmod tests;\n").unwrap();
        fs::create_dir_all(scratch.path().join("widget")).unwrap();
        fs::write(
            scratch.path().join("widget").join("tests.rs"),
            "proptest! { #[test] fn t(x in 0u32..1) { let _ = widget::widget(); } }\n",
        )
        .unwrap();

        let own = fs::read_to_string(&file).unwrap();
        let extra = declared_test_module_content(&file, &own);
        assert!(
            extra.contains("proptest!"),
            "declared `mod tests;` content should be pulled in: {extra:?}"
        );
    }

    #[test]
    fn read_module_content_follows_path_attribute() {
        let scratch = TempScratch::new("path-attr");
        let file = scratch.path().join("gadget.rs");
        fs::write(
            &file,
            "pub fn gadget() {}\n\n#[cfg(test)]\n#[path = \"gadget_tests.rs\"]\nmod tests;\n",
        )
        .unwrap();
        fs::write(
            scratch.path().join("gadget_tests.rs"),
            "proptest! { #[test] fn t(x in 0u32..1) { let _ = gadget::gadget(); } }\n",
        )
        .unwrap();

        let own = fs::read_to_string(&file).unwrap();
        let extra = declared_test_module_content(&file, &own);
        assert!(
            extra.contains("proptest!"),
            "`#[path = ...]`-declared mod content should be pulled in: {extra:?}"
        );
    }

    #[test]
    fn read_module_content_ignores_a_commented_out_mod_tests() {
        let scratch = TempScratch::new("commented-mod");
        let file = scratch.path().join("widget.rs");
        fs::write(
            &file,
            "pub fn widget() {}\n\n/*\n#[cfg(test)]\nmod tests;\n*/\n",
        )
        .unwrap();
        fs::create_dir_all(scratch.path().join("widget")).unwrap();
        fs::write(
            scratch.path().join("widget").join("tests.rs"),
            "proptest! { #[test] fn t(x in 0u32..1) { let _ = widget::widget(); } }\n",
        )
        .unwrap();

        let own = fs::read_to_string(&file).unwrap();
        let extra = declared_test_module_content(&file, &own);
        assert!(
            extra.is_empty(),
            "a `mod tests;` inside a block comment must not be honored: {extra:?}"
        );
    }

    #[test]
    fn read_module_content_ignores_a_nested_block_comment_mod_tests() {
        let scratch = TempScratch::new("nested-comment-mod");
        let file = scratch.path().join("widget.rs");
        fs::write(
            &file,
            "pub fn widget() {}\n\n/* outer /* inner */\nmod tests;\n*/\n",
        )
        .unwrap();
        fs::create_dir_all(scratch.path().join("widget")).unwrap();
        fs::write(
            scratch.path().join("widget").join("tests.rs"),
            "proptest! { #[test] fn t(x in 0u32..1) { let _ = widget::widget(); } }\n",
        )
        .unwrap();

        let own = fs::read_to_string(&file).unwrap();
        let extra = declared_test_module_content(&file, &own);
        assert!(
            extra.is_empty(),
            "a `mod tests;` still inside a NESTED block comment must not be honored: {extra:?}"
        );
    }

    #[test]
    fn read_module_content_ignores_an_indented_mod_tests() {
        let scratch = TempScratch::new("indented-mod");
        let file = scratch.path().join("widget.rs");
        fs::write(
            &file,
            "pub fn widget() {}\n\nfn inner() {\n    #[cfg(test)]\n    mod tests;\n}\n",
        )
        .unwrap();
        fs::create_dir_all(scratch.path().join("widget")).unwrap();
        fs::write(
            scratch.path().join("widget").join("tests.rs"),
            "proptest! { #[test] fn t(x in 0u32..1) { let _ = widget::widget(); } }\n",
        )
        .unwrap();

        let own = fs::read_to_string(&file).unwrap();
        let extra = declared_test_module_content(&file, &own);
        assert!(
            extra.is_empty(),
            "a non-top-level (indented) `mod tests;` must not be honored: {extra:?}"
        );
    }

    #[test]
    fn read_module_content_ignores_mod_tests_nested_inside_an_inline_mod_block() {
        // Column 0 is not the same as brace depth 0: this `mod tests;` is
        // unindented but sits at depth 1, inside the (disabled) inline
        // `mod disabled { ... }` block — a column-only check would wrongly
        // accept it.
        let scratch = TempScratch::new("nested-inline-mod");
        let file = scratch.path().join("widget.rs");
        fs::write(
            &file,
            "pub fn widget() {}\n\n#[cfg(any())]\nmod disabled {\nmod tests;\n}\n",
        )
        .unwrap();
        fs::create_dir_all(scratch.path().join("widget")).unwrap();
        fs::write(
            scratch.path().join("widget").join("tests.rs"),
            "proptest! { #[test] fn t(x in 0u32..1) { let _ = widget::widget(); } }\n",
        )
        .unwrap();

        let own = fs::read_to_string(&file).unwrap();
        let extra = declared_test_module_content(&file, &own);
        assert!(
            extra.is_empty(),
            "a `mod tests;` nested inside another (disabled) inline mod block, even at column \
             0, must not be honored: {extra:?}"
        );
    }

    #[test]
    fn read_module_content_resolves_an_indented_but_top_level_mod_tests() {
        // The flip side of brace depth replacing column 0: this `mod
        // tests;` is indented, but is not nested inside any block — real
        // top-level declaration, oddly formatted, and it must still
        // resolve.
        let scratch = TempScratch::new("indented-top-level-mod");
        let file = scratch.path().join("widget.rs");
        fs::write(
            &file,
            "pub fn widget() {}\n\n    #[cfg(test)]\n    mod tests;\n",
        )
        .unwrap();
        fs::create_dir_all(scratch.path().join("widget")).unwrap();
        fs::write(
            scratch.path().join("widget").join("tests.rs"),
            "proptest! { #[test] fn t(x in 0u32..1) { let _ = widget::widget(); } }\n",
        )
        .unwrap();

        let own = fs::read_to_string(&file).unwrap();
        let extra = declared_test_module_content(&file, &own);
        assert!(
            extra.contains("proptest!"),
            "an indented but genuinely top-level (depth-0) `mod tests;` should still resolve: \
             {extra:?}"
        );
    }

    /// `manifest_dir` for `proptest_in_is_valid` calls below: a scratch dir
    /// containing one `tests.rs` file, addressed as `"tests.rs::<fn>"`.
    fn write_spec_file(scratch: &TempScratch, content: &str) {
        fs::write(scratch.path().join("tests.rs"), content).unwrap();
    }

    #[test]
    fn proptest_in_rejects_missing_fn() {
        let scratch = TempScratch::new("missing-fn");
        write_spec_file(
            &scratch,
            "proptest! { #[test] fn other(x in 0u32..1) {} }\n",
        );
        let err =
            proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::not_there").unwrap_err();
        assert!(err.contains("not found"), "{err}");
    }

    #[test]
    fn proptest_in_rejects_fn_that_never_references_the_stem() {
        let scratch = TempScratch::new("no-stem-ref");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        let _ = other_module::parse(x);\n    }\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").unwrap_err();
        assert!(err.contains("never references"), "{err}");
    }

    #[test]
    fn proptest_in_rejects_fn_outside_its_proptest_block() {
        let scratch = TempScratch::new("outside-block");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn other(x in 0u32..1) {}\n}\n\n#[test]\nfn t() {\n    let _ = foo::parse();\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").unwrap_err();
        assert!(err.contains("proptest! block"), "{err}");
    }

    #[test]
    fn proptest_in_accepts_a_valid_reference() {
        let scratch = TempScratch::new("valid");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        let _ = foo::parse(x);\n    }\n}\n",
        );
        assert!(
            proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").is_ok(),
            "a fn inside proptest! referencing the module stem should validate"
        );
    }

    #[test]
    fn proptest_in_rejects_a_reference_inside_a_string_literal() {
        let scratch = TempScratch::new("string-literal-ref");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        let _ = \"foo::not_a_real_call(x)\";\n    }\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").unwrap_err();
        assert!(err.contains("never references"), "{err}");
    }

    #[test]
    fn proptest_in_rejects_a_reference_inside_a_comment() {
        let scratch = TempScratch::new("comment-ref");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        // foo::parse(x) is called conceptually\n        let _ = x;\n    }\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").unwrap_err();
        assert!(err.contains("never references"), "{err}");
    }

    #[test]
    fn proptest_in_rejects_ambiguous_duplicate_fn_names() {
        // A same-named fn that is NOT a top-level `#[test]` property (here,
        // buried in an unrelated `mod other`) no longer counts as a
        // candidate at all under brace-depth + `#[test]` selection — so
        // genuine ambiguity now needs TWO fns that both qualify: two
        // separate `proptest! { ... }` blocks each declaring `#[test] fn
        // t(...)`.
        let scratch = TempScratch::new("duplicate-fn");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        let _ = foo::parse(x);\n    }\n}\n\nproptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        let _ = foo::parse(x);\n    }\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").unwrap_err();
        assert!(err.contains("ambiguous"), "{err}");
    }

    #[test]
    fn proptest_in_accepts_the_real_property_despite_an_unrelated_same_named_fn() {
        let scratch = TempScratch::new("unrelated-same-name-fn");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        let _ = foo::parse(x);\n    }\n}\n\nmod other {\n    fn t() {}\n}\n",
        );
        assert!(
            proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").is_ok(),
            "the real proptest! property must still resolve when an unrelated fn happens \
             to share its name outside any proptest! block"
        );
    }

    #[test]
    fn proptest_in_rejects_a_property_nested_inside_another_test_fn() {
        let scratch = TempScratch::new("nested-property");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn outer(x in 0u32..1) {\n        fn t() { foo::parse(); }\n        let _ = t as fn();\n        prop_assert!(x < 1);\n    }\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").unwrap_err();
        assert!(err.contains("proptest! block"), "{err}");
    }

    #[test]
    fn proptest_in_rejects_a_test_attributed_fn_nested_at_depth_2() {
        // Isolates the DEPTH condition from the `#[test]` condition: this
        // nested fn carries `#[test]` too, so only the brace-depth check
        // (top of the brace stack must be the proptest! block itself, not
        // merely SOME ancestor) can reject it.
        let scratch = TempScratch::new("nested-test-attr");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn outer(x in 0u32..1) {\n        #[test]\n        fn t() { foo::parse(); }\n        prop_assert!(x < 1);\n    }\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").unwrap_err();
        assert!(err.contains("proptest! block"), "{err}");
    }

    #[test]
    fn proptest_in_rejects_an_uncalled_nested_fn_wrapping_the_reference() {
        let scratch = TempScratch::new("nested-fn");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        fn inner() { foo::parse(); }\n        let _ = inner as fn();\n        prop_assert!(x < 1);\n    }\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/source_adapters/foo.rs", "tests.rs::t")
            .unwrap_err();
        assert!(err.contains("nested fn"), "{err}");
    }

    #[test]
    fn proptest_in_rejects_a_file_that_only_imports_a_different_same_stem_module() {
        let scratch = TempScratch::new("cross-module-stem");
        write_spec_file(
            &scratch,
            "use report_diff::foo;\n\nproptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        let _ = foo::parse(x);\n    }\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/extraction/foo.rs", "tests.rs::t")
            .unwrap_err();
        assert!(err.contains("extraction::foo"), "{err}");
    }

    #[test]
    fn proptest_in_accepts_a_file_that_imports_the_real_parent_path() {
        let scratch = TempScratch::new("real-parent-import");
        write_spec_file(
            &scratch,
            "use extraction::foo;\n\nproptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        let _ = foo::parse(x);\n    }\n}\n",
        );
        assert!(
            proptest_in_is_valid(scratch.path(), "src/extraction/foo.rs", "tests.rs::t").is_ok(),
            "a call-shaped reference plus a real parent-path import should validate"
        );
    }
}
