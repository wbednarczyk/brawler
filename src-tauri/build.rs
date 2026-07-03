fn main() {
    tauri_build::build();

    // Dual-execution mock-fidelity corpus (ADR 0049, T6): the corpus JSON lives
    // one level above the Cargo workspace (`../src/test/scenarios/`) so both the
    // Rust replayer and the TS replayer share a single file. `cargo-mutants`
    // copies only the workspace directory into a scratch tree, so that relative
    // path doesn't exist there. `BRAWLER_FIDELITY_CORPUS`, if already set in the
    // environment (the `mutants` Makefile target exports it as an absolute path
    // before invoking `cargo mutants`), wins and points at the real, uncopied
    // file; otherwise default to the normal relative location used by every
    // other build (`cargo test`, `cargo nextest run`, IDE, CI).
    let corpus_path = std::env::var("BRAWLER_FIDELITY_CORPUS").unwrap_or_else(|_| {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
        format!("{manifest_dir}/../src/test/scenarios/fidelity-corpus.json")
    });
    println!("cargo:rustc-env=BRAWLER_FIDELITY_CORPUS={corpus_path}");
    println!("cargo:rerun-if-env-changed=BRAWLER_FIDELITY_CORPUS");
    println!("cargo:rerun-if-changed={corpus_path}");
}
