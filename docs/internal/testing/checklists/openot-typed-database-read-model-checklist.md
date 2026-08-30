# OpenOT Typed Database Read Model Implementation Checklist

Status: implementation complete; exact-candidate release publication awaits authorization
Owner: truST runtime logging persistence
Owning specification: `docs/specs/33-openot-database-persistence.md`
Architecture decision: `docs/internal/architecture/openot-database-persistence-contract.md`

## Execution Evidence

- The projector/schema slices retain their expected-red and paired-green
  commands in the parent implementation checklist. The final live-counter
  slice reached an assertion red because `projection_rows_committed` reported
  zero while the SQLite public tables contained two rows; the same focused
  remote test then passed with an exact count of two, and the complete status
  projection module passed 26 tests.
- The real adapter contract suite passed 47 tests on the six network products
  (one release-only performance test ignored in debug). The same authored ST
  workload passed through the production runtime path for SQLite and all six
  network products; system, loss, and unresolved documents also passed the
  seven-product round trip.
- Release-profile qualification passed the specified floors on all products:
  SQLite 298.4/2686.4 docs/s, PostgreSQL 298.4/1048.4, TimescaleDB
  298.4/714.2, MySQL 298.4/933.1, MariaDB 298.4/1032.3, SQL Server
  298.4/606.7, and InfluxDB 3 164.9/306.1. All recorded p95 commit latencies
  were below 500 ms.
- `scripts/check_openot_database_examples.py`, the authored-event workload,
  the coverage manifest, public IA/links/assets/search checks, strict MkDocs,
  rendered PlantUML, and diagram drift all pass on `trust-builder`.

## Required Workflow

The implementation order is:

1. specification and acceptance examples;
2. architecture, schema, and migration freeze;
3. smallest native expected-red test for one observable behavior;
4. minimum production code for that behavior;
5. same focused test green, then negative/restart tests;
6. repeat steps 3 through 5 one behavior slice at a time;
7. runnable examples and user documentation;
8. real PLC runtime against every real supported database;
9. security, performance, compatibility, broad gates, and release proof.

A compile error, missing dependency, broken harness, timeout, ignored test, or
unrelated failure is not valid red evidence. Code MUST NOT be written for a
behavior until its specification is approved and its native test has reached
the expected assertion failure. A behavior-preserving refactor uses a green
behavior-lock test before and after instead of manufacturing a red result.

## Phase 0 - Specification And Acceptance Freeze

- [x] `OTRM-SPEC-001` Freeze the truST/OpenOT ownership boundary: OpenOT owns the resolved input document; truST owns configuration, projection, schema, migration, durability, and public database names.
- [x] `OTRM-SPEC-002` Freeze all stable public and internal database objects from specification Section 4.
- [x] `OTRM-SPEC-003` Require common value, alarm, message, state, and audit queries to work without JSON paths, OpenOT field-array knowledge, EAV pivots, or `openot_*` public names.
- [x] `OTRM-SPEC-004` Freeze typed value lanes, exact values, source/receive timestamps, provenance, quality, audit fields, and lossless full-range `ULINT` behavior.
- [x] `OTRM-SPEC-005` Freeze known-event classification, fail-closed malformed-known-event behavior, relational atomicity, TimescaleDB hypertables, InfluxDB reconciliation, and schema-v2-to-v3 reconstruction.
- [x] `OTRM-SPEC-GATE` Review representative expected table rows for every logging domain and IEC value type before test authoring begins.

## Phase 1 - Architecture, Schema, And Migration Design

- [x] `OTRM-ARCH-001` Define one backend-neutral `LoggingProjector` consuming typed documents plus exact hash-matched definition metadata and producing canonical, envelope, and domain rows; missing/mismatched historical definitions fail closed.
- [x] `OTRM-ARCH-002` Keep adapters responsible only for DDL, native type binding, transactions, errors, and product verification; prohibit adapter-owned JSON parsing or semantic dispatch.
- [x] `OTRM-ARCH-003` Freeze exact columns, nullability, keys, constraints, indexes, and native types for SQLite, PostgreSQL, MySQL, MariaDB, and SQL Server.
- [x] `OTRM-ARCH-004` Freeze TimescaleDB hypertables, partition/chunk/index policy, retention interaction, and uniqueness constraints.
- [x] `OTRM-ARCH-005` Freeze InfluxDB measurements, tags, typed fields, deterministic point identities, delivery parts, reconciliation, and retention interaction.
- [x] `OTRM-ARCH-006` Define an idempotent interruption-safe schema-v2-to-v3 migration using the same projector as new writes.
- [x] `OTRM-ARCH-007` Define least-privilege reporting roles: public read-model `SELECT`, no public mutation, and no required internal-table access.
- [x] `OTRM-ARCH-008` Update PlantUML ownership/data flow and pass SOLID/KISS/DRY review, with no new large mixed-responsibility module.
- [x] `OTRM-ARCH-GATE` Approve schema examples and migration rollback/recovery procedure before production migrations.

## Phase 2 - Native Tests Before Production Code

Each row requires a specification section, test path/name, expected-red
command/result, implementation commit, and same-test green command/result.

- [x] `OTRM-TEST-001` Project every known OpenOT event ID to the correct public object and fields.
- [x] `OTRM-TEST-002` Project every supported IEC type into exactly one typed lane and preserve its exact representation.
- [x] `OTRM-TEST-003` Preserve extrema including `ULINT#18446744073709551615` without float conversion or rounding.
- [x] `OTRM-TEST-004` Preserve provenance, source/receive timestamps, quality, flags, and definition identity.
- [x] `OTRM-TEST-005` Project audited parameter changes consistently into `logged_values` and `audit_log` without duplicating canonical events.
- [x] `OTRM-TEST-006` Retain future unknown events; reject malformed known events and missing/mismatched referenced definitions before checkpoint advancement.
- [x] `OTRM-TEST-007` Prove documented value, alarm, message, state, batch, operator, and audit queries without JSON functions.
- [x] `OTRM-TEST-008` Prove duplicate replay is idempotent and conflicting payloads fault visibly.
- [x] `OTRM-TEST-009` Prove relational canonical rows, projections, and checkpoints commit or roll back as one unit.
- [x] `OTRM-TEST-010` Prove v2-to-v3 backfill, interruption/restart, malformed-canonical failure, newer-schema rejection, and canonical preservation.
- [x] `OTRM-TEST-011` Prove TimescaleDB hypertables, typed inserts, filters, time-window queries, and migrations.
- [x] `OTRM-TEST-012` Prove InfluxDB typed measurements, deterministic points, partial-write recovery, per-part reconciliation, retry, and spool exhaustion.
- [x] `OTRM-TEST-013` Prove TOML selects exactly one backend, uses the specified names, and never falls back.
- [x] `OTRM-TEST-GATE` Every production behavior has a valid expected assertion red or justified green behavior lock before Phase 3 edits.

## Phase 3 - Incremental Implementation

- [x] `OTRM-CODE-001` Implement the backend-neutral row model and `LoggingProjector` in small single-responsibility modules.
- [x] `OTRM-CODE-002` Implement SQLite schema v3, typed writes, migration, exact unsigned storage, and indexes.
- [x] `OTRM-CODE-003` Implement PostgreSQL schema v3, typed writes, migration, native types, constraints, and indexes.
- [x] `OTRM-CODE-004` Implement TimescaleDB typed hypertables and product verification over PostgreSQL transport.
- [x] `OTRM-CODE-005` Implement shared MySQL/MariaDB transport with separately verified DDL, types, collation, and migration behavior.
- [x] `OTRM-CODE-006` Implement SQL Server schema v3, typed bindings, transactions, constraints, and migration.
- [x] `OTRM-CODE-007` Implement InfluxDB measurements plus durable per-part spool/reconciliation.
- [x] `OTRM-CODE-008` Extend status with projection, unclassified-event, reconciliation, pending-part, schema, and redacted failure state.
- [x] `OTRM-CODE-009` Keep diagrams, architecture checklist, specification references, and migration documentation synchronized.
- [x] `OTRM-CODE-GATE` All focused tests, malformed cases, rollback, restart, and duplicate tests pass before examples begin.

## Phase 4 - Runnable Examples

- [x] `OTRM-EXAMPLE-001` Provide one real ST program that logs every supported IEC value type and changes values during execution.
- [x] `OTRM-EXAMPLE-002` Emit alarms, messages, states, batches, recipes, material additions, operator activity, audited changes, signatures, system events, loss, and unresolved records.
- [x] `OTRM-EXAMPLE-003` Provide runnable TOML for SQLite, PostgreSQL, TimescaleDB, MySQL, MariaDB, SQL Server, and InfluxDB 3 without embedded credentials.
- [x] `OTRM-EXAMPLE-004` Provide native queries for each product showing readable tables without normal-query JSON extraction.
- [x] `OTRM-EXAMPLE-005` Show outage/recovery, restart/catch-up, backup/restore, migration, retention, and safe teardown per product.
- [x] `OTRM-EXAMPLE-GATE` Validate every command mechanically and compare logical results across products.

## Phase 5 - User And Operator Documentation

- [x] `OTRM-DOC-001` Document TOML selection, defaults, secret environment variables, TLS, paths, and fail-closed errors.
- [x] `OTRM-DOC-002` Document every public object/column with type, nullability, meaning, example, index, and compatibility promise.
- [x] `OTRM-DOC-003` Explain internal canonical versus public read-model ownership and that users normally do not query JSON.
- [x] `OTRM-DOC-004` Explain SQLite unsigned values, TimescaleDB hypertables, and InfluxDB tags/fields without changing the logical domain model.
- [x] `OTRM-DOC-005` Document sizing, retention, backup/restore, migration, corruption recovery, least privilege, TLS, credential rotation, monitoring, and capacity alerts.
- [x] `OTRM-DOC-006` Include table-form native-client output showing actual alarms, messages, and typed values.
- [x] `OTRM-DOC-GATE` Pass link, navigation, example, strict documentation-build, and claim-to-native-test checks.

## Phase 6 - Real Runtime And Real Database Proof

Mocks, wire-compatible substitutes, compile-only checks, and direct fixture
insertion do not satisfy this phase. The same built truST candidate and real PLC
program MUST drive every database through the production runtime path.

- [x] `OTRM-REAL-001` Freeze one candidate and record local/remote HEAD, OpenOT revision, toolchains, exact server/client versions, provenance, runner architecture, and TLS mode.
- [x] `OTRM-REAL-002` Run the real ST program through `trust-runtime`; prove scans publish OpenOT records and the production worker persists them.
- [x] `OTRM-REAL-003` Query real SQLite and verify all expected canonical, public, checkpoint, typed-value, alarm, message, loss, and unresolved rows.
- [x] `OTRM-REAL-004` Repeat the complete runtime/query proof on real PostgreSQL.
- [x] `OTRM-REAL-005` Repeat on real TimescaleDB and verify actual hypertables/time-window queries.
- [x] `OTRM-REAL-006` Repeat separately on real MySQL and real MariaDB; never infer one from the other.
- [x] `OTRM-REAL-007` Repeat on real Microsoft SQL Server.
- [x] `OTRM-REAL-008` Repeat on real InfluxDB 3 and verify measurements, native field types, point counts, and drained reconciliation state.
- [x] `OTRM-REAL-009` For every product, stop the database while the PLC continues, restart it, prove catch-up/no silent loss, then prove runtime restart and duplicate replay.
- [x] `OTRM-REAL-010` Migrate populated real schema-v2 databases, verify v3 projections/canonical preservation, and perform backup/restore verification.
- [x] `OTRM-REAL-011` Compare expected and actual identities, counts, exact values, timestamps, provenance, loss ranges, and domain fields with a machine-readable oracle.
- [x] `OTRM-REAL-012` Archive redacted commands, logs, queries, table-form output, versions, candidate SHA, checksums, and scoped teardown evidence.
- [x] `OTRM-REAL-GATE` All seven named products pass the same real-runtime acceptance contract on the exact candidate SHA.

## Phase 7 - Nonfunctional And Release Gates

- [x] `OTRM-NFR-001` Measure sustained ingest, catch-up, latency, queue/spool growth, CPU, memory, disk, and database size against specified budgets.
- [x] `OTRM-NFR-002` Test disk/spool full, network partition, TLS/certificate failure, bad credentials, permissions, corruption, and shutdown deadline behavior.
- [x] `OTRM-NFR-003` Run license, advisory, unsafe, unused-dependency, architecture-doctor, diagram-drift, and supply-chain gates.
- [x] `OTRM-NFR-004` Run remote runtime vertical tests: `api_smoke`, `debug_control`, `complete_program`, and `runtime_reliability`.
- [x] `OTRM-NFR-005` After remote disk preflight, run remote `just fmt`, `just clippy`, and `just test-all` on the frozen candidate.
- [x] `OTRM-NFR-006` Update changelog, synchronized versions, public support matrix, examples, docs, and release artifacts.
- [x] `OTRM-NFR-006A` Prove production persistence code and every shipped example use native platform path handling, reject Unix-only path literals, and compile the complete database feature set for the Windows target.
  Evidence: the example contract first failed on `unix:///tmp/trust-openot-multi-program.sock`, then passed after all seven examples moved to portable TCP control endpoints. `RUSTFLAGS=-Dwarnings cargo check --target x86_64-pc-windows-gnu -p trust-runtime --features openot-database-all` first failed because `open-ot-carriage` was Unix-scoped, then passed after the portable carriage dependency moved to the common dependency set while `open-ot-shm` remained Unix-only and enabled persistence continued to fail closed on non-Unix hosts.
- [ ] `OTRM-NFR-007` Prepare the exact-SHA candidate, push once, await all CI, guarded-merge, and complete tag/release/latest/assets/post-merge proof when authorized.

## Completion And Stop Rules

- [ ] `OTRM-DONE-001` Specification, tests, code, examples, docs, real proof, nonfunctional gates, and release evidence describe the same candidate.
- [x] `OTRM-DONE-002` Users retrieve common PLC values, alarms, messages, and audit history from descriptive typed objects without OpenOT/JSON expertise.
- [x] `OTRM-DONE-003` Every supported database has exact-version evidence produced by a real PLC program through the production runtime.

Stop and return to the owning earlier phase if a common query requires canonical
JSON/EAV/`openot_*`, behavior lacks a pre-code native test, adapters duplicate
OpenOT semantics, relational projection is non-atomic, typed values lose
precision, Influx partial delivery can be acknowledged, a mock substitutes for
real-runtime proof, or the candidate changes after freeze.
