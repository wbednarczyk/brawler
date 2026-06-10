#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
validator="$script_dir/validate-commit-message.sh"

"$validator" --message "feat(release): validate commit messages"
"$validator" --message "fix(api)!: validate breaking change syntax"
"$validator" --message "Merge pull request #1 from example/branch"
"$validator" --message "bad commit message" >/tmp/brawler-invalid-commit-message-check.out 2>&1 && {
  printf "Invalid commit message unexpectedly passed validation.\n" >&2
  exit 1
}

printf "Commit message validator accepts expected messages and rejects invalid messages.\n"
