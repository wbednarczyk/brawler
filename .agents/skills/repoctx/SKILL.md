---
name: repoctx
description: Use repoctx for fast, indexed code navigation in this repo. Beats grep/find/cat on structural questions about symbols, definitions, and surrounding source.
---

# repoctx

`repoctx` is an AI-oriented repository intelligence CLI. The index is
built incrementally over Tree-sitter parses (Go, Rust, TypeScript, TSX,
JavaScript, Python, JSON, YAML, TOML, Markdown). All read commands
auto-index on first run.

## When to use

Use this skill when the user's question is **structural**:

- "Where is X defined?"
- "What's the body of function Y?"
- "What public symbols does this file expose?"
- "Are there any TODO-tagged sections?"
- "Show me every type that mentions Foo."

Do NOT reach for `grep`/`rg`/`find`/wholesale `Read` for these. Read
files when you need prose-level reasoning about implementation; reach
for `repoctx` to navigate.

## Commands

`/usr/local/bin/repoctx` is the binary that installed this skill. All commands
default to TOON for piped reads and human for TTYs; pass `--json` when
parsing with `jq` or `serde_json`.

### `/usr/local/bin/repoctx symbols <substring>`

Case-insensitive substring search across every indexed symbol. Narrow
with `--kind` (`function`, `method`, `class`, `interface`, `type`,
`module`, `macro`, `constant`, `variable`, `field`, `section`, `key`,
`other`) and `--lang` (`rust`, `go`, `typescript`, `tsx`, `javascript`,
`python`, `json`, `yaml`, `toml`, `markdown`). `--limit` caps results
(default 50; `0` = unlimited).

Use this for **exploration** when you don't know the exact identifier.

### `/usr/local/bin/repoctx definition <name>`

Exact-name (case-sensitive) lookup. Auto-filters to definition kinds —
field/variable/section/key noise is excluded. `--lang`/`--limit` apply.
Multiple hits are normal; pick the right one. Zero hits returns exit 0
with `count: 0`.

Use this when you **know the name** and want canonical definition
sites.

### `/usr/local/bin/repoctx context <symbol> [--context C] [--limit N]`

Exact-name match + source window. Defaults: 5 lines above/below, top 3
hits. Reads source from disk so the bytes are current; `stale: true`
flags hits whose indexed `(mtime_ns, size)` no longer matches disk.

Prefer this over `definition` + a follow-up file Read — one round trip
instead of two.

### `/usr/local/bin/repoctx outline <file>`

Document-symbol tree for one file. Path may be repo-relative or
absolute. Indented containment tree (human) or flat `{count, items}`
(machine).

Use this when you need the **structure** of a file but not its full
contents.

### `/usr/local/bin/repoctx status`

File/symbol counts + per-language breakdown + staleness. `--fast` skips
the staleness walk. Quick health check before deeper work.

### `/usr/local/bin/repoctx gain`

Surface the navigation tokens this skill has actually saved. `gain top
--by saved` ranks per command.

## Output formats

| Flag | Format |
|---|---|
| (none, TTY) | Human (aligned columns) |
| (none, pipe) | TOON (token-efficient default) |
| `--json` | JSON |
| `--toon` | TOON forced on TTY |

`--json` and `--toon` are mutually exclusive.

## Empty results and errors

- An empty result set is exit 0 with `count: 0`. Check exit codes, not
  stderr strings.
- The index manages itself — read commands silently build the DB if
  needed and incrementally reindex changed files before answering.
- File arguments must be inside the repo root (`/home/wojtas/projects/brawler`).

## Gotchas

- Rust `struct`/`enum`/`union`/`type` are all reported as `class` per
  the upstream `tags.scm` mapping; Rust `trait` is `interface`.
- TypeScript upstream only tags `interface_declaration`, `abstract_class`,
  and `method_signature`. Plain `class`/`function` are untagged. Use
  TSX/JS coverage when you need plain class/function navigation.
- Markdown headings (ATX and setext) are reported as `section`.
- Top-level JSON/YAML/TOML keys are `key`; nested keys are not
  surfaced.

## Examples

```sh
# Find every type alias that mentions "Token"
/usr/local/bin/repoctx symbols Token --kind type --json | jq '.items[]'

# Where is `parse_config` defined?
/usr/local/bin/repoctx definition parse_config

# Show me the resolve_path function with 10 lines of context
/usr/local/bin/repoctx context resolve_path --context 10 --limit 1

# What's the structure of the main entry point?
/usr/local/bin/repoctx outline src/main.rs

# How healthy is the index?
/usr/local/bin/repoctx status
```
