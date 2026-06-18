#!/usr/bin/env bash
# Claude Code SessionStart hook. Anything echoed to stdout here is
# injected into the agent's context at the start of every session.
#
# The block below is managed by `repoctx init` (regenerated on re-run).
# Add your own context after it — it is preserved across re-runs.

# >>> repoctx (managed — edits here are overwritten) >>>
repoctx prime 2>/dev/null
# <<< repoctx (managed) <<<

# --- your session-start context below (preserved across `repoctx init`) ---
# Brawler live repo snapshot: branch/commits, working-tree state, version,
# active Radicle work, and the milestone load. Read-only and best-effort — every
# command is guarded and the script always exits 0, so a hook hiccup never blocks
# session start. The standing RULES and the doc load order live in
# session-context.sh (the committed, all-agents hook); this is only the dynamic
# SNAPSHOT and does not repeat them.
# Commands run raw here (this is a hook, not an agent action — the `rtk` prefix
# rule applies to commands the agent issues, not to this script).
set -u
cd "${CLAUDE_PROJECT_DIR:-$(pwd)}" 2>/dev/null || exit 0

print_section() {
  printf '\n--- %s ---\n' "$1"
}

print_section "branch + last 5 commits"
printf 'on %s\n' "$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')"
git log --format="%h %s" -5 2>/dev/null || echo "(no git)"

print_section "working tree"
dirty=$(git status --short 2>/dev/null | wc -l | tr -d ' ')
if [ "${dirty:-0}" -eq 0 ]; then
  echo "clean"
else
  echo "${dirty} uncommitted file(s) — do NOT commit/push unless the user asks or you are running the brawler-release skill"
  git status --short 2>/dev/null | head -12
fi

print_section "version + latest release tag"
ver=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' package.json 2>/dev/null | head -1)
printf 'package.json: %s    latest tag: %s\n' \
  "${ver:-?}" "$(git describe --tags --abbrev=0 2>/dev/null || echo none)"

# Radboard label conventions (docs/kanban.md): epic / parent:<hex7> /
# milestone:vX.Y.Z / state:* / priority:* / area:*. `rad issue list` shows OPEN
# issues only, which is what we want for an "active work" snapshot.
if command -v rad >/dev/null 2>&1; then
  print_section "active work (state:doing / in-progress)"
  active=$(rad issue list 2>/dev/null | grep -Ei "state:(doing|in-progress)" | head -10)
  if [ -n "${active}" ]; then
    printf '%s\n' "${active}"
  else
    echo "(none flagged state:doing — see open epics below for what's next)"
  fi

  print_section "open epics"
  rad issue list 2>/dev/null | grep -E "(^|[, ])epic([, ]|$)" | head -10 || true

  print_section "open issues per milestone"
  rad issue list 2>/dev/null | grep -oE "milestone:v[0-9.]+" | sort | uniq -c | sort -rn | head -10
else
  print_section "radicle"
  echo "(rad not on PATH)"
fi

exit 0
