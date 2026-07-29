fn main() {
    tauri_build::build();

    // Shared test fixtures (the mock-fidelity corpus and scenario JSONs, ADR
    // 0049) live one level above the Cargo workspace (`../src/test/scenarios/`)
    // so the Rust and TS sides read single files. `cargo-mutants` copies only
    // the workspace directory into a scratch tree, so that relative path does
    // not exist there. `BRAWLER_SCENARIOS_DIR`, if already set in the
    // environment (the `mutants` Makefile target exports it as an absolute path
    // before invoking `cargo mutants`), wins and points at the real, uncopied
    // directory; otherwise default to the normal relative location used by
    // every other build (`cargo test`, `cargo nextest run`, IDE, CI). Tests
    // reach any shared fixture ONLY via
    // `include_str!(concat!(env!("BRAWLER_SCENARIOS_DIR"), "/<file>"))` —
    // a literal `../../../` path compiles locally but breaks every mutants
    // sweep (#110); the `source_tree_guards` test enforces this.
    let scenarios_dir = std::env::var("BRAWLER_SCENARIOS_DIR").unwrap_or_else(|_| {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
        format!("{manifest_dir}/../src/test/scenarios")
    });
    println!("cargo:rustc-env=BRAWLER_SCENARIOS_DIR={scenarios_dir}");
    println!("cargo:rustc-env=BRAWLER_FIDELITY_CORPUS={scenarios_dir}/fidelity-corpus.json");
    println!("cargo:rerun-if-env-changed=BRAWLER_SCENARIOS_DIR");
    println!("cargo:rerun-if-changed={scenarios_dir}");
}
