#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 3 ]; then
  printf "Usage: %s <version> <exe-path> <output-dir>\n" "$0" >&2
  exit 2
fi

version="$1"
exe_path="$2"
output_dir="$3"
zip_name="brawler-${version}-windows-x64-portable.zip"
staging_dir="target/release-staging/windows-x64-portable"
readme_path="$staging_dir/README-portable-windows.txt"

if [ ! -f "$exe_path" ]; then
  printf "Expected Windows executable not found: %s\n" "$exe_path" >&2
  exit 1
fi

rm -rf "$staging_dir"
mkdir -p "$staging_dir" "$output_dir"
zip_path="$(cd "$output_dir" && pwd)/$zip_name"

cp -f "$exe_path" "$staging_dir/brawler.exe"
cat > "$readme_path" <<'README'
Brawler portable Windows build

Run brawler.exe from this folder. On first start, Brawler creates a data folder
next to the executable and stores its local database and logs there.

The application uses the system Microsoft WebView2 runtime. Current Windows 10
and Windows 11 installations usually already include it. If the application
does not start because WebView2 is missing, install the Microsoft WebView2
Runtime from Microsoft and try again.

Do not place credentials, private backups, logs, or user databases inside this
portable folder unless you intentionally want to move them with the application.
README

(
  cd "$staging_dir"
  zip -q -9 "$zip_path" brawler.exe README-portable-windows.txt
)

printf "Created %s\n" "$zip_path"
