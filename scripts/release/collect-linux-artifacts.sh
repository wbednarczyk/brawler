#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  printf "Usage: %s <version> <output-dir>\n" "$0" >&2
  exit 2
fi

version="$1"
output_dir="$2"
bundle_dir="src-tauri/target/release/bundle"
artifact_prefix="brawler-${version}-linux-amd64"

mkdir -p "$output_dir"

copy_one() {
  local kind="$1"
  local extension="$2"
  local source
  source="$(find "$bundle_dir/$kind" -maxdepth 1 -type f -name "*.$extension" | sort | head -n 1 || true)"

  if [ -z "$source" ]; then
    printf "Expected Linux %s artifact was not found under %s/%s\n" "$extension" "$bundle_dir" "$kind" >&2
    exit 1
  fi

  cp -f "$source" "$output_dir/${artifact_prefix}.${extension}"
  printf "Copied %s\n" "$output_dir/${artifact_prefix}.${extension}"
}

copy_one "deb" "deb"
copy_one "rpm" "rpm"
copy_one "appimage" "AppImage"

