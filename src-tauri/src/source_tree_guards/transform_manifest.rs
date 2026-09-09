//! Transform-manifest guard helpers (G7, ADR 0049, issue #194 S0): resolving
//! a file module's declared test submodule into its content, and validating
//! an out-of-file `"proptest_in": "<file>::<fn>"` cross-reference. Kept out
//! of `mod.rs` to stay under the file-size ratchet threshold (ADR 0103) —
//! same reasoning as `scan.rs`.

use std::path::Path;

use super::extract_fn_body;
use super::scan::strip_comments_and_strings;

/// The content of every module `path` (a `.rs` file) declares at top level
/// with `mod x;` (any visibility) — a preceding `#[path = "P"]` attribute
/// resolves relative to `path`'s directory; otherwise the target is
/// `<dir>/<stem>/x.rs`, falling back to `<dir>/<stem>/x/mod.rs`. One level
/// deep: this is how a file module's declared test submodule (`esef.rs`'s
/// `mod tests;` -> `esef/tests.rs`; `report_documents_capture.rs`'s
/// `#[path = "report_documents_capture_tests.rs"] mod tests;`) counts toward
/// the manifest guard's `proptest!`/`insta::` search without the guard
/// having to know every file's test-module layout by hand. Only a **column-0**
/// declaration counts — an indented `mod tests;` is a nested item, not a
/// file-level one — and comments/strings are blanked before matching (via
/// `strip_comments_and_strings`) so a `mod tests;` sitting inside `/* … */`
/// or a multi-line string fixture is never mistaken for a real declaration.
/// The `#[path = "…"]` filename itself is read back from the *unstripped*
/// line, since the stripper blanks its own string value too.
pub(super) fn declared_test_module_content(path: &Path, content: &str) -> String {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let mut extra = String::new();
    let mut pending_path_attr: Option<&str> = None;
    // Comments and string literals are blanked (length/line-preserving)
    // before matching, and — unlike the old `line.trim_start()` — the
    // prefix checks below run against the line's own indentation, so only a
    // column-0 (top-level) `mod`/`#[path]` counts: an indented one is a
    // nested item, and a commented-out or string-embedded one collapses to
    // blank spaces that match no prefix either.
    let stripped = strip_comments_and_strings(content);
    for (line, trimmed) in content.lines().zip(stripped.lines()) {
        if trimmed.starts_with("#[path") {
            pending_path_attr = line
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

/// Validates a manifest row's `"proptest_in": "<file>::<fn>"` claim (the
/// property test lives elsewhere, e.g. `tests/parser_fuzz.rs`, rather than in
/// the module's own file/declared test submodule): `<file>` (relative to
/// `manifest_dir`, i.e. `src-tauri/`) exists and declares `fn <name>(`
/// exactly once (two-or-more `fn <name>(` definitions in the file is an
/// "ambiguous property name" violation rather than silently picking the
/// first); that fn sits inside a `proptest! { ... }` block — the nearest
/// preceding line whose trimmed text starts with `proptest!`, whose
/// same-indentation closing `}` comes after the fn; and the fn's own
/// brace-matched body (via `extract_fn_body`) makes a genuine call/path
/// reference to `<stem>::` (`body_references_stem`), where `<stem>` is
/// `module_path`'s (`module_stem`). Fn discovery and the `proptest!` block
/// search both run over `strip_comments_and_strings`-blanked lines, so a
/// commented-out `fn`/`proptest!` line is never matched. Any failure is a
/// plain reason string — the caller names the manifest row.
///
/// Note the inherent limit this leaves: `<stem>::` cannot distinguish two
/// different modules that happen to share a file stem (`foo/bar.rs` and
/// `baz/bar.rs` both stem to `bar`) — the manifest lists the full path per
/// row, so a reviewer can catch a cross-module false-positive by eye even
/// though this helper cannot.
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
    let stripped_lines: Vec<&str> = stripped.lines().collect();
    let target = format!("{fn_name}(");
    let is_target_fn = |line: &str| {
        strip_fn_modifiers(line)
            .strip_prefix("fn ")
            .is_some_and(|after| after.starts_with(&target))
    };
    let matches: Vec<usize> = stripped_lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| is_target_fn(line).then_some(index))
        .collect();
    if matches.len() > 1 {
        return Err(format!(
            "ambiguous property name: fn {fn_name} is defined {} times in {file_rel}",
            matches.len()
        ));
    }
    let fn_line = *matches
        .first()
        .ok_or_else(|| format!("fn {fn_name} not found in {file_rel}"))?;

    let proptest_line = stripped_lines[..fn_line]
        .iter()
        .rposition(|line| line.trim_start().starts_with("proptest!"))
        .ok_or_else(|| format!("fn {fn_name} is not inside a proptest! block"))?;
    let block_indent = lines[proptest_line].len() - lines[proptest_line].trim_start().len();
    let closing_line = stripped_lines[proptest_line + 1..]
        .iter()
        .position(|line| {
            let indent = line.len() - line.trim_start().len();
            indent == block_indent && line.trim() == "}"
        })
        .map(|rel| proptest_line + 1 + rel);
    match closing_line {
        Some(c) if c > fn_line => {}
        _ => {
            return Err(format!(
                "fn {fn_name} is not inside its nearest proptest! block"
            ))
        }
    }

    let body = extract_fn_body(&content, &lines, fn_line);
    let stem = module_stem(module_path);
    if !body_references_stem(&body, stem) {
        return Err(format!("fn {fn_name} body never references {stem}::"));
    }
    Ok(())
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

/// `line` with its leading modifier keywords (`pub`, `async`, ...) stripped,
/// same rule `is_fn_signature` checks against — shared here so the fn-name
/// match below does not re-derive it.
fn strip_fn_modifiers(line: &str) -> &str {
    let mut rest = line.trim_start();
    loop {
        let prefixes = [
            "pub(crate) ",
            "pub(super) ",
            "pub ",
            "async ",
            "unsafe ",
            "const ",
        ];
        match prefixes.iter().find_map(|p| rest.strip_prefix(p)) {
            Some(stripped) => rest = stripped,
            None => break,
        }
    }
    rest
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
        let scratch = TempScratch::new("duplicate-fn");
        write_spec_file(
            &scratch,
            "proptest! {\n    #[test]\n    fn t(x in 0u32..1) {\n        let _ = foo::parse(x);\n    }\n}\n\nmod other {\n    fn t() {}\n}\n",
        );
        let err = proptest_in_is_valid(scratch.path(), "src/foo.rs", "tests.rs::t").unwrap_err();
        assert!(err.contains("ambiguous"), "{err}");
    }
}
