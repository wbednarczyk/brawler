#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];

if (!version) {
  console.error("Usage: scripts/release/bump-version.mjs <version>");
  process.exit(64);
}

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`Invalid SemVer version: ${version}`);
  process.exit(64);
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function replaceRequired(path, pattern, replacement) {
  const original = readFileSync(path, "utf8");
  const updated = original.replace(pattern, replacement);

  if (updated === original) {
    console.error(`Could not update version in ${path}`);
    process.exit(1);
  }

  writeFileSync(path, updated);
}

const packageJson = readJson("package.json");
const currentVersion = packageJson.version;
packageJson.version = version;
writeJson("package.json", packageJson);

const packageLock = readJson("package-lock.json");
packageLock.version = version;
if (packageLock.packages?.[""]) {
  packageLock.packages[""].version = version;
}
writeJson("package-lock.json", packageLock);

replaceRequired(
  "src-tauri/Cargo.toml",
  /(^\[package\][\s\S]*?^version = )"[^"]+"/m,
  `$1"${version}"`,
);

replaceRequired(
  "src-tauri/Cargo.lock",
  /(\[\[package\]\]\nname = "brawler"\nversion = )"[^"]+"/,
  `$1"${version}"`,
);

const tauriConfig = readJson("src-tauri/tauri.conf.json");
tauriConfig.version = version;
writeJson("src-tauri/tauri.conf.json", tauriConfig);

replaceRequired(
  "src-tauri/src/lib.rs",
  new RegExp(`assert_eq!\\(response\\.version, "${currentVersion.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"\\);`),
  `assert_eq!(response.version, "${version}");`,
);

console.log(`Bumped version: ${currentVersion} -> ${version}`);
