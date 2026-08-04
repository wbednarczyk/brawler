# Committed real ESPI filing samples

Real, complete, unmodified official ESPI/EBI periodic filing files committed as test
samples under [ADR 0094](../../../docs/adr/0094-committed-public-espi-report-samples.md)
(a narrow amendment of ADR 0091 decision 4). These are mandated-public issuer disclosures;
attribution lives in [MANIFEST.json](MANIFEST.json) and is machine-checked by the manifest
guard test (budget ≤ 5 MB, every file manifested, hash/size/container match).

Consumed by the real-report sample tests in the extraction/report_diff module trees
(files are read at runtime via `CARGO_MANIFEST_DIR`, never embedded with `include_bytes!`).
Never edit a sample file; replacing one is an ADR-0094-conscious change (update MANIFEST
hash + expected values + the PR explains why).
