# Brawler — agent entry point

Project agent contract and required reading: @AGENTS.md

## Token discipline

Prefix shell/file commands with `rtk` (e.g. `rtk git`, `rtk grep`, `rtk read`, `rtk cargo`, `rtk rad`); it compresses output before it reaches context. `rtk proxy <cmd>` runs raw. Run `rtk trust` once in this repo so the project-local filters in `.rtk/` apply. Prefer `repoctx` and targeted reads over whole-file reads.
<!-- repoctx:start -->
## Code navigation with `repoctx`

Prefer `repoctx` over `grep`/`find`/wholesale `Read` for structural
questions about this repo. The `repoctx` skill at
`.claude/skills/repoctx/SKILL.md` carries the full command reference
and choose-the-right-tool guidance.

Quick cues:

- "Get oriented in this repo" → `repoctx overview`
- "Where is X defined?" → `repoctx definition X`
- "Show me X and its surrounding code" → `repoctx context X`
- "Explore symbols matching ..." → `repoctx symbols <substring>`
- "Find X everywhere (defs + textual, incl. comments)" → `repoctx search X`
- "Who calls X / what does X call?" → `repoctx callers X` / `repoctx callees X`
- "Trace the call chain from X" → `repoctx callgraph X --depth N --direction up|down|both`
- "What breaks if I change X?" → `repoctx impact X`
- "What does this branch change + its blast radius?" → `repoctx changed --since main`
- "Find dead code / call cycles" → `repoctx deadcode` / `repoctx cycles`
- "What does this file import / what imports module M?" → `repoctx deps <file>` / `repoctx rdeps <module>`
- "Does layer A import layer B?" → `repoctx boundary --from <path> --to <module>`
- "Circular imports / module build order?" → `repoctx import-cycles` / `repoctx modules`
- "Structure of one file" → `repoctx outline <file>`
- "Index health" → `repoctx status`

All read commands auto-index on first run. Default output is TOON for
pipes (token-efficient) and human for TTYs; pass `--json` when piping
into `jq`. Working tree: `/home/wojtas/projects/brawler`.

<!-- repoctx:end -->