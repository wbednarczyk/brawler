---
name: repoctx
description: Use repoctx for fast, indexed code navigation in this repo. Beats grep/find/cat on structural questions about symbols, definitions, the call graph (who-calls / callees / callgraph), and surrounding source.
---

# repoctx

`repoctx` is an AI-oriented repository intelligence CLI. The index is
built incrementally over Tree-sitter parses of 20 languages (Rust, Go,
Python, TypeScript, TSX, JavaScript, C, C++, Java, C#, Ruby, PHP, Lua,
Kotlin, Swift, Bash, Markdown, JSON, YAML, TOML). All read commands
auto-index on first run.

## When to use

Use this skill when the user's question is **structural**:

- "Where is X defined?"
- "What's the body of function Y?"
- "What public symbols does this file expose?"
- "Who calls function Z?" / "What does Z call?"
- "Trace the call chain from Z."
- "What does this file import?" / "What imports module M?"
- "Does layer A import layer B?" (boundary / layering checks)
- "Are there any TODO-tagged sections?"
- "Show me every type that mentions Foo."

Do NOT reach for `grep`/`rg`/`find`/wholesale `Read` for these. Read
files when you need prose-level reasoning about implementation; reach
for `repoctx` to navigate.

## Commands

`repoctx` is the binary that installed this skill. All commands
default to TOON for piped reads and human for TTYs; pass `--json` when
parsing with `jq` or `serde_json`.

### `repoctx symbols <substring>`

Case-insensitive substring search across every indexed symbol. Narrow
with `--kind` (`function`, `method`, `class`, `interface`, `type`,
`module`, `macro`, `constant`, `variable`, `field`, `section`, `key`,
`other`) and `--lang` (`rust`, `go`, `typescript`, `tsx`, `javascript`,
`python`, `json`, `yaml`, `toml`, `markdown`). `--limit` caps results
(default 50; `0` = unlimited).

Use this for **exploration** when you don't know the exact identifier.

### `repoctx search <pattern>`

Textually-complete search with **provenance**. One flat `results` stream;
every item carries a `source` tag telling you how much to trust it:

- `structural` — tree-sitter confirmed a symbol here (name/kind/range known).
  **Trust this.** Each structural item carries its own `callers` + `callees`.
- `reference` — a call site of the queried name (from the call graph).
- `textual` — substring matched but unconfirmed (comment, string, or a call
  to a *different* symbol). Treat like grep.

So you can separate confirmed symbols from noise, and see **who calls the
symbol and what it calls** without a second query — the thing `rg` can't do.
`callers`/`callees` are grouped by resolution within the indexed scope:
`internal` (one indexed def — expanded with location, the signal),
`ambiguous` (several indexed defs — per-name `{name,count}`), and
`external_count` (calls to stdlib/third-party/uncovered code — a count, so
`format`/`Some`/`Ok` noise doesn't bury the internal calls). `--all-callees`
expands the collapsed `external` names + ambiguous `candidates`.

`--lang` restricts by language. This is what `rg <ident>` rewrites to, so you
usually get it automatically. Output:
`{pattern, results:[{source, path, line, name?, kind?, end_line?, text?, callers?, callees?}]}`
where `callers`/`callees` = `{internal:[{name,path,line,kind}], ambiguous:[{name,count}], external_count}`
(lines 0-based). An `advisory` fires on truncation or if ripgrep isn't installed.

### `repoctx definition <name>`

Exact-name (case-sensitive) lookup. Auto-filters to definition kinds —
field/variable/section/key noise is excluded. `--lang`/`--limit` apply.
Multiple hits are normal; pick the right one. Zero hits returns exit 0
with `count: 0` — but because the match is case-sensitive, a 0-hit may
carry an `advisory` naming case-insensitive near-misses (e.g. you typed
`store`, `Store` exists). Retry with the exact casing it suggests.

Use this when you **know the name** and want canonical definition
sites.

### `repoctx context <symbol> [--context C] [--limit N]`

Exact-name match + source window. Defaults: 5 lines above/below, top 3
hits. Reads source from disk so the bytes are current; `stale: true`
flags hits whose indexed `(mtime_ns, size)` no longer matches disk.

Prefer this over `definition` + a follow-up file Read — one round trip
instead of two.

### `repoctx callers <name>` / `callees <name>`

Direct call-graph edges. `callers` = who calls `name`; `callees` = what
`name` calls. `--limit` caps results (default 50). Each edge gives the
caller symbol, the callee (resolved symbol, or `null` when
external/unresolved), the call site, and an `ambiguous` flag.

Use these instead of `rg "name("` — you get structured caller/callee
symbols with locations, not raw text matches.

### `repoctx deadcode` / `impact <name>` / `cycles`

Analyses over the call graph (no extra indexing): `deadcode` = function/method
symbols with no in-repo caller (entry points excluded) — grep can't do this;
`impact <name>` = blast radius (everything transitively calling `name`, i.e.
"what breaks if I change this"); `cycles` = recursion / mutual recursion.
Name-based + advisory — verify before acting (dynamic dispatch / traits / FFI
/ public API are invisible).

### `repoctx callgraph <name> [--depth N] [--direction up|down|both]`

Transitive call graph from `name`. `--depth` (default 3) bounds the walk;
`--direction down` follows callees, `up` follows callers, `both` does
both. Each edge is tagged with `depth` and `direction`. Cycle-safe.

**Accuracy — read before trusting it.** The call graph is name-based and
approximate, the same accuracy class as `definition`:

- No receiver-type disambiguation — `a.foo()` and `b.foo()` both resolve
  to every `foo` (such edges are flagged `ambiguous: true`).
- External / stdlib / dynamically-dispatched callees show `callee: null`
  (name shown, location unknown).
- Function pointers, higher-order calls, and cross-language edges are
  invisible.
- Languages: the core 8 — Rust, Python, JavaScript, TypeScript, Go, C,
  C++, Java. Other languages return no edges yet.

When edges are ambiguous or unresolved the command emits an `advisory`;
treat the graph as a strong hint and cross-check with `rg` when it
matters.

### `repoctx deps <file>` / `rdeps <module>`

The import / dependency graph. `deps <file>` lists the module specifiers a
file imports; `rdeps <module>` lists the files whose import specifier
**contains** the argument as a substring (so `rdeps storage-idb` finds every
importer of `@adapters/storage-idb`). Core 8 languages (Rust/Python/JS/TS/
TSX/Go/C/C++/Java). Each edge: `{file, module, line, resolution}`.

For an explicit **layering check**, `repoctx boundary --from <path>
--to <module>` lists files whose path contains `--from` importing a
specifier containing `--to` (e.g. `boundary --from src/ui --to @adapters`
= "does the UI import the storage adapter?"); `--forbid` exits non-zero on
any crossing (CI gate). `repoctx import-cycles` finds circular
imports; `repoctx modules` gives the resolved import topology + a
dependency-first build order (relative imports resolved to files;
alias/package specifiers counted external).

Use these for **architecture / boundary questions** instead of grepping
import lines: "what depends on this module", "does the UI layer import the
storage adapter directly". String-based — the raw specifier is stored as
written (aliases like `@adapters/x` match exactly; relative `./x` are
verbatim). Precise specifier→file resolution is not done yet; an empty
result carries an `advisory`.

### `repoctx overview`

Repo architecture in one call — totals (code vs doc/config split), per-language
breakdown, per-directory module sizes **ranked by code symbols**, entry points
(`main` + JS/TS bootstraps), and hotspots (most-called symbols, receiver-aware).
Use it first when **dropped into an unfamiliar repo** instead of `ls`/`cat`/grep
round-trips. Public API surface not included yet (#8); name-based (ADR-0010).

### `repoctx communities` / `report` / `export`

The orientation layer — understand a repo's shape without reading it:

- `communities` — clusters the call graph into **subsystems** (Louvain) +
  god nodes (highest-degree hubs). "Where are the seams?"
- `report` — a deterministic one-page markdown architecture report (god nodes,
  subsystems, cross-cluster bridges, entry points, suggested questions). `--out
  REPORT.md` writes a file.
- `export --out graph.html` — a self-contained interactive call-graph (no
  server/build): subsystems colored, edges styled by ambiguity.

A subsystem = a cluster with ≥ `analysis.subsystem_min_size` members (default
5); all three report the same count. To drill into a subsystem, run
`callgraph <its-label> --direction both` or `outline` its members' files.

> The repo map you received at session start came from `repoctx prime` (the
> SessionStart hook). These commands give the fuller, on-demand picture.

### `repoctx changed [--since REF]`

Change-aware blast radius for review: symbols changed since a git ref
(default `HEAD`; `--since main` for a whole branch) + their transitive
callers ("what this change touches + what it can break"). Name-based
(ADR-0010); tracked files only.

### `repoctx outline <file>`

Document-symbol tree for one file. Path may be repo-relative or
absolute. Indented containment tree (human) or flat `{count, items}`
(machine).

Use this when you need the **structure** of a file but not its full
contents.

### `repoctx status`

File/symbol counts + per-language breakdown + staleness. `--fast` skips
the staleness walk. Quick health check before deeper work.

### `repoctx gain`

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

## Use repoctx instead of grep / find / cat

repoctx does **not** intercept your commands — you choose it. For any
structural question, run the repoctx command instead of reaching for
`rg`/`grep`/`find`/wholesale `Read`. Map your instinct to the command:

| Instead of… | Run |
|---|---|
| `rg <ident>` / `grep -r <ident>` | `repoctx search <ident>` (symbol defs + every textual match, compressed) |
| `rg "fn <name>"` / `class`/`struct`/`def` | `repoctx definition <name>` |
| `rg -A/-B/-C <n> <ident>` (context) | `repoctx context <ident> --context <n>` |
| `rg "<name>\("` (call sites) | `repoctx callers <name>` (and `callees` / `callgraph`) |
| `rg "import .*<mod>"` | `repoctx rdeps <mod>` (or `deps <file>`) |
| `find -name '*.x'` / `ls -R` to learn layout | `repoctx overview` / `repoctx report` |
| reading many files to grasp structure | `repoctx communities` / `repoctx export --out graph.html` |

Only fall back to `rg`/`Read` when (a) the language is `partial` coverage
(see the advisory + `languages`), or (b) you need prose-level reasoning
about an implementation after repoctx has located it.

## Coverage advisory — when to fall back to ripgrep

Every machine output can carry an optional top-level `advisory`
field. When present, it means repoctx may be underserving this
query because of language coverage limits. The advisory text
suggests a concrete fallback command, typically `rg -n <pattern>`.

Always check `advisory` on your responses. When set, also run the
suggested `rg` command and merge the results — don't trust
repoctx's `count: 0` blindly on a partial-coverage language.

To get the full coverage matrix in one call:

```sh
repoctx languages --json
```

Returns `{slug, coverage: "full"|"partial", notes}` per language.
Cache this once per session.

Currently 16 languages are `full` — Rust, Go, Python, TypeScript,
TSX, JavaScript, C, C++, Java, C#, Ruby, PHP, Lua, Kotlin, Swift,
Markdown. JSON, YAML, TOML are `partial` (top-level keys / TOML section
headers; opt-in all-depth via `index.nested_keys`), and Bash is
`partial` (functions only). A query like "where is `containerPort`
configured" against a k8s YAML will return zero hits even though `rg`
would find it — but `repoctx search containerPort` will, since it runs
ripgrep too. The advisory
will tell you so.

## Gotchas

- Rust `struct`/`enum`/`union`/`type` are all reported as `class` per
  the upstream `tags.scm` mapping; Rust `trait` is `interface`.
- TypeScript and TSX have full coverage including plain class, plain
  function, arrow functions assigned to identifiers, type aliases,
  and enums. (Vendored from Aider, Apache-2.0.)
- Markdown headings (ATX and setext) are reported as `section`.
- Top-level JSON/YAML/TOML keys are `key`; nested keys are not
  surfaced. The advisory field will warn you when this matters.

## Examples

```sh
# Find every type alias that mentions "Token"
repoctx symbols Token --kind type --json | jq '.items[]'

# Where is `parse_config` defined?
repoctx definition parse_config

# Who calls `parse_config`, and what does it call?
repoctx callers parse_config
repoctx callees parse_config

# Trace two hops of what `handle_request` calls
repoctx callgraph handle_request --depth 2 --direction down

# Show me the resolve_path function with 10 lines of context
repoctx context resolve_path --context 10 --limit 1

# What's the structure of the main entry point?
repoctx outline src/main.rs

# How healthy is the index?
repoctx status
```
