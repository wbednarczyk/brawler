//! Guard (G9, hard gates wave 2): command-shape guard over
//! `src/commands/**` — every Tauri-command-attributed fn taking a
//! `tauri::State<'_, AppState>` param (any qualification: `tauri::State`,
//! bare `State`; `AppState` or `app_state::AppState`) touches shared storage
//! and must not block the Tauri main thread (CLAUDE.md "keep non-trivial
//! work off the UI thread", DoD §C).
//!
//! Signature-based, not body-based: whether a command's async body actually
//! blocks is review territory, not this scan — the two checks below are
//! about SHAPE (`async` + offload marker present; the identity of which
//! commands are sync at all, frozen).

use std::path::Path;

// Built from split literals so this file never contains the contiguous attribute
// text: docs-drift scans every `.rs` line for it and would read the synthetic
// snippets below as real commands missing from contracts.md.
const CMD_ATTR: &str = concat!("#[tauri::", "command]");
const CMD_ATTR_OPEN: &str = concat!("#[tauri::", "command(");

use super::scan::{is_test_file, strip_comments_and_strings};
use super::{extract_fn_body, extract_test_fn_name, is_fn_signature, source_files};

/// One Tauri-command-attributed fn taking a `State<'_, ...AppState>` param.
struct StateCommand {
    /// `"commands/<file>.rs::<fn>"`, relative to `src-tauri/src/`.
    id: String,
    is_async: bool,
    body: String,
}

/// Whether `line`, after stripping the same modifier keywords
/// `is_fn_signature` strips, starts with `async fn `.
fn is_async_signature(line: &str) -> bool {
    let mut rest = line.trim_start();
    loop {
        let prefixes = ["pub(crate) ", "pub(super) ", "pub ", "unsafe ", "const "];
        match prefixes.iter().find_map(|p| rest.strip_prefix(p)) {
            Some(stripped) => rest = stripped,
            None => break,
        }
    }
    rest.starts_with("async fn ")
}

/// Every Tauri-command-attributed fn (plain or `(...)` form) taking a
/// `State<'_, ...AppState>` param in `content` (one file's source, already
/// known non-test — `rel` labels the resulting ids `"<rel>::<fn>"`).
/// Standalone from disk I/O so synthetic snippets can exercise it directly.
fn scan_state_commands_in(rel: &str, content: &str) -> Vec<StateCommand> {
    let code_only = strip_comments_and_strings(content);
    let lines: Vec<&str> = content.lines().collect();
    let mut line_offsets = Vec::with_capacity(lines.len());
    let mut acc = 0usize;
    for line in &lines {
        line_offsets.push(acc);
        acc += line.len() + 1;
    }

    let mut found = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        let trimmed = lines[idx].trim_start();
        let is_command_attr = trimmed == CMD_ATTR || trimmed.starts_with(CMD_ATTR_OPEN);
        if !is_command_attr {
            idx += 1;
            continue;
        }
        let attr_is_async = trimmed.contains("async");

        // Attribute/doc-comment lines may stack between the command attribute
        // and the fn signature (`#[allow(...)]`, `/// docs`).
        let mut fn_line = idx + 1;
        while fn_line < lines.len() {
            let t = lines[fn_line].trim_start();
            if t.starts_with("#[") || t.starts_with("///") || t.starts_with("//!") {
                fn_line += 1;
            } else {
                break;
            }
        }
        if fn_line >= lines.len() || !is_fn_signature(lines[fn_line]) {
            idx += 1;
            continue;
        }

        let sig_start = line_offsets[fn_line];
        let Some(brace_open) = code_only[sig_start..].find('{').map(|rel| sig_start + rel) else {
            idx = fn_line + 1;
            continue;
        };
        let signature_text = &content[sig_start..brace_open];
        let has_state_param =
            signature_text.contains("State<") && signature_text.contains("AppState");
        if has_state_param {
            let name = extract_test_fn_name(lines[fn_line]);
            let body = extract_fn_body(content, &lines, fn_line);
            found.push(StateCommand {
                id: format!("{rel}::{name}"),
                is_async: attr_is_async || is_async_signature(lines[fn_line]),
                body,
            });
        }
        idx = fn_line + 1;
    }
    found
}

/// Every Tauri-command-attributed fn (plain or `(...)` form) taking a
/// `State<'_, ...AppState>` param under `src/commands/**` (test files
/// excluded by the 4-clause predicate — commands are production surface
/// only, so no inline test-span handling is needed here).
fn scan_state_commands() -> Vec<StateCommand> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let commands_root = src_dir.join("commands");
    let mut found = Vec::new();
    for path in source_files(&commands_root) {
        let rel = path
            .strip_prefix(&src_dir)
            .expect("under src")
            .to_string_lossy()
            .replace('\\', "/");
        if is_test_file(&rel) {
            continue;
        }
        let content = std::fs::read_to_string(&path).expect("readable source file");
        found.extend(scan_state_commands_in(&rel, &content));
    }
    found
}

/// Guard (G9a): every `async` State command offloads its work — `#[tauri::
/// command]` runs sync commands ON the main thread and async commands on the
/// async runtime, but an async command whose body still does the real work
/// inline (no `spawn_blocking`/`run_blocking_task`) blocks that runtime's
/// worker just the same. Invariant, empty allowlist by design.
#[test]
fn async_state_commands_offload_their_work() {
    let commands = scan_state_commands();
    let violations: Vec<&str> = commands
        .iter()
        .filter(|c| c.is_async)
        .filter(|c| !c.body.contains("spawn_blocking(") && !c.body.contains("run_blocking_task("))
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        violations.is_empty(),
        "async State command(s) with no spawn_blocking(/run_blocking_task( in their body — \
         Tauri's async runtime still blocks its worker thread on inline work (CLAUDE.md \
         \"keep non-trivial work off the UI thread\", DoD \u{a7}C):\n{}",
        violations.join("\n")
    );
}

const SYNC_COMMANDS_BASELINE_JSON: &str = include_str!("../../sync-commands-baseline.json");

/// Guard (G9b): the frozen identity list of today's SYNC State commands.
/// Sync commands run ON the Tauri main thread by construction — the list may
/// only shrink (a sync command converted to async + offloaded drops off it;
/// nothing new may join it, it must ship async from day one).
#[test]
fn sync_state_commands_are_pinned() {
    let baseline: serde_json::Value = serde_json::from_str(SYNC_COMMANDS_BASELINE_JSON)
        .expect("sync-commands-baseline.json is valid JSON");
    let mut baseline_commands: Vec<String> = baseline["commands"]
        .as_array()
        .expect("sync-commands-baseline.json has a top-level \"commands\" array")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("each baseline entry is a string")
                .to_owned()
        })
        .collect();
    baseline_commands.sort();

    let commands = scan_state_commands();
    let mut sync_commands: Vec<String> = commands
        .iter()
        .filter(|c| !c.is_async)
        .map(|c| c.id.clone())
        .collect();
    sync_commands.sort();
    sync_commands.dedup();

    let mut violations = Vec::new();
    for id in &sync_commands {
        if !baseline_commands.iter().any(|b| b == id) {
            violations.push(format!(
                "{id}: sync State command not in sync-commands-baseline.json — make it \
                 `async fn` and offload via spawn_blocking/run_blocking_task (Tauri runs sync \
                 commands on the main thread) — DoD \u{a7}C"
            ));
        }
    }
    for id in &baseline_commands {
        if !sync_commands.iter().any(|s| s == id) {
            violations.push(format!(
                "{id}: pinned in sync-commands-baseline.json but no longer a sync State command \
                 (converted to async, or removed) — drop it from the baseline (the pin may only \
                 shrink)"
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "sync State command identity guard (G9):\n{}",
        violations.join("\n")
    );
}

#[cfg(test)]
mod predicate_tests {
    use super::{
        is_async_signature, scan_state_commands, scan_state_commands_in, CMD_ATTR, CMD_ATTR_OPEN,
    };

    #[test]
    fn is_async_signature_recognizes_modifier_stacking() {
        assert!(is_async_signature("pub(crate) async fn foo("));
        assert!(!is_async_signature("pub fn foo("));
    }

    #[test]
    fn sync_state_command_is_detected() {
        let source = format!(
            "{CMD_ATTR}\n{}",
            "\
pub fn list_things(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(vec![])
}
"
        );
        let found = scan_state_commands_in("commands/x.rs", &source);
        assert_eq!(found.len(), 1);
        assert!(!found[0].is_async);
        assert_eq!(found[0].id, "commands/x.rs::list_things");
    }

    #[test]
    fn async_state_command_with_spawn_blocking_is_ok() {
        let source = format!(
            "{CMD_ATTR}\n{}",
            "\
pub async fn list_things(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(vec![])).await.unwrap()
}
"
        );
        let found = scan_state_commands_in("commands/x.rs", &source);
        assert_eq!(found.len(), 1);
        assert!(found[0].is_async);
        assert!(found[0].body.contains("spawn_blocking("));
    }

    #[test]
    fn async_state_command_without_offload_is_flagged_by_the_body_check() {
        let source = format!(
            "{CMD_ATTR}\n{}",
            "\
pub async fn list_things(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state.list())
}
"
        );
        let found = scan_state_commands_in("commands/x.rs", &source);
        assert_eq!(found.len(), 1);
        assert!(found[0].is_async);
        assert!(
            !found[0].body.contains("spawn_blocking(")
                && !found[0].body.contains("run_blocking_task(")
        );
    }

    #[test]
    fn tauri_command_async_attribute_counts_as_async() {
        let source = format!(
            "{CMD_ATTR_OPEN}async)]\n{}",
            "\
pub fn list_things(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(vec![])
}
"
        );
        let found = scan_state_commands_in("commands/x.rs", &source);
        assert_eq!(found.len(), 1);
        assert!(
            found[0].is_async,
            "the `command(async)` attribute form must count as async even on a sync fn signature"
        );
    }

    #[test]
    fn command_without_state_param_is_ignored() {
        let source = format!(
            "{CMD_ATTR}\n{}",
            "\
pub fn health() -> HealthResponse {
    HealthResponse::default()
}
"
        );
        let found = scan_state_commands_in("commands/x.rs", &source);
        assert!(found.is_empty());
    }

    /// End-to-end smoke over the real `src/commands` tree: at least one
    /// State command is found (the scanner did not silently break), and
    /// every found command's id looks like `commands/<file>.rs::<fn>`.
    #[test]
    fn scan_finds_state_commands_shaped_as_expected() {
        let commands = scan_state_commands();
        assert!(!commands.is_empty(), "expected at least one State command");
        assert!(commands
            .iter()
            .all(|c| c.id.starts_with("commands/") && c.id.contains("::")));
    }
}
