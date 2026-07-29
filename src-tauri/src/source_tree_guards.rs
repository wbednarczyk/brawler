//! Source-tree guard tests (ADR 0045 harvests). Compiled only under
//! `cfg(test)`, never into the shipped binary.

use std::path::{Path, PathBuf};

/// Guard (#110): no `include_str!`/`include_bytes!` may embed a literal path
/// that escapes the Cargo workspace (`src-tauri/`). `cargo-mutants` copies only
/// the workspace into its build sandbox, so a cross-tree literal compiles in
/// every normal build but breaks every mutants sweep before a single mutant
/// runs. Shared fixtures under `src/test/scenarios/` must be reached via the
/// build.rs-resolved env instead:
/// `include_str!(concat!(env!("BRAWLER_SCENARIOS_DIR"), "/<file>"))`.
#[test]
fn no_include_literal_escapes_the_workspace() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();
    let mut stack = vec![manifest_dir.join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable source dir") {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let content = std::fs::read_to_string(&path).expect("readable source file");
                for literal in include_literals(&content) {
                    if escapes(&path, &literal, manifest_dir) {
                        violations.push(format!("{}: {literal:?}", path.display()));
                    }
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "literal include paths escaping src-tauri/ break the cargo-mutants \
         sandbox (#110); route shared fixtures through \
         include_str!(concat!(env!(\"BRAWLER_SCENARIOS_DIR\"), \"/<file>\")):\n{}",
        violations.join("\n")
    );
}

/// Extract the argument of every `include_str!`/`include_bytes!` whose
/// argument is a plain string literal. Macro-built paths (`concat!(env!(..))`,
/// `env!(..)`) are the sanctioned pattern and yield no literal to check.
fn include_literals(content: &str) -> Vec<String> {
    let mut found = Vec::new();
    for marker in ["include_str!", "include_bytes!"] {
        for (idx, _) in content.match_indices(marker) {
            let rest = content[idx + marker.len()..].trim_start();
            let Some(rest) = rest.strip_prefix('(') else {
                continue;
            };
            if let Some(rest) = rest.trim_start().strip_prefix('"') {
                if let Some(end) = rest.find('"') {
                    found.push(rest[..end].to_string());
                }
            }
        }
    }
    found
}

/// Lexically resolve `literal` against the including file's directory and
/// report whether the result leaves `manifest_dir`.
fn escapes(file: &Path, literal: &str, manifest_dir: &Path) -> bool {
    let mut resolved: PathBuf = file.parent().expect("source file has a parent").into();
    for component in literal.split('/') {
        match component {
            ".." => {
                resolved.pop();
            }
            "." | "" => {}
            normal => resolved.push(normal),
        }
    }
    !resolved.starts_with(manifest_dir)
}
