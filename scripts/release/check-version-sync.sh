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
  printf "Version files are synchronized at %s.\n" "$package_version"
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
