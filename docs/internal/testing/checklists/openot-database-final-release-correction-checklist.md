# OpenOT database final release correction checklist

Owner specification: `docs/specs/33-openot-database-persistence.md`, especially
Sections 2.1 and 6.1. Candidate under correction:
`69dfa503a98e00e420dc8a9ce7b48f069e1700f7`.

This is the single repair ledger for PR #118 after all 24 exact-head GitHub
checks and the exact-head automatic review completed. The aggregate Release
Gate Report failure is derivative of the Clippy failure, not a separate code
defect. No corrective push is permitted until Sections 1 through 7 are green
on one frozen replacement commit.

## 1. Complete failure ledger

- [x] Preserve the complete GitHub failure artifact and logs for candidate
  `69dfa503a`: Clippy failed and Release Gate Report failed as its aggregate.
- [x] Record the three warning-deny failures: audited-value projection return
  types in MySQL, SQL Server, and TimescaleDB exceed Clippy complexity limits.
- [x] Record the release-guard parity defect: the guard ran `just clippy`
  without `-D warnings`, while GitHub ran
  `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] Record exact-head review findings: initial InfluxDB transport failure is
  not retryable, and interrupted runner jobs can leave Docker resources when
  temporary state is lost.

## 2. Specification before tests

- [x] Specify initial InfluxDB `/health` transport failure as retryable while a
  received HTTP rejection remains permanent.
- [x] Specify exact per-run Docker ownership labels and fail-closed orphan
  discovery independent of temporary state.
- [x] Confirm no public database schema, TOML shape, example, or database
  migration/version change is introduced by this correction.
- [x] Confirm SOLID/KISS/DRY boundaries: HTTP error classification remains in
  the Influx adapter; Docker ownership remains in runner scripts; CI parity
  remains in the release-candidate validator; shared audited projection tuples
  use one descriptive type alias.

## 3. Focused tests before production changes

- [x] Add and run an expected-red Influx adapter test proving a refused initial
  HTTP transport maps to `PersistenceError::Connection`.
- [x] Add and run an expected-red service lifecycle test proving unreachable
  Influx starts the PLC runtime in `retrying`, not `faulted`.
- [x] Add and run an expected-red runner test proving exact-labelled orphaned
  containers, attached volumes, and network are reclaimed without state files.
- [x] Add and run an expected-red release-guard self-test requiring the exact
  GitHub warning-deny Clippy command.
- [x] Preserve the GitHub Clippy log as the pre-fix red baseline for all three
  audited-projection type-complexity warnings.

## 4. Minimal implementation

- [x] Classify only initial Influx HTTP transport failures as connection errors;
  preserve non-transport HTTP failures as permanent open errors.
- [x] Label every prepared container and network and make teardown discover
  only the exact prefix-labelled resources before inspecting/removing volumes.
- [x] Replace repeated audited projection tuple signatures with one descriptive
  shared alias; do not suppress the lint.
- [x] Make both `just clippy` and exact-candidate preparation use GitHub's
  `cargo clippy --all-targets --all-features -- -D warnings` shape.

## 5. Focused green proof

- [x] Rerun each focused red test green using the same command and feature set.
- [x] Run the complete release-guard Python self-test suite.
- [x] Run the runner contract suite, including symlink/mismatched-label safety.
- [x] Run strict warning-deny Clippy and confirm zero warnings.
- [x] Run the four required runtime vertical integration suites.

## 6. Examples, docs, and real systems

- [x] Run the OpenOT example validator and confirm all backend examples remain
  unchanged and valid on Linux/Windows path rules.
- [x] Update changelog and operator verification documentation for retry and
  runner-cleanup behavior; no new example is required because configuration
  and stored data are unchanged.
- [ ] Run the real database gate with a real PLC workload against SQLite,
  PostgreSQL, TimescaleDB, MySQL, MariaDB, SQL Server, and InfluxDB 3.
- [x] Exercise teardown once with normal state and once through labelled orphan
  recovery; verify no owned container, volume, network, credential, or TLS
  artifact remains.

## 7. Detailed audit and frozen candidate

- [x] Perform a line-by-line spec-to-test-to-code audit after implementation;
  inspect all Influx HTTP call sites and every runner-created Docker resource.
- [x] Verify `AGENTS.md`, skills, workflow, `justfile`, and guard command shapes
  agree exactly on warning-deny Clippy.
- [ ] Run remote format, strict Clippy, `test-all`, Windows cross-target warning
  gates, runtime verticals, docs/diagram checks, and exact-candidate preparation
  on one clean frozen commit.
- [ ] Push once, request/read exact-head automatic review, collect all GitHub
  checks, and make no partial corrective push.
- [ ] Merge only through `check-merge`, then verify main CI, annotated tag,
  Release workflow, GitHub Latest/assets/checksums, Marketplace propagation,
  and a clean post-merge audit.
