//! Source-tree guard tests (ADR 0045 harvests). Compiled only under
//! `cfg(test)`, never into the shipped binary.

use std::path::{Path, PathBuf};

/// Whether the current process is running inside the cargo-mutants scratch
/// sandbox, which copies only `src-tauri/` — never the sibling
/// `package.json`/`src/api`/`src/shared` frontend tree (harvest, B5/ADR 0045:
/// two cross-tree guards below used to no-op on ANY missing sibling path,
/// which also silently hid a genuinely broken real checkout). A real
/// checkout always has `../package.json` next to `src-tauri/`; only the
/// mutants sandbox does not.
pub(crate) fn is_crate_only_sandbox() -> bool {
    !Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../package.json")
        .exists()
}

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

/// Guard (G6, CLAUDE.md "a capability is not done until a user can reach
/// it"): every command registered in `tauri::generate_handler![...]`
/// (`lib.rs`) must either be called from the frontend (`src/api/**/*.ts`, a
/// quoted string literal of its name) or be declared in
/// `src/test/scenarios/headless-only.json` with a reason (MCP-only / a
/// headless acquisition driver — testing.md § Mock-runtime fidelity). A
/// registered command reachable from neither is dead surface: it compiled
/// and passed its own unit tests but no user or agent can ever invoke it.
///
/// Reads `src/api/**/*.ts` at runtime via a path relative to the workspace
/// root, which escapes `src-tauri/` — `cargo-mutants` copies only the
/// workspace into its build sandbox, so `src/api` does not exist there. Per
/// `no_runtime_cross_tree_read_escapes_the_workspace` above, cargo-mutants
/// 24.9.0 exposes no `cfg`/env flag a test can branch on to detect the
/// sandbox at runtime, so this test detects it via `is_crate_only_sandbox`
/// (B5/ADR 0045 harvest) instead of independently no-op'ing on any missing
/// directory: a real checkout always has `src/api`, so a missing one there
/// panics instead of silently degrading to a no-op unrelated to any mutant.
/// cross-tree-read-ok: reachability needs src/api; no-ops when the directory
/// is absent (the mutants sandbox).
#[test]
fn every_registered_command_is_reachable_from_the_frontend_or_declared_headless() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().expect("src-tauri has a parent");
    let api_root = repo_root.join("src/api");
    if !api_root.is_dir() {
        assert!(
            is_crate_only_sandbox(),
            "src/api is missing outside the cargo-mutants sandbox: {} — a real checkout always \
             has this directory; investigate before trusting this guard's silence",
            api_root.display()
        );
        eprintln!(
            "skipping reachability guard: {} does not exist (cargo-mutants sandbox)",
            api_root.display()
        );
        return;
    }

    let lib_rs = std::fs::read_to_string(manifest_dir.join("src/lib.rs")).expect("readable lib.rs");
    let registered = generate_handler_command_names(&lib_rs);
    assert!(
        !registered.is_empty(),
        "found zero commands in tauri::generate_handler![...] — the parser likely broke, not the app"
    );

    let headless_only_json = include_str!(concat!(
        env!("BRAWLER_SCENARIOS_DIR"),
        "/headless-only.json"
    ));
    let headless: std::collections::HashMap<String, String> =
        serde_json::from_str(headless_only_json).expect("headless-only.json is valid JSON");

    let mut api_sources = String::new();
    let mut stack = vec![api_root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| {
            panic!("readable src/api dir {}: {e}", dir.display());
        }) {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|ext| ext == "ts" || ext == "tsx")
            {
                api_sources.push_str(
                    &std::fs::read_to_string(&path).expect("readable src/api source file"),
                );
                api_sources.push('\n');
            }
        }
    }

    let unreachable: Vec<&str> = registered
        .iter()
        .map(String::as_str)
        .filter(|name| {
            let literal = format!("\"{name}\"");
            !api_sources.contains(&literal) && !headless.contains_key(*name)
        })
        .collect();
    assert!(
        unreachable.is_empty(),
        "command(s) registered in tauri::generate_handler![...] but reachable \
         from neither src/api (no \"<name>\" literal) nor declared headless-only \
         — add a frontend caller, or add an entry (with reason) to \
         src/test/scenarios/headless-only.json:\n{}",
        unreachable.join("\n")
    );

    // G6b (finding 10, ADR 0045 harvest 2026-09-08): headless-only.json is a
    // manually-maintained exemption list — a stale key (a renamed/retired
    // command) or a blank reason silently buys an exemption for nothing,
    // which the check above can never catch since it only reads the map
    // forward (does this registered name have an exemption?), never
    // backward (does this exemption name a real, still-registered command,
    // with an actual reason?).
    let registered_set: std::collections::HashSet<&str> =
        registered.iter().map(String::as_str).collect();
    let bad_entries: Vec<String> = headless
        .iter()
        .filter_map(|(key, reason)| {
            if !registered_set.contains(key.as_str()) {
                Some(format!("{key}: not a registered command (stale entry)"))
            } else if reason.trim().is_empty() {
                Some(format!("{key}: reason is blank"))
            } else {
                None
            }
        })
        .collect();
    assert!(
        bad_entries.is_empty(),
        "src/test/scenarios/headless-only.json entries must each name a live registered \
         command with a non-empty reason:\n{}",
        bad_entries.join("\n")
    );
}

/// Extract command names from `tauri::generate_handler![ commands::foo::bar,
/// ... ]`: takes the last `::`-separated segment of each entry (module path
/// is irrelevant — the wire command name is the function name Tauri exposes).
fn generate_handler_command_names(lib_rs: &str) -> Vec<String> {
    let marker = "tauri::generate_handler![";
    let start = lib_rs
        .find(marker)
        .expect("lib.rs contains tauri::generate_handler![...]")
        + marker.len();
    let rest = &lib_rs[start..];
    let end = rest.find(']').expect("generate_handler![...] is closed");
    rest[..end]
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry
                .rsplit("::")
                .next()
                .expect("split always yields at least one segment")
                .to_string()
        })
        .collect()
}

/// Guard (G3a, ADR 0045 harvest from mutation-audit run 33890327229): no
/// runtime read may leave the crate by joining `CARGO_MANIFEST_DIR` with a
/// literal `..` path segment. `cargo-mutants` copies only `src-tauri/` into
/// its scratch sandbox — the same hazard `no_include_literal_escapes_the_workspace`
/// covers for compile-time `include_str!`/`include_bytes!` literals, but this
/// form survives that guard because the escape happens at *runtime* via
/// `std::fs::read_to_string` instead of a macro literal. This is exactly what
/// broke `jobs::activity_identity::tests::job_kind_list_matches_registry`
/// reading `../src/shared/formatting/jobKinds.ts`.
///
/// A read that cannot fire during a normal `cargo test`/`cargo nextest run`
/// is not a mutants hazard, so three things exempt a match:
/// - the nearest enclosing `fn` (scanning backward for a signature line) is
///   not itself `#[test]` — a helper only ever reached from an already-
///   `#[ignore]`d caller poses no risk. (Heuristic caveat: a genuinely nested
///   `fn` between the real enclosing test and the read would be found first;
///   none of today's cross-tree reads nest this way.)
/// - the nearest enclosing `#[test]` also carries `#[ignore` — every
///   `private/realdata/` ground-truth harness in this crate is `#[ignore]`d
///   already, so it never runs under mutants (or any default `cargo test`)
///   in the first place.
/// - a `// cross-tree-read-ok: <reason>` comment on the line immediately
///   above the `CARGO_MANIFEST_DIR` use — for a `#[test]` that DOES run
///   normally but is excluded from the mutants sweep specifically, via
///   `.cargo/mutants.toml` `additional_cargo_test_args`. (Verified against
///   the pinned cargo-mutants 24.9.0: it exposes no `cfg`/env flag a test can
///   branch on to detect the sandbox at runtime — `--cfg mutants` does not
///   exist — so skipping the *test itself* via nextest's `--skip` from the
///   mutants config is the actual mechanism, not a cfg guard on the test.)
#[test]
fn no_runtime_cross_tree_read_escapes_the_workspace() {
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
                for line_no in cross_tree_read_lines(&content) {
                    violations.push(format!("{}:{line_no}", path.display()));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "runtime read(s) join CARGO_MANIFEST_DIR with a literal .. path \
         segment, escaping the cargo-mutants sandbox (it copies only \
         src-tauri/); remedies: (1) mark the enclosing #[test] \
         #[ignore = \"...\"] if it genuinely needs external data anyway, or \
         (2) exclude it from the mutants sweep via .cargo/mutants.toml \
         additional_cargo_test_args and mark the read with a \
         `// cross-tree-read-ok: <reason>` comment on the line above it:\n{}",
        violations.join("\n")
    );
}

/// 1-based line numbers in `content` where a runtime read joins
/// `CARGO_MANIFEST_DIR` with a literal `..` path segment and none of the
/// exemptions documented on the guard above apply.
///
/// Two literal shapes both escape the workspace and must both be caught
/// (finding 6, ADR 0045 harvest 2026-09-08): a bare `".."` segment (e.g.
/// `.join("..").join("src/shared/...")`) AND a single combined literal that
/// merely STARTS with `../` (e.g. `.join("../src/shared/jobKinds.ts")`) — the
/// original check matched only the former. Both are tied to an actual
/// `.join(` call (not a bare quoted-string proximity check) so an unrelated
/// `include_str!("../other-file-still-inside-src-tauri")` a few lines away
/// (a real pattern in this crate — `transform-modules.json`) is not swept in.
fn cross_tree_read_lines(content: &str) -> Vec<usize> {
    let lines: Vec<&str> = content.lines().collect();
    let mut found = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if !line.contains("CARGO_MANIFEST_DIR") {
            continue;
        }
        let window_end = (idx + 4).min(lines.len());
        if !lines[idx..window_end]
            .iter()
            .any(|l| l.contains(".join(\"..\")") || l.contains(".join(\"../"))
        {
            continue;
        }
        let commented = idx > 0
            && lines[idx - 1]
                .trim_start()
                .starts_with("// cross-tree-read-ok:");
        if commented || enclosing_fn_is_mutants_safe(&lines, idx) {
            continue;
        }
        found.push(idx + 1);
    }
    found
}

/// Finding 6 (ADR 0045 harvest 2026-09-08): both cross-tree literal shapes
/// must be caught — a bare `".."` segment and a single literal that starts
/// with `../` — proven inline so a future edit to the matcher can't silently
/// drop either shape again.
///
/// The fixture text is assembled via `format!` rather than written as a
/// plain literal: `cross_tree_read_lines` is itself part of the crate-wide
/// scan `no_runtime_cross_tree_read_escapes_the_workspace` runs, so a fixture
/// containing the literal substring "CARGO_MANIFEST_DIR" would make THIS
/// FILE flag itself as a violation.
#[test]
fn cross_tree_read_lines_catches_both_dotdot_literal_shapes() {
    // A leading `#[test]` line is required in the fixture: the enclosing-fn
    // heuristic (`enclosing_fn_is_mutants_safe`) treats a non-`#[test]` fn as
    // safe by design (it can never run under a default `cargo test`), so an
    // un-annotated fixture fn would be silently exempted rather than flagged.
    let marker = "CARGO_MANIFEST_DIR";
    let split_join = format!(
        "#[test]\nfn reads_split() {{\n    let path = Path::new(env!(\"{marker}\")).join(\"..\").join(\"x\");\n}}\n"
    );
    assert!(
        !cross_tree_read_lines(&split_join).is_empty(),
        "the split `.join(\"..\")` form must be caught"
    );

    let combined_join = format!(
        "#[test]\nfn reads_combined() {{\n    let path = Path::new(env!(\"{marker}\")).join(\"../x\");\n}}\n"
    );
    assert!(
        !cross_tree_read_lines(&combined_join).is_empty(),
        "the combined \"../...\" literal form must be caught"
    );
}

/// Whether the innermost `fn` enclosing line `idx` (scanning backward for a
/// signature line) can never run in a default `cargo test`/`cargo nextest
/// run`: either it is not itself `#[test]`, or it is `#[test]` with a
/// sibling `#[ignore` attribute.
fn enclosing_fn_is_mutants_safe(lines: &[&str], idx: usize) -> bool {
    let Some(fn_line) = (0..idx).rev().find(|&i| is_fn_signature(lines[i])) else {
        return false;
    };
    let attrs = &lines[fn_line.saturating_sub(6)..fn_line];
    let is_test = attrs.iter().any(|l| l.trim_start().starts_with("#[test]"));
    let is_ignored = attrs.iter().any(|l| l.trim_start().starts_with("#[ignore"));
    !is_test || is_ignored
}

/// Whether `line` is a function signature, after stripping the modifier
/// keywords that can precede `fn` (`pub fn`, `pub(crate) async fn`, ...).
fn is_fn_signature(line: &str) -> bool {
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
    rest.starts_with("fn ")
}

/// Guard (G7, ADR 0049 "data transform correctness"): `transform-modules.json`
/// is the manifest of Brawler's data-transform modules (dedup, normalization,
/// entity matching, classification, merge) and whether each one carries the
/// `proptest` invariants / `insta` golden snapshot ADR 0049 calls for. This
/// guard has two halves:
/// - **Truth**: every listed `path` exists, and a module claiming
///   `proptest: true`/`insta: true` actually contains a `proptest!`/`insta::`
///   usage — the manifest cannot silently drift from reality in either
///   direction (a claimed test that got deleted, or a ratchet flip that never
///   happened).
/// - **Discovery**: every module directory/file directly under
///   `src/fundamentals/extraction/` or `src/source_adapters/` that defines a
///   `parse*`/`normalize*`/`resolve*`/`dedup*`/`match*` fn — the naming
///   convention this codebase's real transforms already follow — must be
///   LISTED in the manifest (with `proptest`/`insta` truthfully `false` if
///   neither exists yet). A new transform lands with a manifest entry in the
///   same change, even if its properties/golden are follow-up work.
///
/// Both roots have `snapshots/` subdirectories (insta fixtures, not code),
/// excluded from discovery.
#[test]
fn transform_modules_carry_their_property_and_golden_tests() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_json = include_str!("../transform-modules.json");
    let manifest: serde_json::Value =
        serde_json::from_str(manifest_json).expect("transform-modules.json is valid JSON");
    let modules = manifest["modules"]
        .as_array()
        .expect("transform-modules.json has a top-level \"modules\" array");

    let mut violations = Vec::new();
    let mut listed_paths = std::collections::HashSet::new();
    for module in modules {
        let path = module["path"]
            .as_str()
            .expect("each module entry has a string \"path\"");
        listed_paths.insert(path.to_string());
        let full = manifest_dir.join(path);
        if !full.exists() {
            violations.push(format!("{path}: listed module path does not exist"));
            continue;
        }
        let content = read_rs_module_content(&full);
        let claims_proptest = module["proptest"]
            .as_bool()
            .expect("each module entry has a boolean \"proptest\"");
        let claims_insta = module["insta"]
            .as_bool()
            .expect("each module entry has a boolean \"insta\"");
        if claims_proptest && !(content.contains("proptest!") || content.contains("proptest::")) {
            violations.push(format!(
                "{path}: manifest claims proptest:true but no proptest!/proptest:: usage found \
                 (the ratchet flips false→true only, fix the claim or add the test)"
            ));
        }
        if claims_insta && !content.contains("insta::") {
            violations.push(format!(
                "{path}: manifest claims insta:true but no insta:: usage found \
                 (the ratchet flips false→true only, fix the claim or add the snapshot)"
            ));
        }
    }

    // Ratchet floors (review 2026-09-08, B1): the manifest may only gain
    // property/golden coverage. Counts may only rise, and a module may carry
    // `proptest: false` only if it was already an offender when the manifest
    // was frozen — a NEW transform must ship with proptest:true.
    const PROPTEST_TRUE_FLOOR: usize = 11;
    const INSTA_TRUE_FLOOR: usize = 13;
    const FROZEN_NO_PROPTEST: &[&str] = &[
        "src/fundamentals/extraction/esef.rs",
        "src/fundamentals/extraction/esef_package.rs",
        "src/fundamentals/extraction/html.rs",
        "src/fundamentals/extraction/mod.rs",
        "src/fundamentals/extraction/text_numbers.rs",
        "src/fundamentals/extraction/pipeline",
        "src/source_adapters/bankier_calendar.rs",
        "src/source_adapters/bankier_company.rs",
        "src/source_adapters/bankier_rss.rs",
        "src/source_adapters/biznesradar_fundamentals.rs",
        "src/source_adapters/biznesradar_ownership.rs",
        "src/source_adapters/biznesradar_recommendations.rs",
        "src/source_adapters/company_directory.rs",
        "src/source_adapters/gpw_company_registry.rs",
        "src/source_adapters/gpw_market_events.rs",
        "src/source_adapters/knf_short_selling.rs",
        "src/source_adapters/newconnect_company_directory.rs",
        "src/storage/ingestion.rs",
        "src/report_documents_capture.rs",
    ];
    let proptest_true = modules
        .iter()
        .filter(|m| m["proptest"].as_bool() == Some(true))
        .count();
    let insta_true = modules
        .iter()
        .filter(|m| m["insta"].as_bool() == Some(true))
        .count();
    if proptest_true < PROPTEST_TRUE_FLOOR {
        violations.push(format!(
            "manifest proptest:true count {proptest_true} fell below the floor {PROPTEST_TRUE_FLOOR} — coverage may only rise"
        ));
    }
    if insta_true < INSTA_TRUE_FLOOR {
        violations.push(format!(
            "manifest insta:true count {insta_true} fell below the floor {INSTA_TRUE_FLOOR} — coverage may only rise"
        ));
    }
    for frozen in FROZEN_NO_PROPTEST {
        let has_proptest = modules
            .iter()
            .any(|m| m["path"].as_str() == Some(frozen) && m["proptest"].as_bool() == Some(true));
        if has_proptest {
            violations.push(format!(
                "{frozen}: now carries proptest:true — delete it from FROZEN_NO_PROPTEST so it can never flip back (per-module ratchet)"
            ));
        }
    }
    for module in modules {
        let path = module["path"].as_str().expect("path");
        if module["proptest"].as_bool() == Some(false) && !FROZEN_NO_PROPTEST.contains(&path) {
            violations.push(format!(
                "{path}: proptest:false is only allowed for the frozen pre-existing offenders — a new transform ships with its properties (ADR 0049)"
            ));
        }
    }

    for root in ["src/fundamentals/extraction", "src/source_adapters"] {
        let root_path = manifest_dir.join(root);
        for entry in std::fs::read_dir(&root_path).expect("readable discovery root") {
            let entry_path = entry.expect("readable dir entry").path();
            let name = entry_path
                .file_name()
                .expect("dir entry has a file name")
                .to_string_lossy()
                .into_owned();
            if name == "snapshots" {
                continue;
            }
            let is_module_unit =
                entry_path.is_dir() || entry_path.extension().is_some_and(|ext| ext == "rs");
            if !is_module_unit {
                continue;
            }
            let content = read_rs_module_content(&entry_path);
            if !contains_transform_shaped_fn(&content) {
                continue;
            }
            let rel_path = format!("{root}/{name}");
            if !listed_paths.contains(&rel_path) {
                violations.push(format!(
                    "{rel_path}: defines a parse*/normalize*/resolve*/dedup*/match* fn but is \
                     not declared in transform-modules.json"
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "transform-modules.json manifest drift (ADR 0049 / G7):\n{}",
        violations.join("\n")
    );
}

/// Concatenate every `.rs` file's content under `path` (or read it directly
/// if `path` is itself a file), skipping `snapshots/` subdirectories.
fn read_rs_module_content(path: &Path) -> String {
    if path.is_file() {
        return std::fs::read_to_string(path).unwrap_or_default();
    }
    let mut content = String::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let entry_path = entry.expect("readable dir entry").path();
            if entry_path.is_dir() {
                if entry_path.file_name().is_some_and(|n| n == "snapshots") {
                    continue;
                }
                stack.push(entry_path);
            } else if entry_path.extension().is_some_and(|ext| ext == "rs") {
                content.push_str(&std::fs::read_to_string(&entry_path).unwrap_or_default());
                content.push('\n');
            }
        }
    }
    content
}

/// Whether `content` defines a fn whose name starts with `parse`, `normalize`,
/// `resolve`, `dedup`, or `match` — the naming convention Brawler's real data
/// transforms already follow (adapter parsers, normalization, dedup keys,
/// company/entity matching).
fn contains_transform_shaped_fn(content: &str) -> bool {
    const PREFIXES: [&str; 5] = ["parse", "normalize", "resolve", "dedup", "match"];
    for (idx, _) in content.match_indices("fn ") {
        let name: String = content[idx + 3..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
            return true;
        }
    }
    false
}

/// Guard (G10): a `#[test]`/`#[tokio::test]` fn that asserts nothing is worse
/// than no test — it stays green forever regardless of what the code does,
/// silently certifying nothing. A fn is exempt when it: carries
/// `#[should_panic]` (the panic itself is the assertion); is named
/// `*_never_panics`/`*_does_not_panic` (same reasoning, spelled in the name);
/// or carries a `// no-assert-ok: <reason>` comment directly above (or trailing) its attribute
/// block (e.g. a smoke test whose only job is "this compiles and runs
/// without an early return/panic"). Otherwise the body must contain one of
/// `assert`/`expect(`/`unwrap_err`/`is_err()`/`is_ok()`/`insta::`/
/// `prop_assert`/`panic!`.
///
/// Heuristic, not a parser: body extraction brace-matches from the first `{`
/// after the fn signature, skipping quoted/raw-string content so a hand-built
/// JSON fixture's stray `{`/`}` bytes don't mis-scope the body (this bit a
/// real test — `providers::common::tests::extracts_balanced_json_object_from_fenced_response_with_trailing_text`
/// — before the skip was added). Single-quoted char literals and comments are
/// NOT excluded, so a `'{'`/`'}'` char literal or a brace inside a comment
/// could still mis-scope a body; none of today's offenders hit that. A false
/// positive here is a `// no-assert-ok:` away from going green.
#[test]
fn every_test_asserts_something() {
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
                violations.extend(assertion_free_test_violations(&path, &content));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "test(s) with no detectable assertion (G10) — add an assertion, rename \
         to *_never_panics/*_does_not_panic if that IS the test, or add \
         `// no-assert-ok: <reason>` above the #[test]:\n{}",
        violations.join("\n")
    );
}

/// Scan one file's `#[test]`/`#[tokio::test]` functions for a missing
/// assertion; see the guard above for the exemptions.
fn assertion_free_test_violations(path: &Path, content: &str) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut violations = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        let trimmed = lines[idx].trim_start();
        if trimmed != "#[test]" && !trimmed.starts_with("#[tokio::test") {
            idx += 1;
            continue;
        }
        let mut has_should_panic = trimmed.starts_with("#[should_panic");

        // Attributes can stack above OR below `#[test]` (`#[should_panic]`
        // commonly precedes it) — walk both directions over contiguous `#[`
        // lines to find the block's true top and collect every marker.
        let mut block_start = idx;
        while block_start > 0 && lines[block_start - 1].trim_start().starts_with("#[") {
            block_start -= 1;
            if lines[block_start]
                .trim_start()
                .starts_with("#[should_panic")
            {
                has_should_panic = true;
            }
        }
        let mut fn_line = idx;
        while fn_line < lines.len() && lines[fn_line].trim_start().starts_with("#[") {
            if lines[fn_line].trim_start().starts_with("#[should_panic") {
                has_should_panic = true;
            }
            fn_line += 1;
        }
        if fn_line >= lines.len() || !is_fn_signature(lines[fn_line].trim_start()) {
            idx += 1;
            continue;
        }

        let name = extract_test_fn_name(lines[fn_line]);
        let name_exempt = name.ends_with("_never_panics") || name.ends_with("_does_not_panic");
        let comment_exempt = (block_start > 0
            && lines[block_start - 1]
                .trim_start()
                .starts_with("// no-assert-ok:"))
            || lines[block_start..=fn_line]
                .iter()
                .any(|line| line.contains("// no-assert-ok:"));

        if !has_should_panic && !name_exempt && !comment_exempt {
            let body = extract_fn_body(content, &lines, fn_line);
            if !body_has_assertion(&body) {
                violations.push(format!("{}:{} fn {name}", path.display(), fn_line + 1));
            }
        }
        idx = fn_line + 1;
    }
    violations
}

/// Extract the fn name from its signature line, after stripping the same
/// modifier keywords `is_fn_signature` strips.
fn extract_test_fn_name(line: &str) -> String {
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
    let rest = rest.strip_prefix("fn ").unwrap_or(rest).trim_start();
    rest.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Brace-match the body of the fn whose signature starts at `lines[fn_line]`,
/// from the first `{` at or after that line through its matching `}`.
fn extract_fn_body(content: &str, lines: &[&str], fn_line: usize) -> String {
    let start_offset: usize = lines[..fn_line].iter().map(|l| l.len() + 1).sum();
    let rest = &content[start_offset..];
    let bytes = rest.as_bytes();
    // Single pass, skipping string/raw-string content throughout — not just
    // once depth-tracking starts. A proptest signature commonly embeds a
    // regex string with a `{n,m}` quantifier (e.g. `".{0,200}"`) BEFORE the
    // real body brace; naively `find`ing the first `{` in the whole slice
    // would lock onto that literal instead of the body, undercounting from
    // the wrong starting point (bit `mcp::protocol::tests::
    // dispatcher_never_panics_on_arbitrary_json` before this was unified).
    let mut i = 0;
    let mut depth = 0usize;
    let mut body_start = None;
    while i < bytes.len() {
        match bytes[i] {
            // A quoted string can contain unbalanced `{`/`}` bytes (a
            // regex quantifier, a hand-built JSON fixture) — skip its
            // content, honoring `\"`/`\\` escapes.
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            // A raw string `r"..."` / `r#"..."#` / `r##"..."##` has no escapes
            // — its content ends only at a `"` followed by the same hash count.
            b'r' if matches!(bytes.get(i + 1), Some(b'"') | Some(b'#')) => {
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
                    i = j;
                    continue;
                }
            }
            b'{' => {
                if body_start.is_none() {
                    body_start = Some(i + 1);
                }
                depth += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                if let Some(start) = body_start {
                    if depth == 0 {
                        return rest[start..i].to_string();
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    body_start
        .map(|start| rest[start..].to_string())
        .unwrap_or_default()
}

/// Whether a test body contains one of the accepted assertion markers.
///
/// `expect(` and `is_ok()` were dropped (B9/ADR 0045 harvest, 2026-09-08):
/// both accept ANY body that merely calls `.expect(...)`/`.is_ok()` on a
/// setup value without checking the value under test — e.g. `state.create_x
/// (..).expect("created")` proves nothing about `create_x`'s behavior, so a
/// test built entirely of such calls passed this guard while asserting
/// nothing.
fn body_has_assertion(body: &str) -> bool {
    [
        "assert",
        "unwrap_err",
        "is_err()",
        "insta::",
        "prop_assert",
        "panic!",
    ]
    .iter()
    .any(|marker| body.contains(marker))
}

/// Guard (G11, finding 10, ADR 0045 harvest 2026-09-08): a `// no-assert-ok:`
/// or `// cross-tree-read-ok:` comment with a BLANK reason defeats the whole
/// point of the escape hatch — it silences a guard with nothing left for a
/// reviewer to check, exactly the silent-degradation failure mode this
/// harvest closes elsewhere (B5, G6b). Every such comment anywhere in the
/// crate must carry non-empty text after the marker.
#[test]
fn escape_hatch_reasons_are_non_empty() {
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
                for (line_no, line) in content.lines().enumerate() {
                    for marker in ["no-assert-ok:", "cross-tree-read-ok:"] {
                        let Some(pos) = line.find(marker) else {
                            continue;
                        };
                        let reason = line[pos + marker.len()..].trim();
                        if reason.is_empty() {
                            violations.push(format!(
                                "{}:{} blank reason after {marker}",
                                path.display(),
                                line_no + 1
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "escape-hatch comment(s) with a blank reason (G11) — a reason with nothing to check \
         defeats the point of the escape hatch; add a real one:\n{}",
        violations.join("\n")
    );
}
