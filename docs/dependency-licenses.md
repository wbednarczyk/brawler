# Dependency License Audit

This document records the current dependency-license posture for public-opening work. It is not legal advice.

## Project License

Brawler source code is licensed under the Mozilla Public License 2.0.

The npm package and Cargo crate remain marked non-publishable because Brawler is distributed as source code and desktop binaries, not as npm or crates.io packages.

## Audit Method

Frontend dependency licenses were read from installed `node_modules/*/package.json` metadata matching `package-lock.json`.

Rust dependency licenses were read from `cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1`.

## Frontend License Families

Current frontend dependency metadata contains these license families:

- MIT
- Apache-2.0
- Apache-2.0 OR MIT
- MIT OR Apache-2.0
- BSD-2-Clause
- BSD-3-Clause
- ISC
- MIT-0
- CC-BY-4.0 for `caniuse-lite`

No mandatory GPL-family frontend dependency was found in the current audit.

## Rust License Families

Current Rust dependency metadata contains these license families:

- MIT
- Apache-2.0
- MIT OR Apache-2.0
- Apache-2.0 OR MIT
- BSD-2-Clause
- BSD-3-Clause
- ISC
- MPL-2.0
- Unicode-3.0
- Unlicense OR MIT
- Zlib
- permissive multi-license expressions including MIT, Apache-2.0, BSD, Zlib, BSL-1.0, LLVM-exception, or Unicode-3.0 options
- `MIT OR Apache-2.0 OR LGPL-2.1-or-later` for `r-efi`, where permissive alternatives are available

No mandatory GPL-family Rust dependency was found in the current audit.

## Publication Rule

Before adding a runtime dependency, review its license and document the reason for the dependency. Dependencies that introduce mandatory copyleft, network-service obligations, unusual attribution duties, paid/commercial restrictions, or unclear metadata require maintainer review before adoption.
