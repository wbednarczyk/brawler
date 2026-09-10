#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  printf "Usage: %s <commit-message-file>|--message <message>\n" "$0" >&2
}

# File mode validates the WHOLE message (subject schema + forge attribution);
# --message validates a bare subject only.
message_file=""
if [ "$#" -eq 1 ]; then
  message_file="$1"
  if [ ! -f "$message_file" ]; then
    printf "Commit message file not found: %s\n" "$message_file" >&2
    exit 2
  fi
  subject="$(sed -n '/^[[:space:]]*#/!{/./{p;q;}}' "$message_file")"
elif [ "$#" -eq 2 ] && [ "$1" = "--message" ]; then
  subject="$2"
else
  usage
  exit 2
fi

if [ -z "${subject:-}" ]; then
  printf "Commit message subject is empty.\n" >&2
  exit 1
fi

subject_ok=0
if printf "%s" "$subject" | grep -Eq '^(Merge|Revert) '; then
  subject_ok=1
elif printf "%s" "$subject" | grep -Eq '^(build|chore|ci|docs|feat|fix|perf|refactor|style|test)(\([a-z0-9._-]+\))?!?: .+$'; then
  subject_ok=1
fi

if [ "$subject_ok" -ne 1 ]; then
  cat >&2 <<'HELP'
Commit message must use Conventional Commits:

  <type>(optional-scope): <subject>

Allowed types:
  build, chore, ci, docs, feat, fix, perf, refactor, style, test

Examples:
  feat(research): add company evidence timeline
  fix(sources): include NewConnect lookup results
  docs(release): document changelog workflow
  chore(release): bump version to 0.25.0

Use "!" before ":" for breaking changes when needed:
  feat(api)!: change research timeline result shape
HELP
  exit 1
fi

if [ -n "$message_file" ]; then
  "$script_dir/check-forge-attribution.sh" "$message_file"
fi
