#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf "Usage: %s <commit-message-file>|--message <message>\n" "$0" >&2
}

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

if printf "%s" "$subject" | grep -Eq '^(Merge|Revert) '; then
  exit 0
fi

if printf "%s" "$subject" | grep -Eq '^(build|chore|ci|docs|feat|fix|perf|refactor|style|test)(\([a-z0-9._-]+\))?!?: .+$'; then
  exit 0
fi

cat >&2 <<'EOF'
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
EOF
exit 1
