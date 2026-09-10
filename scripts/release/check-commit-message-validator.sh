#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
validator="$script_dir/validate-commit-message.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

"$validator" --message "feat(release): validate commit messages"
"$validator" --message "fix(api)!: validate breaking change syntax"
"$validator" --message "Merge pull request #1 from example/branch"
"$validator" --message "bad commit message" >"$tmp/invalid.out" 2>&1 && {
  printf "Invalid commit message unexpectedly passed validation.\n" >&2
  exit 1
}

# Forge attribution (CLAUDE.md § Claude-Native Ecosystem): an AI co-author
# trailer or a "generated with" footer fails in file mode; a human co-author
# trailer passes (the rule is precise, never a broad flag).
printf 'fix(x): a\n\nbody\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>\n' >"$tmp/ai-trailer"
"$validator" "$tmp/ai-trailer" >"$tmp/ai-trailer.out" 2>&1 && {
  printf "An AI co-author trailer unexpectedly passed validation.\n" >&2
  exit 1
}
printf 'fix(x): a\n\n🤖 Generated with [Claude Code](https://claude.com/claude-code)\n' >"$tmp/ai-footer"
"$validator" "$tmp/ai-footer" >"$tmp/ai-footer.out" 2>&1 && {
  printf "A 'generated with' AI footer unexpectedly passed validation.\n" >&2
  exit 1
}
printf 'fix(x): a\n\nCo-authored-by: Jan Kowalski <jan@example.com>\n' >"$tmp/human-trailer"
"$validator" "$tmp/human-trailer"

printf "Commit message validator accepts expected messages and rejects invalid messages and AI attribution.\n"
