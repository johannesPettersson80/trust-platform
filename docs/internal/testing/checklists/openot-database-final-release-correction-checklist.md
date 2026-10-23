# OpenOT database final release correction checklist

Owner specification: `docs/specs/33-openot-database-persistence.md`, especially
Sections 2, 4.5, 4.8, 6.1, and 9. Candidates under correction:
`69dfa503a98e00e420dc8a9ce7b48f069e1700f7` and
`e4e8b05bfb755527e7170cb8ecddf2408648cefa`.

This is the single repair ledger for PR #118 after all 24 exact-head GitHub
checks and the exact-head automatic review completed. Each aggregate Release
Gate Report failure is derivative of its owning failed job, not a separate code
defect. No corrective push is permitted until Sections 1 through 8 are green
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

## 8. Exact-head supply-chain and automatic-review correction

- [x] Preserve the complete 24-check GitHub failure artifact and both logs for
  `e4e8b05b`: `Supply Chain` rejected yanked `mysql@28.0.0`; `Release Gate
  Report` failed only because the supply-chain marker was absent.
- [x] Record the exact-head automatic review findings: SQL Server logged-value
  requests can exceed 2,100 parameters, and MySQL/MariaDB does not physically
  constrain the schema marker to singleton `1`.
- [x] Specify non-yanked dependency proof, exact pre-push supply-chain parity,
  SQL Server parameter-bounded chunking, and MySQL/MariaDB singleton DDL before
  tests or production changes.
- [x] Add and run expected-red focused tests for all four missing behaviors.
- [x] Replace yanked `mysql@28.0.0` with the supported non-yanked patch release.
- [x] Make CI and exact-candidate preparation call one shared supply-chain gate
  and require its exact-SHA artifact result.
- [x] Chunk SQL Server logged-value inserts below the 2,100-parameter ceiling
  without splitting the surrounding transaction.
- [x] Chunk every SQL Server canonical/event/domain statement below 2,100
  parameters and prove that multiple loss/unresolved documents are all
  projected rather than collapsed to the first match.
- [x] Add the MySQL/MariaDB `CHECK(singleton=1)` constraint and validate it on
  both real products.
- [ ] Rerun focused supply-chain, guard, SQL Server, MySQL, and MariaDB tests;
  then rerun the complete real-database gate and exact candidate preparation.
- [ ] Push one frozen replacement, request/read exact-head automatic review,
  collect every GitHub check, and proceed only when the complete ledger is
  green.

## 9. Source-observation and architecture-parity correction

- [x] Record the exact automatic-review P1: initial sink outage left source
  `head_abs` and pending bytes falsely zero because source observation was
  gated by database opening.
- [x] Record the exact GitHub Architecture Safety failure: SQL Server
  `commit_inner` was 202 lines against the enforced 200-line threshold.
- [x] Specify non-consuming source observation during initial open/reconnect
  outage before changing tests or production code.
- [x] Add and run the focused expected-red real-carriage service test; require
  nonzero head/pending during unreachable InfluxDB startup and preservation
  through shutdown.
- [x] Implement an independent shared-memory control observer and rerun the
  identical focused test green.
- [x] Add and run the expected-red release-guard self-test requiring one shared
  Architecture Safety script in GitHub CI and exact-candidate preparation.
- [x] Split SQL Server commit orchestration below the architecture threshold
  without changing transaction, chunk, projection, or checkpoint behavior.
- [x] Preserve the exact expected-red Windows warning-deny failure for the
  Unix-only retry helper, specify its platform boundary, apply the same
  `cfg(unix)` ownership as the worker path, and rerun that cross-target gate.
- [x] Preserve the exact `test-all` infrastructure red where 3,866 tests passed
  but a just-built test executable was concurrently deleted; specify and test
  stable shared-target leases before changing cleanup or guard production code.
- [x] Lease every Cargo-producing exact-candidate command, replace direct
  target deletion with nonblocking lease-aware cleanup, remove glob cleanup
  from `AGENTS.md` and builder skills, and rerun the lease and guard suites
  green.
- [ ] Run the shared Architecture Safety gate, focused SQL Server tests, real
  databases with the real PLC runtime, and every enumerated PR CI command on
  `trust-builder` before freezing another candidate.
- [ ] Commit and prepare one clean exact-SHA artifact, push once, request/read
  the exact-head automatic review, and merge/release only after every check is
  green.

## 10. Fresh shutdown target and exact-label teardown correction

- [x] Preserve the complete exact-head GitHub ledger for `33f3c0407`: all 24
  checks completed with no failures; exact-head security review found no
  security issue.
- [x] Record both exact-head code-review findings: shutdown can freeze an older
  buffered-poll head, and state-file presence can widen cleanup to predictable
  unlabelled Docker names.
- [x] Specify a fresh non-consuming producer-head shutdown snapshot and forbid
  name-based cleanup fallback even when the state marker matches.
- [x] Add focused tests before production changes for a buffered two-record
  poll followed by a third pre-shutdown record, and for empty exact-label
  discovery with matching state and unlabelled predictable names.
- [x] Capture honest assertion red against untouched `33f3c0407`: shutdown
  stored 2 of 3 unique records, while teardown invoked both `docker rm -f` and
  `docker network rm` for names that label discovery did not own.
- [x] Capture focused green for the exact-label teardown test and 20 repeated
  fresh-head shutdown-drain runs without weakening either assertion.
- [x] Keep the repair minimal and owner-specific: the service observer freezes
  the drain target; the teardown script removes only label-discovered IDs; no
  backend adapter, schema generation, TOML shape, or example changes.
- [ ] Run the complete service/runner focused suites, runtime verticals,
  diagram render/drift, architecture/SOLID checks, and final remote broad gates.
- [ ] Freeze, commit, prepare, and push one replacement exact SHA; request and
  read exact-head code/security reviews and collect every GitHub check before
  merge.
- [ ] Merge only through the release guard, verify main CI and `v0.24.68`
  release/Latest/assets/Marketplace, then complete the post-merge audit.
