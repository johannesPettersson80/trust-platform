# A4 run 1 — failed, no runtime behavior proof

Run: 10 October 2026, 05:36:26–05:40:04 UTC, on `scena-rust-builder`,
Rust/Cargo 1.95.0, base `77b91381f`, six Cargo jobs and an isolated mounted-volume target.
The [ledger](ledger.tsv) has 10 required passes, 12 required failures and four
required unrun steps; advisory maintenance has one pass and two failures.

The frozen formatted-source manifest has 243 path/deletion records and SHA256
`ee0fd1f77ba7d1d846dccb270feabfad881d12e63b292e9859b154c8a759ccf8`.
The exact frozen files are retained in `formatted-source-snapshot.tar.gz`; every
record was matched against the manifest before archiving. Deletions remain recorded
in the manifest. Its SHA256 is
`8780116538a9b82eebbe5cacf3e909acb3025a8859b48de20c4c768af17824e3`.
The original saved application remains retained as `original-program-v2.stbc`;
fixture generation did not reach execution. No numeric companion was generated.

## Causes and limits

- Core compilation failed on one missing call-frame presence field and three
  unused bindings/imports (the fourth diagnostic appears in core unit-test builds).
  Core/native/i686/MCU and hosted suites therefore provide no runtime assertions
  or successful cross-target compilation. Clippy and cross-warning checks also failed.
- The architecture check needs ownership classification for the shared VM namespace
  and smaller preparation, typed-construction and assignment functions. Diagram
  rendering and drift checks were unrun behind that failed prerequisite.
- Supply-chain tooling ran 50 tests with six setup errors: canonical guard tests
  require `scripts/cargo_target_path.sh`, absent on this older branch. Dependency
  exception checking passed; cargo-deny/audit were not reached.
- Advisory provenance refresh found the retain mutant's old hosted owner path and
  failed closed before writing the collected updates. The metadata validator then
  reported six stale case-source bindings across three execution-contract cases.

Successful executable checks were two Rust provenance-helper tests and 16 Python
mutation-tooling tests. Formatting, diff integrity and feature-tree collection
passed; the feature trees alone do not prove the library compiles without std.

All reachable independent steps finished before any correction. The raw logs,
commands and artifacts are indexed in `artifact-sha256.json`. That index excludes
itself; this README is included. The temporary Git metadata index is retained only
in the external raw artifact directory, not as repository evidence. Historical
measured mutation records were not regenerated or relabeled.
