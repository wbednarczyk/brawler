#!/usr/bin/env bash
set -euo pipefail

tag="${1:-}"
changelog="${2:-CHANGELOG.md}"

if [[ -z "$tag" ]]; then
  printf "Usage: %s <tag> [CHANGELOG.md]\n" "$0" >&2
  exit 64
fi

if [[ ! -f "$changelog" ]]; then
  printf "Changelog file not found: %s\n" "$changelog" >&2
  exit 66
fi

awk -v tag="$tag" '
  BEGIN {
    found = 0
    printing = 0
  }

  $0 ~ "^##[[:space:]]+" tag "([[:space:]-]|$)" {
    found = 1
    printing = 1
    print
    next
  }

  printing && $0 ~ "^##[[:space:]]+" {
    exit
  }

  printing {
    print
  }

  END {
    if (!found) {
      exit 2
    }
  }
' "$changelog" || {
  status=$?
  if [[ "$status" -eq 2 ]]; then
    printf "No changelog entry found for %s in %s\n" "$tag" "$changelog" >&2
  fi
  exit "$status"
}
