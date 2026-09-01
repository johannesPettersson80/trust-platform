# OpenOT database initial-schema corrective checklist

Owner specification: `docs/specs/33-openot-database-persistence.md`, especially
Section 4.8.

This checklist supersedes every completed v1/v2/v3/v4 migration, rename,
backfill, and reconstruction row in the older persistence checklists. Database
persistence has not shipped, so those development histories are not product
compatibility requirements. They must be removed, not repaired.

## 1. Specification and design freeze

- [x] State that every backend and the InfluxDB spool use schema generation 1.
- [x] Define empty-target initialization, exact-generation validation, and
  fail-closed handling of incompatible pre-release targets.
- [x] Prohibit migration, rename, backfill, rebuild, downgrade, and automatic
  repair paths in the initial release.
- [x] Preserve the shared public typed logging model and backend-specific
  physical mappings without treating them as different schema generations.
- [x] Review failure classification: reachability is retryable; malformed
  configuration, TLS verification, authentication, schema, and storage errors
  are permanent.
- [x] Confirm SOLID/KISS/DRY ownership: one projector, one generation constant,
  adapter-owned DDL/binding only, no backend-owned semantic projection.

## 2. Focused native tests before production changes

- [x] Add an expected-red contract that every adapter reports generation 1.
- [x] Replace legacy migration tests with expected-red tests proving an empty
  target creates the complete final schema directly.
- [x] Add expected-red tests proving markerless legacy objects, non-1 markers,
  and incomplete generation-1 layouts fail without modification.
- [x] Add PostgreSQL expected-red tests proving malformed DSNs and TLS
  verification failures fault immediately while typed I/O reachability retries.
- [x] Preserve behavior-lock tests for canonical fidelity, typed projections,
  atomic checkpoints, duplicate replay, loss, placeholders, and status.

## 3. Minimal production implementation

- [x] Use one shared schema-generation constant with value 1.
- [x] Delete all legacy migration, rename, backfill, replay, reconstruction,
  downgrade, and test-only migration-seeding implementation.
- [x] Initialize final SQLite, PostgreSQL, TimescaleDB, MySQL/MariaDB, SQL
  Server, and InfluxDB spool schemas directly and validate exact compatibility.
- [x] Enforce marker value 1 in every backend DDL; use transactional DDL where
  supported and write the marker last where it is not.
- [x] Fail closed without mutation for incompatible pre-release state.
- [x] Classify PostgreSQL no-SQLSTATE errors by typed source, never diagnostic
  text, so I/O reachability retries and configuration/TLS failures do not.
- [x] Rename migration-specific operator wording and warning codes to schema
  compatibility wording.

## 4. Examples, operator documentation, and architecture evidence

- [x] Keep TOML backend selection and cross-platform path behavior unchanged;
  use descriptive `trust_logging` namespaces and `trust-logging` filenames in
  shipped examples and retained evidence.
- [x] Keep every backend example querying values, alarms, messages, states,
  batches, recipes, materials, operator/audit/signature/system events, loss,
  unresolved records, and canonical integrity without JSON extraction.
- [x] Document generation 1 and pre-release database recreation for every
  backend; remove upgrade/migration instructions.
- [x] Update changelog, architecture contract, diagrams, manifest, and drift
  checks for the initialization flow.

## 5. Real-product and real-runtime proof

- [x] On the remote builder, initialize fresh real SQLite, PostgreSQL,
  TimescaleDB, MySQL, MariaDB, SQL Server, and InfluxDB 3 targets.
- [x] Run the real PLC conformance program and verify canonical documents plus
  every typed public table/measurement on every product.
- [x] Prove restart/idempotency/checkpoint behavior on every product.
- [x] Prove incompatible pre-release targets remain byte/row unchanged after
  rejection on every practical product boundary.
- [x] Prove unreachable PostgreSQL retries and malformed DSN, bad TLS, auth,
  and schema failures fault without retry loops.
- [x] Publish observed throughput/latency and retain teardown evidence.

## 6. Frozen candidate and release

- [x] Run focused tests and cheap full-diff preflight, then freeze the code.
- [ ] Run remote `just fmt`, `just clippy`, `just test-all`, runtime vertical
  tests, cross-platform path/compile gates, example validator, docs/diagram
  gates, supply-chain gates, and exact release-candidate guard.
- [x] Perform a second line-by-line spec-to-test-to-code audit after all code
  changes; resolve the complete finding ledger before pushing.
- [ ] Push the exact artifact-bound SHA once; wait for every GitHub check and
  every automatic review before editing any failure.
- [ ] Merge only through the exact-SHA guard when all findings are resolved.
- [ ] Verify main CI, annotated tag, Release workflow, GitHub Latest, assets,
  checksums, and Marketplace propagation where applicable.
- [ ] Run the post-merge audit until clean.
