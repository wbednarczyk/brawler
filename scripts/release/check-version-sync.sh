#!/usr/bin/env bash
set -euo pipefail

package_version="$(node -p "require('./package.json').version")"
package_lock_version="$(node -p "require('./package-lock.json').version")"
tauri_version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
cargo_version="$(awk '
  /^\[package\]/ { in_package = 1; next }
  /^\[/ && in_package { in_package = 0 }
  in_package && /^version = / {
    gsub(/"/, "", $3)
    print $3
    exit
  }
' src-tauri/Cargo.toml)"
cargo_lock_version="$(awk '
  /^\[\[package\]\]/ { in_package = 1; found_name = 0; next }
  in_package && /^name = "brawler"/ { found_name = 1; next }
  in_package && found_name && /^version = / {
    gsub(/"/, "", $3)
    print $3
    exit
  }
' src-tauri/Cargo.lock)"

if [ "$package_version" = "$package_lock_version" ] \
  && [ "$package_version" = "$tauri_version" ] \
  && [ "$package_version" = "$cargo_version" ] \
  && [ "$package_version" = "$cargo_lock_version" ]; then
  # Tag parity (ADR 0090 amendment, 2026-07-28): the release bot stamps the
  # released version into the manifests right after tagging, so on any checkout
  # that can see v* tags the manifest version must equal the newest tag —
  # anything else means the stamp commit failed or was reverted and the repo is
  # silently drifting from the released version. Skipped when no v* tag is
  # reachable (shallow CI checkouts fetch no tags); the net is local runs,
  # where tags always exist.
  latest_tag="$(git tag --list 'v[0-9]*' --sort=-v:refname 2>/dev/null | head -1)"
  if [ -n "$latest_tag" ] && [ "v$package_version" != "$latest_tag" ]; then
    cat >&2 <<EOF
Version files are synchronized at $package_version, but the newest release tag
is $latest_tag. The repo manifests must carry the released version (release.yml
stamps them post-tag). If the release bot-commit failed, re-apply it with:
  node scripts/release/bump-version.mjs ${latest_tag#v}
EOF
    exit 1
  fi
  printf "Version files are synchronized at %s%s.\n" "$package_version" \
    "${latest_tag:+ (matches $latest_tag)}"
  exit 0
fi

cat >&2 <<EOF
Version files are not synchronized:
  package.json:              $package_version
  package-lock.json:         $package_lock_version
  src-tauri/tauri.conf.json: $tauri_version
  src-tauri/Cargo.toml:      $cargo_version
  src-tauri/Cargo.lock:      $cargo_lock_version
EOF
exit 1
