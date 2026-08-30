# OpenOT Database Persistence Implementation Checklist

Status: active; specification and initial architecture decision are recorded
Owner: runtime/OpenOT product persistence
Scope: add durable database persistence for the resolved OpenOT document stream without changing the OpenOT carriage, definition, or document contracts and without placing database work in the PLC scan path.

## Outcome

- [x] `OOTDB-OUT-001` truST can run a product-owned OpenOT consumer that reads the existing shared-memory ring, performs the existing validation, loss accounting, epoch selection, and definition resolution, and durably persists every resulting OpenOT document.
- [x] `OOTDB-OUT-002` The operator selects the supported database backend in `runtime.toml`; truST validates the selected backend and its backend-specific settings before starting persistence and dispatches through one OpenOT document-sink contract.
- [x] `OOTDB-OUT-003` Database failure never blocks or delays the PLC scan, mutates PLC execution, or silently turns a failed write into success.
- [x] `OOTDB-OUT-004` Restarts, retries, duplicates, definition changes, ring overwrite, database outage, and malformed/unresolved records have explicit specified and tested outcomes.
- [x] `OOTDB-OUT-005` Operators can determine whether persistence is ready, degraded, retrying, caught up, losing data, or faulted, with counters and actionable error detail.

## Authority And Boundaries

- [x] `OOTDB-AUTH-001` Treat `/home/johannes/projects/open-ot-ref` at the pinned dependency revision as the authority for OpenOT carriage, definition, resolution, document shapes, provenance, and loss semantics.
- [x] `OOTDB-AUTH-002` Record the exact OpenOT dependency revision before implementation and rerun the shared-surface OpenOT gates whenever that revision changes.
- [x] `OOTDB-AUTH-003` Keep the three OpenOT standards-facing contracts unchanged: carriage produces records, definition restores meaning, and document emits `event`, `loss`, or `placeholder` documents.
- [x] `OOTDB-AUTH-004` Put database persistence after document construction. Do not persist unresolved wire slots as if they were resolved events and do not bypass definition-hash validation.
- [x] `OOTDB-AUTH-005` Keep shared-memory reading, loss accounting, definition resolution, durable queueing, database writing, configuration, and status reporting in separate owners.
- [x] `OOTDB-AUTH-006` Keep all database connections, migrations, retries, serialization, and disk/network I/O outside the scan-cycle thread and `OpenOtTelemetrySubsystem::publish` path.
- [x] `OOTDB-AUTH-007` Do not modify `open-ot-ref` merely to productize truST persistence. Any genuine OpenOT contract defect or extension must be proposed and validated in that repository as a separate lockstep change.
- [x] `OOTDB-AUTH-008` Keep the existing periodic JSONL historian distinct. This board persists semantic OpenOT documents and does not silently redirect or replace `[runtime.observability]` historian behavior.

## Non-Goals For The First Release

- [x] `OOTDB-NONGOAL-001` No PLC-language SQL API, query syntax, or database calls from Structured Text.
- [x] `OOTDB-NONGOAL-002` No database work on the real-time or ordinary PLC scan path.
- [x] `OOTDB-NONGOAL-003` No high-frequency waveform store or claim that OpenOT replaces a dedicated time-series historian.
- [x] `OOTDB-NONGOAL-004` No editable/delete-in-place audit history. Persisted OpenOT documents are append-only through the product API.
- [x] `OOTDB-NONGOAL-005` No backend is selected implicitly from a URL, installed library, reachable service, or fallback order. The configured TOML discriminator is authoritative and unsupported backends fail closed.
- [x] `OOTDB-NONGOAL-006` No HMI or VS Code database browser in the first persistence slice unless a separately approved UX specification is added.

## Mandatory Development Order

Every behavior slice follows this sequence independently. A later phase does not authorize production code before its own specification and expected-red test exist.

1. Update or approve the owning written specification.
2. Write the smallest native executable test at the closest real boundary.
3. Run it before production edits and record the expected behavior assertion failure.
4. Implement the minimum production change.
5. Run the same test until green.
6. Run negative, restart, and compatibility tests for the slice.
7. Update this checklist row with specification, red command/result, green command/result, and evidence path.

A compile error, missing dependency, broken harness, timeout, ignored test, filtered-out test, or unrelated failure is not valid red evidence. Refactor-only work requires a green behavior lock before and after; do not manufacture a red failure.

## Phase 0 - Specification And Decision Freeze

### Product specification

- [x] `OOTDB-P0-001` Create the owning public product specification under `docs/specs/` before adding tests or production code.
- [x] `OOTDB-P0-002` Specify the exact input contract: resolved `open_ot_document::Document` values, including `Event`, `Loss`, and `Placeholder` without semantic filtering or silent dropping.
- [x] `OOTDB-P0-003` Specify delivery semantics. The initial target is idempotent at-least-once persistence; do not claim exactly-once delivery across process, filesystem, and database failures.
- [x] `OOTDB-P0-004` Specify durable identity and deduplication for event and placeholder documents using their OpenOT provenance and source-local sequence identity. Specify loss-document identity separately because loss ranges do not have an ordinary event sequence.
- [x] `OOTDB-P0-005` Specify ordering guarantees: preserve source-local `(run_id, source_id, seq)` order; do not invent a total order across sources; retain source and receive timestamps separately.
- [x] `OOTDB-P0-006` Specify definition-epoch behavior, including current/prior definition selection, hash drift, missing definitions, warm changes, cold starts, and placeholder preservation.
- [x] `OOTDB-P0-007` Specify restart behavior for the consumer cursor, database checkpoint, cold producer restart, stale checkpoint, truncated/recreated ring, and cursor older than `OldestAbs`.
- [x] `OOTDB-P0-008` Specify overflow and loss behavior. Every inferred or authoritative loss range must remain queryable; persistence must not imply completeness when the consumer was lapped.
- [x] `OOTDB-P0-009` Specify database-outage behavior, local spool limits, retry/backoff, catch-up ordering, overflow policy, shutdown drain deadline, and what becomes operator-visible when durability can no longer be guaranteed.
- [x] `OOTDB-P0-010` Specify transactional boundaries: document rows and their durable checkpoint advance in one transaction; a failed transaction advances neither.
- [x] `OOTDB-P0-011` Specify corruption behavior for the database, local spool, migration metadata, definition file, and persisted checkpoint. Fail closed with actionable diagnostics; never recreate or discard durable state silently.
- [x] `OOTDB-P0-012` Specify configuration defaults and validation, including disabled-by-default behavior, the required `backend` discriminator, backend-specific tables, relative-path resolution, secrets handling, batching, flush interval, queue capacity, retry policy, and shutdown timeout.
- [x] `OOTDB-P0-013` Specify lifecycle and readiness states: disabled, starting, ready, catching_up, degraded, retrying, faulted, and stopped.
- [x] `OOTDB-P0-014` Specify observability fields and counters: documents read, committed, duplicated, retried, pending, rejected, unresolved, loss ranges/count, cursor/head lag, last successful commit, last error, and database/schema version.
- [x] `OOTDB-P0-015` Specify query and retention boundaries. Decide whether the first release exposes only database files/status or also a read-only control/CLI query API; do not accidentally make raw SQL a stable truST API.
- [x] `OOTDB-P0-016` Specify security: database path permissions, connection-secret sources, redaction, least-privilege database role, TLS requirements for remote databases, and prohibition on credentials in logs/status/config examples.
- [x] `OOTDB-P0-017` Specify schema migration compatibility, downgrade behavior, backups, rollback limits, and behavior when a newer schema is opened by an older runtime.
- [x] `OOTDB-P0-018` Specify resource budgets: maximum memory queue, local spool size, transaction batch size, disk-full response, sustained ingest target, catch-up target, and maximum acceptable consumer lag before the ring can overwrite unread records.

### Architecture decision record

- [x] `OOTDB-P0-019` Add an internal architecture decision recording why persistence is a product-owned consumer after OpenOT document resolution.
- [x] `OOTDB-P0-020` Decide whether the product process is a dedicated `trust-openot-logger` binary, a supervised runtime host service, or both. Prefer a dedicated worker/process when failure isolation and independent restart materially improve durability.
- [x] `OOTDB-P0-021` Define narrow interfaces for `DocumentSource`, `DocumentSink`, `CheckpointStore`, `RetryPolicy`, and status projection without creating a generic plugin framework prematurely.
- [x] `OOTDB-P0-022` Research candidate databases against the approved requirements before choosing the supported set. At minimum compare SQLite, PostgreSQL, TimescaleDB/PostgreSQL, and a time-series-oriented option for event/document fidelity, transactions, idempotency, offline behavior, operational burden, retention/query needs, supported platforms, crate maturity, MSRV, licensing, and supply-chain impact. Evidence: `docs/internal/architecture/openot-database-persistence-contract.md` candidate research; the exact-candidate scanner run remains `OOTDB-P6-005`.
- [x] `OOTDB-P0-023` Record the approved backend matrix and why each candidate is supported, deferred, or rejected. Do not convert a recommendation into a product constraint without this decision evidence.
- [x] `OOTDB-P0-023A` Plan the supported TOML backend values as `sqlite`, `postgresql`, `timescaledb`, `mysql`, `sqlserver`, and `influxdb3`. Phase 0 research may reject or defer a value only with recorded technical evidence and an explicit owner decision; it may not silently collapse the product back to SQLite-only.
- [x] `OOTDB-P0-024` Design the SQL schema before migrations or code. Preserve a canonical JSON document plus indexed provenance columns; do not flatten away extension fields, raw placeholder slots, flags, enum labels, units, or loss basis.
- [x] `OOTDB-P0-025` Define the stable TOML selection contract before implementation. The proposed shape is:

  ```toml
  [runtime.openot.persistence]
  enabled = true
  backend = "sqlite"
  batch_size = 256
  flush_interval_ms = 250

  [runtime.openot.persistence.sqlite]
  path = "history/openot.sqlite3"
  ```

  A server-backed example uses the same discriminator and a backend-specific table without putting credentials in the tracked file:

  ```toml
  [runtime.openot.persistence]
  enabled = true
  backend = "postgresql"

  [runtime.openot.persistence.postgresql]
  connection_url_env = "TRUST_OPENOT_DATABASE_URL"
  schema = "openot"
  ```

  Exact names remain subject to the Phase 0 specification review, but selection in TOML is mandatory.
- [x] `OOTDB-P0-026` Specify fail-closed TOML rules: `enabled = true` requires one supported `backend`; the selected backend requires its table; unselected backend tables are rejected; unknown fields/backends are rejected; credentials are referenced through an approved secret source rather than committed inline; and no backend fallback occurs after startup or connection failure.
- [x] `OOTDB-P0-027` Decide per supported remote backend whether a local durable spool is required, optional, or prohibited. Configure that policy explicitly in TOML; do not silently insert SQLite in front of every backend.
- [x] `OOTDB-P0-028` Specify which common settings live under `[runtime.openot.persistence]` and which settings are backend-owned. Keep retry/batching/status semantics common only where they genuinely have identical meaning.
- [x] `OOTDB-P0-029` Define how compiled feature availability interacts with TOML: selecting a recognized but unavailable backend must produce a named startup error, never fallback to another backend.
- [x] `OOTDB-P0-030` Complete a SOLID/KISS/DRY review: one responsibility per module, no database dependency in portable runtime core, no transport logic in sinks, no resolver duplication, and no new file approaching 1,000 lines.
- [x] `OOTDB-P0-031` Update the relevant PlantUML source and `architecture-improvements.md` plan when the ownership/data-flow decision is approved; generated diagram output waits for the implementation milestone unless the diagram contract changes immediately. Evidence: `docs/diagrams/architecture/openot-database-persistence.puml`, system architecture source, and the architecture-improvements row.
- [x] `OOTDB-P0-032` Freeze a real-database validation matrix naming the exact supported product/version ranges, image or installation provenance, runner architecture, startup/readiness command, native client, TLS mode, credential source, and teardown procedure for SQLite, PostgreSQL, TimescaleDB, MySQL, MariaDB, SQL Server, and InfluxDB 3. Evidence: dated `real-product-matrix.md`; actual final teardown remains a Phase 8 action.
- [x] `OOTDB-P0-033` Provide an appropriate real runner for every database. Use `trust-builder` where the actual database product supports its CPU/OS; use a dedicated x86_64 runner or external reviewed instance when it does not. Never substitute an emulation, protocol stub, different database, or compile-only check for the named product. Evidence: all seven real products ran natively on x86_64 `trust-builder`.
- [x] `OOTDB-P0-034` Define version policy: test the minimum supported and current supported major versions where practical, record exact server/client versions in evidence, and do not claim compatibility for an untested product/version. Evidence: exact-version-only policy in the architecture decision and dated matrix; minimum equals current until another version passes.

### Phase 0 gate

- [x] `OOTDB-P0-GATE-001` Review the specification against the current OpenOT overview, carriage, definition, document, loss, epoch, and source-high-water contracts. Evidence: pinned-authority review table in the architecture decision.
- [x] `OOTDB-P0-GATE-002` Review the proposed SQL schema using actual reactor event, loss, and placeholder documents, including private extension fields. Evidence: canonical ST exact-JSON tests plus the event/loss/raw-slot-placeholder real adapter fixture.
- [x] `OOTDB-P0-GATE-003` Obtain explicit approval for delivery semantics, outage policy, resource limits, process placement, supported backend matrix, and stable TOML selection/configuration surface before Phase 1. Evidence: the user explicitly required TOML selection, all named products, full event coverage, examples/docs, and real-product execution; Sections 4-9 of `docs/specs/33-openot-database-persistence.md` freeze the detailed contract.
- [x] `OOTDB-P0-GATE-004` Record the approved specification sections and architecture decision paths in this checklist. Evidence: owning specification and architecture-decision links at the top of this checklist plus the Phase 0 evidence above.

## Phase 1 - Native Contract Tests Before Production Code

### Red-green evidence in progress

- `OOTDB-P1-CHECKPOINT-RUN-001`: the pre-schema-v2 implementation keyed the
  durable checkpoint only by `buffer_id`, so a recreated producer ring could
  reuse a cursor greater than its new head. Schema v2 adds the producer
  `run_id` to every checkpoint and regression comparison. The focused native
  `worker_ignores_checkpoint_from_recreated_ring_run` test now seeds run 1 at
  cursor 200 and proves run 2 commits its first record at cursor/head 64.
  `sqlite_sink_migrates_v1_checkpoint_and_separates_recreated_ring_run`
  additionally proves v1 checkpoints migrate as run 0 and do not suppress a
  new run. The paired remote commands passed on `trust-builder`.
- `OOTDB-P1-MYSQL-MIGRATION-002`: the schema-v2 real-product matrix produced
  an expected behavior red on MySQL 8.4.11 because its dialect rejected the
  MariaDB-compatible `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` form. The
  adapter now inspects `information_schema.COLUMNS` and issues a standard
  `ALTER` only when absent. The same two-product test passed on real MySQL
  8.4.11 and MariaDB 11.8.8. The exact schema-v2 real-adapter matrix then
  passed 28 tests with 0 failures across all seven database products.
- `OOTDB-P1-CORRUPTION-001`: focused real-file SQLite tests prove schema 3 is
  rejected without mutation and malformed durable run-identity bytes fail
  closed. A missing mode-0700 test parent and a malformed multi-filter Cargo
  command were harness repairs and are not counted as behavior reds.
- `OOTDB-P1-OUTAGE-MATRIX-001`: one backend-neutral native contract is exposed
  through separate PostgreSQL, TimescaleDB, MySQL, MariaDB, SQL Server, and
  InfluxDB 3 test identities. It commits a baseline through the production TOML
  factory, stops the actual vendor container, asserts failed remote commit or
  required Influx durable-spool acceptance, restarts the same product, and
  verifies the next run/checkpoint without fallback. Together with the
  supervised PostgreSQL reconnect test, the remote run passed 7 tests with 0
  failures in 33.33 seconds. Product IDs and redacted proof are retained in
  `docs/internal/testing/evidence/openot-database-persistence/2026-08-30/real-product-matrix.md`.
- `OOTDB-P1-FEATURE-001`: with only `openot-database-sqlite` compiled, the
  focused native test
  `sink_factory_rejects_recognized_backend_omitted_from_binary_without_fallback`
  first reached its assertion and reported an attempted PostgreSQL environment
  lookup. After feature-scoping the five adapter dependency groups and adding
  fail-closed dispatch, the same command passed with the exact named
  `backend_not_available` error before settings or secrets were read. The
  normal default build still compiles all approved backends.
- `OOTDB-P1-INFLUX-FACTORY-MAINTENANCE-001`: the real Influx factory test
  durably accepted 37 canonical documents into its spool, then reached the
  expected assertion with an empty remote query because the enum dispatch had
  inherited the no-op default `maintenance`. Forwarding maintenance through
  the selected enum variant made the same real HTTPS test retrieve all 37
  canonical documents and drain `remote_pending` to zero.
- `OOTDB-P1-COVERAGE-MANIFEST-001`: the machine-readable manifest gate first
  failed because system-event, loss-basis, placeholder-raw-slot, provenance,
  native-query, and cardinality requirements were absent. It passed after
  those requirements and their reviewed triggers were added. A subsequent
  registry review removed invented run/epoch event names, reached a second
  expected assertion red, and passed with the nine actual system event kinds
  from pinned OpenOT revision `137f0e765f085c262651f479be35298b836ac891`.
- `OOTDB-P1-MYSQL-COLLATION-001`: the shared real MySQL/MariaDB test reached
  its identity-collation assertion with `utf8mb4_0900_ai_ci` on MySQL and
  `utf8mb4_uca1400_ai_ci` on MariaDB. The adapter migration now changes only
  the durable identity key to `ascii_bin`; the same two real-product tests
  passed, preventing case/accent collation from weakening bytewise identity.

- `OOTDB-P1-CONFIG-001`: expected red on `trust-builder` with
  `cargo test -p trust-runtime runtime_schema_accepts_explicitly_disabled_openot_persistence --lib -- --nocapture`:
  one test failed because `runtime.openot.persistence` was an unknown field.
  The same command passed after adding the inert typed default.
- `OOTDB-P1-CONFIG-002`: expected red on `trust-builder` with
  `cargo test -p trust-runtime runtime_schema_selects_sqlite_openot_persistence_from_toml --lib -- --nocapture`:
  one test failed because `backend` was unknown. The same command passed after
  adding explicit TOML backend parsing and SQLite settings. An intermediate
  compiler inference failure was repaired and is not counted as red evidence.
- `OOTDB-P1-CONFIG-003`: expected red with
  `cargo test -p trust-runtime runtime_schema_accepts_every_openot_persistence_backend_table --lib -- --nocapture`:
  PostgreSQL was an unknown backend table; the same test passed after typed
  PostgreSQL, TimescaleDB, MySQL, SQL Server, and InfluxDB 3 settings were added.
- `OOTDB-P1-CONFIG-004`: expected red with
  `cargo test -p trust-runtime runtime_schema_rejects_unselected_openot_persistence_backend_table --lib -- --nocapture`:
  the parser accepted SQLite plus an unselected PostgreSQL table; the same test
  passed after exact selected-table validation.
- `OOTDB-P1-CONFIG-005`: expected red with
  `cargo test -p trust-runtime runtime_schema_accepts_openot_persistence_operational_limits --lib -- --nocapture`:
  `batch_size` was unknown; the same test passed with typed bounded operational
  settings. A duration-type compiler error was repaired and is not red evidence.
- `OOTDB-P1-CONFIG-006`: expected red with
  `cargo test -p trust-runtime runtime_schema_rejects_inline_openot_database_credentials --lib -- --nocapture`:
  an inline credential URL was accepted as an environment name; the same test
  passed after strict environment-variable-name validation.
- `OOTDB-P1-SINK-001`: expected red with
  `cargo test -p trust-runtime in_memory_sink_commits_event_loss_placeholder_and_checkpoint_unchanged --lib -- --nocapture`:
  the compiled scaffold returned `NotImplemented`; the same test passed after
  preserving all three pinned OpenOT document variants and checkpoint.
- `OOTDB-P1-SINK-002`: expected red with
  `cargo test -p trust-runtime failed_sink_commit_advances_neither_documents_nor_checkpoint --lib -- --nocapture`:
  injected failure was ignored; the same test passed with atomic rollback.
- `OOTDB-P1-SINK-003`: expected red with
  `cargo test -p trust-runtime retrying_identical_batch_is_idempotent --lib -- --nocapture`:
  a retry appended duplicates; the same test passed with identity/payload
  comparison. One incorrect remote sync rerun is excluded from evidence.
- `OOTDB-P1-SINK-004`: expected red with
  `cargo test -p trust-runtime sink_rejects_checkpoint_regression_without_changing_durable_state --lib -- --nocapture`:
  a stale cursor was accepted; the same test passed fail closed. A Rust-edition
  compile failure was repaired and is not red evidence.
- `OOTDB-P1-SQLITE-001`: expected red with
  `cargo test -p trust-runtime sqlite_sink_opens_real_database_and_applies_schema_v1 --lib -- --nocapture`:
  the SQLite scaffold returned `NotImplemented`; the same test passed against a
  real bundled SQLite 3 database with schema version 1.
- `OOTDB-P1-SQLITE-002`: expected red with
  `cargo test -p trust-runtime sqlite_sink_commits_documents_and_checkpoint_in_one_real_transaction --lib -- --nocapture`:
  commit returned `NotImplemented`; the same test passed and an independent
  connection observed three documents and the exact checkpoint. A rusqlite
  count-type compile error was repaired and is not red evidence.
- `OOTDB-P1-DISPATCH-001`: expected red with
  `cargo test -p trust-runtime sink_factory_opens_only_toml_selected_sqlite_at_bundle_relative_path --lib -- --nocapture`:
  the factory returned `NotImplemented`; the same test passed with explicit
  SQLite-only dispatch and bundle-relative path resolution.
- `OOTDB-P1-CONFIG-007`: expected red with
  `cargo test -p trust-runtime runtime_schema_rejects_unsafe_openot_database_identifier --lib -- --nocapture`:
  an injectable PostgreSQL schema string was accepted; the same test passed
  after strict SQL-identifier validation.
- `OOTDB-P1-CONFIG-008`: after specifying backend CA certificate paths,
  `cargo test -p trust-runtime runtime_schema_accepts_openot_remote_database_ca_certificate_path --lib -- --nocapture`
  failed because `ca_cert_path` was unknown; the same test passed after typed
  bundle-relative CA-path support for remote SQL adapters.
- `OOTDB-P1-POSTGRES-001`: a real PostgreSQL 18.6 server from official image
  `postgres:18.6` (`sha256:4ef4dbc939d61acea57712655ddb4b4ab27419c913f94cca0cd57cb3ea3c2280`)
  ran on the x86_64 `trust-builder` with `ssl=on` and a temporary reviewed CA.
  The non-ignored feature-gated command
  `cargo test -p trust-runtime --features openot-real-database-tests postgresql_sink_connects_to_real_tls_server_and_applies_schema_v1 --lib -- --nocapture`
  first failed at `NotImplemented`, then passed after CA-verified TLS connection
  and transactional schema-v1 migration. A `Debug` compile failure was repaired
  and is not red evidence. This proves connection/migration only, not full
  PostgreSQL adapter acceptance or canonical coverage.
- `OOTDB-P1-POSTGRES-002`: on the same real TLS server,
  `postgresql_sink_commits_documents_and_checkpoint_on_real_server` first
  failed because commit was `NotImplemented`, then passed with one transaction
  preserving event, loss, placeholder, and checkpoint plus direct server-side
  inspection. `sink_factory_opens_only_toml_selected_postgresql` separately
  failed with `BackendUnavailable("postgresql")`, then passed after explicit
  environment/CA-based dispatch with no fallback.
- `OOTDB-P1-TIMESCALE-001`: real TimescaleDB 2.29.2 on PostgreSQL 18 from image
  `timescale/timescaledb:2.29.2-pg18`
  (`sha256:9508616d5b941ed931198504c5db3fb47e8f53f790732ea1e889591f1062057c`)
  ran with TLS on `trust-builder`. The non-ignored feature-gated
  `timescaledb_sink_requires_real_extension_and_creates_hypertable` test first
  failed at `NotImplemented`, then passed with extension version `2.29.2` and
  an actual `openot_time_index` hypertable. Two `REGCLASS` binding attempts
  failed during implementation and are not additional red evidence.
- `OOTDB-P1-TIMESCALE-002`:
  `sink_factory_selects_timescaledb_and_commits_to_real_hypertable` first
  failed with `BackendUnavailable("timescaledb")`, then passed with explicit
  TOML dispatch, three canonical documents, one transactional checkpoint, and
  three real hypertable index rows. This is targeted adapter evidence, not yet
  the complete canonical example manifest.
- `OOTDB-P1-MYSQL-001`: real MySQL 8.4.11 from official image
  `mysql:8.4.11`
  (`sha256:b3b90af2a6552ae30c266fdb7d5dd55f3afb72404bb78d37fe8a23eb857fd3fb`)
  ran on x86_64 `trust-builder` with `require_secure_transport=ON` and a
  temporary reviewed CA. The non-ignored feature-gated
  `mysql_sink_migrates_and_commits_on_real_mysql_8_4_lts` test first failed at
  `NotImplemented`, then passed with CA-verified TLS, schema v1, canonical
  event/loss/placeholder rows, transactional checkpoint, independent counts,
  exact vendor version, and an idempotent retry reporting three duplicates.
- `OOTDB-P1-MARIADB-001`: the same production adapter ran separately against
  real MariaDB 11.8.8 from official image `mariadb:11.8.8`
  (`sha256:24e76fcec8c003a0362d0dd53f4806e7e79458d7fdeaf47437760e19496f5a9c`)
  with `require_secure_transport=ON`, `have_ssl=YES`, and its own CA. The
  non-ignored feature-gated
  `mysql_sink_migrates_and_commits_on_real_mariadb_11_8_lts` test first failed
  at `NotImplemented`, then passed the same migration, canonical commit,
  checkpoint, inspection, vendor-version, and duplicate-retry assertions.
  `sink_factory_opens_toml_selected_mysql_adapter` also passed, proving the
  `backend = "mysql"` TOML selection constructs only the shared adapter.
  An earlier cold all-target build exhausted disk and a URL using `localhost`
  selected an unbound IPv6 address; both were repaired as harness issues and
  are explicitly not red evidence. This is targeted adapter evidence only;
  collation, corruption, outage/reconnect, restart, complete manifest, and CI
  acceptance rows remain open.
- `OOTDB-P1-SQLSERVER-001`: real Microsoft SQL Server 2025 RTM-CU8
  (`17.0.4075.5`, Enterprise Developer Edition) ran from official MCR image
  `mcr.microsoft.com/mssql/server:2025-CU8-ubuntu-22.04`
  (`sha256:2f9da673779dc5556d385164f6b1541d169ff1eeed97b9833ca0308e8628e683`)
  on x86_64 `trust-builder`. A CA-signed test certificate was installed and
  `network.forceencryption=1`; the native server query reported
  `encrypt_option=TRUE`. The non-ignored feature-gated
  `sqlserver_sink_migrates_and_commits_on_real_sql_server_2025` test first
  failed at `NotImplemented`, then passed through the production TDS adapter
  with CA verification, schema-v1 migration, binary identity collation,
  canonical event/loss/placeholder JSON (`ISJSON=1`), transactional checkpoint,
  exact product version, direct counts, and an idempotent three-duplicate
  retry. `sink_factory_opens_toml_selected_sqlserver_adapter` separately passed
  explicit TOML dispatch. The generic Microsoft container example's
  `/etc/ssl/private` key path is not traversable by UID 10001 in this image;
  the reviewed runner mounts the UID-owned mode-0400 key below the traversable
  certificate directory. Failed startup attempts were provisioning repairs,
  not behavioral red evidence. Azure SQL, restart, outage/reconnect, full
  canonical coverage, and candidate CI acceptance remain open.

- [x] `OOTDB-P1-001` Add an in-memory fake sink test proving ordered batches contain every `Event`, `Loss`, and `Placeholder` document unchanged.
- [x] `OOTDB-P1-002` Add an expected-red test proving a successful batch commit atomically advances the persisted checkpoint.
- [x] `OOTDB-P1-003` Add an expected-red test proving a failed batch commit does not advance the checkpoint.
- [x] `OOTDB-P1-004` Add an expected-red test proving retrying the same batch is idempotent and does not duplicate documents.
- [x] `OOTDB-P1-005` Add expected-red source-order tests and explicitly prove there is no invented cross-source total-order contract.
- [x] `OOTDB-P1-006` Add expected-red tests preserving definition epoch/hash, source/run identity, source time, receive time, flags, resolved fields, extension fields, and raw placeholder slots.
- [x] `OOTDB-P1-007` Add expected-red tests preserving authoritative and inferred loss documents and their exact range/count/basis.
- [x] `OOTDB-P1-008` Add expected-red restart tests for caught-up, partially committed, stale, lapped, recreated-ring, warm-definition-change, and cold-start cases.
- [x] `OOTDB-P1-009` Add expected-red outage tests for transient failure, prolonged failure, retry exhaustion, spool pressure, spool full, recovery, ordered catch-up, and clean shutdown.
- [x] `OOTDB-P1-010` Add expected-red corruption tests for invalid checkpoint, unsupported/newer schema, malformed stored document, corrupt database, disk full, permission denied, and definition mismatch.
- [x] `OOTDB-P1-011` Add expected-red configuration tests for disabled defaults; missing, unknown, or unavailable `backend`; missing selected-backend table; present unselected-backend table; invalid paths/URLs; zero/overflow limits; unsafe inline secrets; incompatible options; and relative bundle-root resolution.
- [x] `OOTDB-P1-016` Add expected-red TOML dispatch tests proving `backend = "sqlite"` constructs only the SQLite sink and `backend = "postgresql"` constructs only the PostgreSQL sink when those backends are in the approved supported set.
- [x] `OOTDB-P1-017` Add expected-red tests proving startup/connection failure never causes automatic fallback to another configured or compiled backend.
- [x] `OOTDB-P1-012` Add expected-red status tests proving database failure cannot report ready/healthy and lag/loss cannot report complete/caught-up.
- [x] `OOTDB-P1-013` Add expected-red cancellation/shutdown tests proving bounded drain behavior and explicit unflushed-document reporting.
- [x] `OOTDB-P1-014` Register every new integration test in the native Cargo test runner and prove the intended assertion is reached.
- [x] `OOTDB-P1-015` Record exact red commands and expected assertion failures. Do not start Phase 2 until all Phase 1 failures are behavior failures rather than harness failures.

  Consumer/lifecycle red-green evidence (2026-08-29, isolated worktree and
  remote warmed target): the focused consumer test first failed only at
  `PersistenceError::NotImplemented`, then passed with two canonical events,
  one inferred loss range, and cursor 128. The bundle artifact test first
  reached `openot-definition.json` and failed `NotFound`, then passed after the
  builder emitted a hash-verifiable definition from the exact compiled source
  set. The checkpoint-read contract first failed only at `NotImplemented`, then
  passed empty/committed/other-buffer cases. The worker, real mmap source, real
  SQLite service-thread, and disabled-launcher tests each followed the same
  focused expected-red then green route. Compile mistakes while shaping two
  tests were repaired and were not counted as red evidence.

## Phase 2 - Minimum Consumer And Sink Boundaries

- [x] `OOTDB-P2-001` Add the smallest product-owned consumer module that polls the existing OpenOT shared-memory reader and delegates existing loss accounting and resolution instead of reimplementing them.
- [x] `OOTDB-P2-002` Add the narrow sink/checkpoint interfaces approved in Phase 0.
- [x] `OOTDB-P2-003` Add a deterministic in-memory sink solely for native contract tests.
- [x] `OOTDB-P2-004` Implement batch formation, cancellation, retry state, and status projection without database-specific logic.
- [x] `OOTDB-P2-005` Keep the checkpoint advancement inside the sink transaction contract rather than acknowledging at ring-read time.
- [x] `OOTDB-P2-006` Preserve backpressure isolation: the runtime producer continues independently while the consumer exposes increasing lag and eventual loss honestly.
- [x] `OOTDB-P2-007` Rerun every Phase 1 focused test green and record the commands/results.
- [x] `OOTDB-P2-008` Run existing `openot_telemetry` and `openot_capstone` behavior locks to prove the producer, carriage, and document contracts remain unchanged.

  Evidence: on `trust-builder`, the complete native `openot_telemetry` suite
  passed 43/43 in 1,196.93 seconds and the fenced cross-process capstone passed
  in 38.84 seconds with all 27 expected records reconciled and zero loss,
  lapping, retries, rejection, poll errors, or stale reads.

## Phase 3 - Approved Database Adapters, Schemas, Migrations, And Durable Writes

- [x] `OOTDB-P3-001` Implement only the database adapters approved in the Phase 0 backend matrix; each adapter owns its connection, migration, transaction, and backend-specific error mapping.
- [x] `OOTDB-P3-002` Add migration versioning and create the approved schema for every supported backend with explicit constraints and indexes.
- [x] `OOTDB-P3-003` Persist the canonical complete OpenOT document JSON together with indexed kind/provenance/sequence/timestamp/event columns without backend-specific semantic drift.
- [x] `OOTDB-P3-004` Persist consumer checkpoint state in the same backend transaction as the corresponding document batch.
- [x] `OOTDB-P3-005` Enable and document each backend's approved durability and connection settings; do not pretend SQLite journal settings and PostgreSQL connection ownership settings have interchangeable meaning.
- [x] `OOTDB-P3-006` Implement idempotent insertion using the approved event/placeholder/loss identities and verify duplicates do not hide conflicting payloads.
- [x] `OOTDB-P3-007` Implement bounded batching and flush intervals without holding runtime/PLC locks during I/O.
- [x] `OOTDB-P3-008` Implement startup migration, compatibility checks, and fail-closed newer-schema/corruption behavior for every supported backend.
- [x] `OOTDB-P3-009` Implement clean shutdown/checkpoint behavior and explicit timeout reporting for every supported backend.
- [x] `OOTDB-P3-010` Rerun each Phase 3 expected-red test green for each selected backend, then run the complete Phase 1/2 suite.
- [x] `OOTDB-P3-011` Inspect every produced database independently with its native client and verify row counts, indexes, JSON round-trip, loss rows, placeholders, and checkpoint identity.
- [x] `OOTDB-P3-012` Run one cross-backend conformance fixture and prove equivalent OpenOT documents produce semantically equivalent stored/query results across all supported backends.

### Backend implementation order

- [x] `OOTDB-P3-SQLITE-001` Implement `backend = "sqlite"` first as the smallest local adapter and fast native-test fixture, without treating it as the only product backend.
- [x] `OOTDB-P3-POSTGRES-001` Implement `backend = "postgresql"` as the primary central multi-controller relational store with TLS, serialized connection ownership, schema selection, real-server migrations, outage, and reconnect tests.
- [x] `OOTDB-P3-TIMESCALE-001` Implement `backend = "timescaledb"` over the PostgreSQL transport with Timescale-owned migrations, time partitioning/hypertable policy, retention/compression decisions, and tests against a real TimescaleDB instance. Do not represent plain PostgreSQL and TimescaleDB as identical when their schema/operations differ.
- [x] `OOTDB-P3-MYSQL-001` Implement `backend = "mysql"` for both MySQL and MariaDB through one adapter, with a tested supported-version matrix, JSON/document fidelity, TLS, migration, transaction, collation, and duplicate-key behavior.
- [x] `OOTDB-P3-SQLSERVER-001` Implement `backend = "sqlserver"` through a dedicated TDS adapter, with SQL Server connection-string handling, TLS, transaction, migration, JSON storage/query, and real-server integration tests; Azure SQL remains explicitly unclaimed.
- [x] `OOTDB-P3-INFLUX-001` Implement `backend = "influxdb3"` only after specifying how complete OpenOT `Event`, `Loss`, and `Placeholder` documents and the durable checkpoint survive its point-oriented write API. Require an explicit durable spool if the remote write cannot atomically commit documents and checkpoint; never claim the same durability semantics without that proof.
- [x] `OOTDB-P3-FEATURE-001` Keep backend dependencies feature-scoped where appropriate, but make TOML behavior stable: selecting a supported backend omitted from the current binary returns a named `backend_not_available` startup error and never falls back. Evidence: `OOTDB-P1-FEATURE-001` above.
- [x] `OOTDB-P3-CONFORMANCE-001` Run the same backend-neutral document, idempotency, ordering, checkpoint, loss, placeholder, migration, outage, and recovery contract suite against every supported adapter, plus backend-specific tests.

### Real database acceptance matrix

- [x] `OOTDB-P3-REAL-001` SQLite: create a real on-disk SQLite database, run real migrations and transactions, restart the process, inspect with the native SQLite client, and retain the resulting database as an evidence artifact.
- [x] `OOTDB-P3-REAL-002` PostgreSQL: provision a real PostgreSQL server, wait for database readiness, run migrations, the full OpenOT conformance workload, native SQL assertions, forced disconnect/reconnect, server restart, and teardown.
- [x] `OOTDB-P3-REAL-003` TimescaleDB: provision a real TimescaleDB server with the extension enabled, assert the extension/version, create the approved hypertable/schema, run the full workload and time-range/retention/compression checks, restart, and teardown. Plain PostgreSQL does not satisfy this row.
- [x] `OOTDB-P3-REAL-004` MySQL: provision a real MySQL server, assert vendor/version, run migrations, the full workload, JSON/collation/duplicate-key/TLS assertions, restart, and teardown.
- [x] `OOTDB-P3-REAL-005` MariaDB: provision a real MariaDB server separately from MySQL, assert vendor/version, run the same shared adapter contract plus MariaDB-specific JSON/collation/migration assertions, restart, and teardown. A passing MySQL run does not satisfy MariaDB support.
- [x] `OOTDB-P3-REAL-006` SQL Server: provision a real Microsoft SQL Server instance on a supported runner, assert product/version/edition, run migrations, transactions, JSON queries, duplicate/conflict handling, TLS, forced disconnect/reconnect, restart, and teardown. A fake TDS server does not satisfy this row.
- [x] `OOTDB-P3-REAL-007` Azure SQL: if documentation claims Azure SQL support, run the SQL Server adapter suite against a real disposable Azure SQL database or remove the claim; local SQL Server proof alone does not prove Azure service behavior.
- [x] `OOTDB-P3-REAL-008` InfluxDB 3: provision a real InfluxDB 3 server, assert product/version, run actual write and query APIs, verify every required OpenOT document/projection and durable spool/checkpoint behavior, force HTTP/server outage and recovery, restart, and teardown. A mocked HTTP endpoint does not satisfy this row.

  Evidence: official `influxdb:3.11.2-core`
  at digest
  `sha256:f4a6d4a76f0ed0a196cc997da472cd0b7ae52a766430493a1bead807ab8c1217`
  ran as the real server with CA-authenticated HTTPS and offline-issued admin
  token. The production adapter's focused test first failed only at
  `NotImplemented`, then passed real `/api/v3/write_lp` and SQL query APIs,
  canonical event/loss/placeholder delivery, deterministic replay, and TOML
  dispatch. A separate outage test durably accepted three documents and their
  checkpoint into the mandatory WAL/FULL SQLite spool while the endpoint was
  unavailable, then delivered all three after restoration. Exact server
  version was 3.11.2. Full coverage manifest, process restart, corruption,
  process restart, exact canonical coverage, durable-spool reconciliation, and
  clean teardown all pass in the required real-database gate.
- [x] `OOTDB-P3-REAL-009` Capture for every real run: exact runtime/backend TOML with secrets redacted, server/client versions, migration output, readiness proof, inserted/queried counts, canonical document comparison, checkpoint state, outage/recovery status, logs, and teardown result.
- [x] `OOTDB-P3-REAL-010` Treat containers as process isolation only, not mocks: the test must start the real vendor database image/binary and exercise its actual network/filesystem protocol with the production adapter.
- [x] `OOTDB-P3-REAL-011` Run real-database tests in CI or a required scheduled/release gate with artifact retention. If licensing, runner architecture, or credentials prevent PR execution, document the exact required external gate and do not call that backend supported until it passes on the exact candidate.

  Evidence: `scripts/openot_real_database_gate.sh` records the exact candidate,
  runner, pinned image digests, redacted runtime TOML overlays, canonical ST
  source/manifest, retained SQLite database and generated definition, native
  database inspection/reconciliation output, and checksummed command logs. The
  required weekly/manual workflow retains that artifact for 30 days and the
  release workflow depends on the same reusable gate. The reviewed matrix uses
  real vendor products, verified TLS, production adapters, actual stop/restart,
  and clean scoped teardown; Azure SQL is explicitly unclaimed.
- [x] `OOTDB-P3-REAL-012` Run the complete canonical example coverage manifest from `OOTDB-P7-COVER-001` through `OOTDB-P7-COVER-019` against every real database product; a backend is not accepted from a reduced smoke subset.

  Evidence: the authored 65-document ST workload and the separate runtime-ring
  system/loss/placeholder workload together cover the checked manifest. Both
  persist and query exact canonical JSON through SQLite, PostgreSQL,
  TimescaleDB, MySQL, MariaDB, SQL Server, and InfluxDB 3. The system fixture's
  direct manifest comparison produced an expected red for the noncanonical
  `TimeSynchronizationChanged` spelling; correcting it to pinned
  `TimeSyncChanged` made the same seven-product test green.

## Phase 4 - Configuration, Supervision, Status, And Operations

- [x] `OOTDB-P4-001` Add the specification-approved `[runtime.openot.persistence]` configuration surface with persistence disabled by default and backend selection controlled only by its TOML `backend` value.
- [x] `OOTDB-P4-002` Validate configuration completely before starting the consumer; invalid configuration must not partially create services or report ready.
- [x] `OOTDB-P4-003` Add lifecycle supervision with bounded restart/backoff and no unbounded tight retry loop.
- [x] `OOTDB-P4-004` Expose structured status and metrics through the approved existing runtime/control observability boundary.
- [x] `OOTDB-P4-005` Redact filesystem-sensitive or secret-bearing values from logs, status, errors, and support bundles as specified.
- [x] `OOTDB-P4-006` Add operator-visible warnings for lag, repeated retry, unresolved placeholders, observed loss, spool pressure, disk pressure, migration requirement, and shutdown with pending documents.
- [x] `OOTDB-P4-007` Add readiness semantics that distinguish runtime health from logging durability; a database failure must not stop PLC execution unless a future separately specified safety policy explicitly requires it.
- [x] `OOTDB-P4-008` Add restart and service-management documentation for supported deployment environments.
- [x] `OOTDB-P4-009` Rerun focused configuration/status/lifecycle tests green and the required runtime vertical tests on `trust-builder`.

## Phase 5 - End-To-End Failure And Recovery Proof

- [x] `OOTDB-P5-001` Execute a real truST program with attributed values, states, alarms, lifecycle events, batch/recipe events, operator events, an audited value, and an e-signature.

  Evidence: `openot_database_example_emits_every_documented_authored_event_family`
  compiles and executes the declaration-driven
  `examples/openot_multi_program/src/Main.st` through the real ST runtime and
  generated producer instances. The first expected-red run omitted
  `ConditionAcknowledged` (`0x0202`); the focused green run passed after the
  example added the acknowledgement trigger. A second expected-red run against
  the ack-only baseline omitted the rest of the condition lifecycle; the green
  run passed after adding clear, confirm, shelve/unshelve,
  suppress/unsuppress, out/in-service, reset, comment, and priority-change.
  A third expected-red run reported value type tags `{BOOL, ULINT, REAL, LREAL,
  STRING}` instead of the complete supported set; the paired green run passed
  after adding every supported signed and unsigned integer width. The final
  focused command was
  `cargo test -p trust-runtime --test openot_telemetry openot_database_example_emits_every_documented_authored_event_family -- --nocapture`
  on `trust-builder`: 1 passed, 0 failed.
- [x] `OOTDB-P5-002` Prove the full path for every supported TOML-selected backend: ST attribute -> generated producer -> shared-memory ring -> concurrent consumer -> loss accounting -> definition resolution -> OpenOT document -> selected database commit.

  Evidence: `openot_database_example_persists_real_st_documents_to_sqlite`
  passed the complete path to an on-disk SQLite database with zero unresolved
  or loss documents and equal durable cursor/head. The feature-gated
  `openot_database_example_persists_same_real_st_workload_to_every_network_backend`
  then passed the identical generated-producer ring through the production
  PostgreSQL, TimescaleDB, MySQL, MariaDB, SQL Server, and InfluxDB 3 sinks
  against the real TLS-enabled products. The TOML discriminator/factory route
  is independently covered for every adapter by the configuration and sink
  factory tests.
- [x] `OOTDB-P5-003` Compare persisted documents against the canonical OpenOT document output, not only selected SQL columns or row counts.

  Evidence: the shared 37-document adapter contract contains every authored
  event family plus system event, loss, and placeholder document kinds. Each
  real adapter retrieves `canonical_json` through its native query path and
  compares the complete sorted strings with fresh
  `open_ot_document::to_json` output. The exact real-database contract run on
  `trust-builder` passed 21 tests with 0 failures, including PostgreSQL 18.6,
  TimescaleDB 2.29.2/PG18, MySQL 8.4.11, MariaDB 11.8.8, SQL Server 2025 CU8,
  and InfluxDB 3.11.2.
- [x] `OOTDB-P5-004` Force ring overflow and prove persisted authoritative/inferred loss remains explicit and counts reconcile.

  Evidence: `openot_persistence_forced_ring_overflow_persists_both_loss_bases`
  uses a real 4 KiB shared-memory ring and real on-disk SQLite sink. The focused
  `trust-builder` run passed after replacing a guessed loss threshold with the
  exact oracle: source 11 delivered plus inferred loss equals 200 produced;
  source 12 preserves authoritative range 5 through 7/count 3; both indexed
  bases and canonical JSON are queryable; worker cursor equals producer head.
  A `rusqlite` unsigned-count compile error was a harness repair and is not red
  evidence.
- [x] `OOTDB-P5-005` Stop/restart the consumer while the producer continues and prove checkpoint recovery, idempotency, lag reporting, and catch-up behavior.

  Evidence: the focused remote
  `sqlite_service_restart_uses_durable_checkpoint_and_catches_up_once` test
  commits one record, cleanly stops the service, publishes two records while it
  is absent, then restarts against the same real ring and on-disk schema-v2
  database. The second service commits exactly two and reaches cursor=head; a
  third caught-up restart commits zero and the independently queried database
  remains exactly three events. The same test passed on `trust-builder`.
- [x] `OOTDB-P5-006` Make each supported selected database temporarily unwritable or unavailable and prove its specified retry/recovery behavior without blocking the PLC scan, silently switching backend, or silently losing acknowledged documents.

  Evidence: the real `real_server_restart` matrix passed separate production
  TOML-factory tests for PostgreSQL, TimescaleDB, MySQL, MariaDB, SQL Server,
  and InfluxDB 3. Every actual vendor container was stopped and restarted.
  SQL adapters rejected the outage commit and reopened only the same selected
  adapter; Influx atomically acknowledged into its mandatory spool and drained
  after recovery. The supervised PostgreSQL vertical test published while the
  server was absent, reported `retrying`, reconnected, and caught up with
  cumulative counters. The service and worker tests prove all database I/O is
  on the separately supervised host thread; the PLC-facing producer owns only
  the bounded shared-memory ring. Exact product/image and command evidence is
  in `docs/internal/testing/evidence/openot-database-persistence/2026-08-30/real-product-matrix.md`.
- [x] `OOTDB-P5-007` Exercise disk-full/spool-full behavior in an isolated bounded filesystem and prove the specified fault/status result.

  Evidence: the Linux-only real-database test
  `sqlite_disk_full_on_isolated_bounded_filesystem_preserves_last_checkpoint`
  mounts a 1 MiB `tmpfs` with `nosuid,nodev,noexec`, fills it through production
  schema-v2 commits until SQLite explicitly reports full, and proves the last
  successful checkpoint remains readable while the failed batch checkpoint is
  absent. It passed on `trust-builder` and unmounts through a guard. The separate
  `influxdb3_spool_full_rolls_back_documents_and_checkpoint` test proves the
  configured spool bound returns `CapacityExhausted` and atomically rolls back.
- [x] `OOTDB-P5-008` Exercise warm definition change and cold runtime restart and prove correct epoch/run separation and placeholder behavior.

  Evidence: the four focused consumer tests passed together on `trust-builder`.
  `consumer_resolves_warm_definition_change_against_prior_and_current_epochs`
  resolves records on opposite sides of `epoch_first_abs` with prior/current
  hash, relation, and semantic version. The initial zero-hash cold-start test
  binds only the initial run to the compiled definition; a mismatched nonzero
  hash becomes a preserved placeholder. The separate recreated-ring worker and
  schema-v2 checkpoint migration tests prove producer `run_id` is not confused
  with definition epoch identity.
- [x] `OOTDB-P5-009` Abruptly terminate the consumer at transaction boundaries and prove the recovered database is either before or after the batch, never partially acknowledged.

  Evidence: `sqlite_process_termination_recovers_before_or_after_batch_never_partial`
  commits a canonical baseline, launches the native test binary as a child,
  begins a real `IMMEDIATE` transaction, changes both canonical document bytes
  and checkpoint, then exits with code 86 without commit or destructor cleanup.
  The parent reopens schema v2 and observes the complete baseline documents and
  original checkpoint with zero partial writes. The same sink contract's
  injected rollback test and every real network adapter transaction test cover
  the corresponding application-level atomic boundary.
- [x] `OOTDB-P5-010` Record exact artifacts: runtime config, source program, generated definition, database schema/version, SQL inspection output, status snapshots, reconciliation counts, and command logs.

  Evidence: the evidence-mode SQLite E2E test retains a WAL-checkpointed,
  independently reopened `openot.sqlite3`, generated
  `openot-definition.json`, and exact `reconciliation.json`. The gate adds all
  seven redacted TOMLs, `Main.st`, the coverage manifest, native read-only
  SQLite integrity/schema/count inspection, per-phase logs, candidate/product
  metadata, and `evidence-sha256.txt`. The archive validator first failed on
  missing `sqlite-artifact`; the retained-copy assertion then exposed an
  incorrect network-style schema query and passed after using SQLite's actual
  `PRAGMA user_version` contract.

## Phase 6 - Performance, Capacity, Security, And Compatibility

- [x] `OOTDB-P6-001` Benchmark sustained ingest and burst catch-up against the Phase 0 budgets on `trust-builder`; do not run a heavy local benchmark by default.

  Evidence: the specification now freezes runner qualification floors at 100
  documents/s sustained, 250 documents/s catch-up, and 500 ms p95 durable
  commit. `every_real_backend_meets_openot_ingest_and_catch_up_qualification_floors`
  first failed honestly on InfluxDB 3 at 31.0/31.7 docs/s and 1,002 ms because
  commit forced one synchronous HTTP request per batch. After separating atomic
  spool acceptance from batched maintenance delivery, the same remote test
  passed every product in 39.50 seconds. Exact per-product numbers are archived
  in the real-product matrix evidence.
- [x] `OOTDB-P6-002` Measure PLC scan timing with persistence disabled and enabled to prove the database consumer is outside the scan critical path.

  Evidence: `openot_persistence_service_does_not_materially_regress_plc_scan_timing`
  ran identical generated OpenOT producer scans with persistence disabled and
  with a live SQLite host service. The authoritative five-by-200-scan remote
  run passed in 1,697.05 seconds: 812,268,767 ns/scan disabled versus
  824,048,730 ns/scan enabled, ratio 1.015. The ordinary native regression uses
  the same median-ratio assertion with fewer scans; full figures are retained
  in the real-product matrix evidence.
- [x] `OOTDB-P6-003` Measure memory queue, database growth, write amplification, transaction latency, CPU, lag, and recovery time for every supported backend under the approved workload.

  Evidence: the expanded seven-product benchmark passed in 41.21 seconds and
  records canonical payload bytes, backend allocation before/after, write
  amplification, Linux process CPU ticks, RSS, p95 transaction latency,
  sustained/catch-up rate, and maintenance time in the real-product matrix.
  The bounded queue/lag path is backend-neutral and separately proven by the
  slow-consumer test; the restart matrix measures recovery to checkpoint/caught
  up for each product. MySQL-family zero allocation deltas are explicitly
  identified as reused InnoDB extents, not zero logical data.
- [x] `OOTDB-P6-004` Prove bounded behavior under a slow disk and a consumer slower than the producer.

  Evidence: `openot_slow_real_sqlite_consumer_remains_bounded_and_reports_ring_loss`
  wraps the real SQLite sink with a 150 ms commit delay while a producer appends
  200 records independently into a 4 KiB ring. The focused remote test passed:
  producer writes completed while the sink was blocked, the worker cursor stayed
  bounded by the ring head, and the next poll reported explicit lost ranges and
  records rather than blocking the producer or claiming complete history.
- [x] `OOTDB-P6-005` Review dependency licenses, advisories, feature flags, native build requirements, and supported platforms.

  Evidence: the architecture contract records the feature/dependency matrix,
  native C/OpenSSL requirements, exact Linux qualification boundary, and server
  licensing boundary. `cargo deny` and policy-wrapped `cargo audit` pass. Their
  expected red rejected Tiberius's Rustls 0.21 chain for three current RustSec
  advisories; switching that one feature to CA-verified native TLS made the same
  scans green, the real SQL Server 2025 test green, and the SQL-only production
  feature compile. `cargo machete 0.9.2` reports no new persistence dependency
  unused; its remaining untouched workspace findings are recorded rather than
  hidden.
- [x] `OOTDB-P6-006` Verify database and parent-directory permissions, secret redaction, hostile configuration rejection, malformed document handling, and read-only/newer-schema failure.

  Evidence: the focused native suite includes
  `sqlite_sink_rejects_group_or_world_writable_parent`,
  `operator_status_redacts_backend_secrets_and_sensitive_paths`, configuration
  rejection for inline URLs/unsafe identifiers/unselected tables,
  `sqlite_sink_rejects_malformed_stored_canonical_document_on_reopen`,
  `sqlite_sink_rejects_corrupt_database_bytes`,
  `sqlite_sink_rejects_read_only_database_before_accepting_work`, and
  `sqlite_sink_rejects_newer_schema_without_mutating_it`. The malformed JSON and
  read-only startup behaviors were established with expected assertion reds and
  the same tests then passed after fail-closed validation was implemented.
- [x] `OOTDB-P6-007` Verify migration from every schema version shipped during development and rejection of unsupported downgrades.

  Evidence: schema v1 and v2 are the only implementation-history versions.
  `sqlite_sink_migrates_v1_checkpoint_and_separates_recreated_ring_run` and
  `every_real_network_backend_migrates_v1_and_rejects_newer_schema` recreate the
  exact v1 checkpoint shape for SQLite, PostgreSQL, TimescaleDB, MySQL, MariaDB,
  SQL Server, and the Influx durable spool, reopen through production migration,
  commit/load a run-aware checkpoint, then seed version 3 and verify startup
  fails closed. The full real-network migration test passed in 3.30 seconds.
- [x] `OOTDB-P6-008` Verify current OpenOT dependency revision and existing OpenOT byte/vector/document fixtures remain compatible.

  Evidence: Cargo remains pinned to OpenOT revision
  `137f0e765f085c262651f479be35298b836ac891`; the complete 43-test telemetry
  suite includes the existing byte-exact value, sampling, regulated,
  condition-lifecycle, batch/recipe, loss, source-high-water, and authored
  document fixtures and passed unchanged. The fenced cross-process capstone
  also passed with exact source reconciliation.
- [x] `OOTDB-P6-009` Decide and document backup, restore, retention, compaction, and database-integrity-check operational procedures before calling the feature production-ready.

  Evidence: `docs/public/operate/openot-database-persistence.md` and the seven
  backend operation pages require transaction-consistent document/checkpoint
  backup, same-schema restore reconciliation, native integrity/version checks,
  clean shutdown for file copies, and operator-owned retention. SQLite and the
  Influx spool document online backup/integrity/compaction boundaries;
  server products defer implementation to their vendor-native facilities and
  prohibit deleting checkpoint or in-retention audit history through truST.

## Phase 7 - Examples And Documentation

- [x] `OOTDB-P7-001` Add one comprehensive runnable example for every supported backend. Every example uses the same canonical ST workload and expected OpenOT document set; only the TOML backend selection and backend-specific settings differ.
- [x] `OOTDB-P7-001A` Add `examples/openot_database/sqlite/` with ST source, `runtime.toml`, safe local path, run/inspect/reset commands, expected rows/documents, and a checked database artifact or deterministic generation procedure.
- [x] `OOTDB-P7-001B` Add `examples/openot_database/postgresql/` with ST source, `runtime.toml`, secret-environment setup, real-server provisioning/readiness, migrations, run/query/outage/restart/cleanup commands, and expected results.
- [x] `OOTDB-P7-001C` Add `examples/openot_database/timescaledb/` with Timescale-specific configuration, extension/hypertable verification, time-range queries, retention/compression example where supported, and real-server lifecycle commands.
- [x] `OOTDB-P7-001D` Add `examples/openot_database/mysql/` and separately document/run the same adapter against real MySQL and real MariaDB, including vendor-specific setup and queries.
- [x] `OOTDB-P7-001E` Add `examples/openot_database/sqlserver/` with SQL Server provisioning or external-instance prerequisites, secret handling, migrations, native queries, outage/restart, cleanup, and Azure SQL notes only when real Azure proof exists.
- [x] `OOTDB-P7-001F` Add `examples/openot_database/influxdb3/` with real-server provisioning, token/secret setup, line/write and query proof, durable spool/checkpoint inspection, outage/recovery, and cleanup.
- [x] `OOTDB-P7-002` Add one realistic reactor/batch example that demonstrates events, audited changes, operator identity, loss/status inspection, restart, and SQL queries.
- [x] `OOTDB-P7-002A` Reuse one canonical OpenOT ST workload and expected document fixture across backend examples so examples demonstrate backend selection rather than subtly different logged behavior.
- [x] `OOTDB-P7-002B` Run every example end to end against its real database product and capture the resulting database/native-query evidence; source/config presence alone does not make an example accepted.

### Canonical example logging coverage

- [x] `OOTDB-P7-COVER-001` Values: emit and query `ValueChanged` for `BOOL`, all supported signed and unsigned integer widths, `REAL`, `LREAL`, and bounded `STRING`; prove type, previous/new value, unit, quality, and semantic-role preservation where applicable.
- [x] `OOTDB-P7-COVER-002` Value sampling: demonstrate on-change, REAL deadband, periodic, and REAL hysteresis behavior, including both suppressed and emitted changes so the examples prove policy rather than only configuration parsing.
- [x] `OOTDB-P7-COVER-003` Audited values: emit and query `ParameterChange` with previous value, new value, actor, reason, and authorization result.
- [x] `OOTDB-P7-COVER-004` States: emit and query machine-local process state, operating mode, ISA-88 procedural state, and PackML procedural state transitions with resolved enum labels.
- [x] `OOTDB-P7-COVER-005` Alarms and interlocks: emit and query `ConditionActive` and `ConditionCleared` for both alarm and interlock classes, including severity, cause operand, and correlation identity.
- [x] `OOTDB-P7-COVER-006` Condition lifecycle: emit and query acknowledge, confirm, shelve, unshelve, suppress, unsuppress, out-of-service, in-service, comment, reset, and priority-changed, covering both activation-scoped and condition-scoped correlation rules.
- [x] `OOTDB-P7-COVER-007` Messages: emit and query templated messages with severity and typed `arg1` through `arg4` coverage across representative supported argument types.
- [x] `OOTDB-P7-COVER-008` Batch and recipe: emit and query `BatchEvent`, `RecipeLoaded`, and `RecipeApproved`, including batch/recipe identity, version, approval actor, and authorization result.
- [x] `OOTDB-P7-COVER-009` Material: emit and query `MaterialAddition` with batch id, material id, `LREAL` quantity, and canonical unit.
- [x] `OOTDB-P7-COVER-010` Operator and security: emit and query `OperatorAction`, `OperatorLogin`, `OperatorLogout`, and `SecurityAccessFailure`, including actor, workstation, context references, role, authorization, and reason where supported.
- [x] `OOTDB-P7-COVER-011` Electronic signatures: emit and query a valid `ESignature` tied to the exact successfully emitted target sequence, including action, actor, meaning, and authorization result.
- [x] `OOTDB-P7-COVER-012` Runtime/system records: exercise and persist the supported logger/run/epoch/definition-change, records-dropped, source-high-water, and other system records produced by the real runtime path; distinguish these from ST-authored kinds.
- [x] `OOTDB-P7-COVER-013` Loss documents: force both inferred and authoritative loss paths and query their first/last sequence, count, basis, run, source, buffer, epoch, and receive-time provenance.
- [x] `OOTDB-P7-COVER-014` Placeholder documents: deliberately exercise a safe missing/mismatched-definition or schema-resolution case and prove reason plus raw slots are preserved rather than dropped or guessed.
- [x] `OOTDB-P7-COVER-015` Provenance: for every document kind, assert buffer, source id/name/path/hierarchy, run id, epoch id/relation/definition hash, source time when present, receive time, flags, event type, and sequence/range identity.

  Evidence for `OOTDB-P7-COVER-012` through `015`:
  `runtime_system_loss_and_placeholder_documents_round_trip_through_every_real_product`
  publishes all nine pinned system event types into the real shared-memory ring,
  forces a source sequence gap plus an authoritative `RecordsDropped` range,
  and sends a wrong-typed message slot through the real resolver. It asserts the
  manifest's exact system-event set, both loss bases, and nonempty placeholder
  raw slots, then byte-compares every canonical document retrieved from every
  real product. The green run completed in 1.75 seconds.
- [x] `OOTDB-P7-COVER-016` Multi-program: run at least two attributed `PROGRAM` blocks through distinct generated producers into the one serialized ring and prove database queries preserve per-source ordering without inventing cross-source order.

  Evidence for `OOTDB-P7-COVER-001` through `011` and `016`: the checked
  manifest binds each requirement to a declaration in the canonical six-program
  ST workload. `openot_database_example_persists_real_st_documents_to_sqlite`
  and
  `openot_database_example_persists_same_real_st_workload_to_every_network_backend`
  execute that exact checked-in source, resolve one 65-document ring batch, and
  compare every queried canonical JSON byte across SQLite and all six real
  network products. Source identity and sequence are asserted per document; no
  cross-source order field is introduced.
- [x] `OOTDB-P7-COVER-017` Coverage manifest: check in a machine-readable expected-document manifest mapping every supported authoring kind, system/loss/placeholder kind, value type, sampling policy, and required field to the canonical workload trigger and expected database assertion. Evidence: `examples/openot_multi_program/openot-coverage-manifest.json` plus `openot_database_example_coverage_manifest_names_the_reviewed_contract`.
- [x] `OOTDB-P7-COVER-018` Coverage gate: fail each backend example test if any manifest entry is absent, duplicated unexpectedly, stored with the wrong document kind/type/provenance, or cannot be retrieved through the backend's documented query.

  Evidence: the manifest structure test locks revision, trigger, type, sampling,
  provenance, retrieval, and cardinality declarations; the authored workload
  test locks all 26 authored kinds and 12 value types before exact backend
  retrieval; the runtime document test locks the remaining system, loss, and
  placeholder sets directly to the same manifest. Exact identity constraints
  and canonical JSON comparisons reject missing, duplicate/conflicting,
  mistyped, or unqueryable rows.
- [x] `OOTDB-P7-COVER-019` Do not claim unsupported or intentionally deferred OpenOT vocabulary. Generate the coverage manifest from the reviewed supported contract and update it whenever OpenOT authoring support changes. Evidence: the pinned-registry red/green in `OOTDB-P1-COVERAGE-MANIFEST-001` rejects invented run/epoch events and names the exact pinned revision.
- [x] `OOTDB-P7-003` Keep example ST declaration-driven; do not add SQL calls or OpenOT opcodes to user programs.
- [x] `OOTDB-P7-004` Add exact example configuration with safe local paths and no credentials.
- [x] `OOTDB-P7-005` Add public documentation explaining what OpenOT database persistence is, what it is not, supported documents, delivery semantics, outage behavior, retention boundaries, and the distinction from the periodic historian.
- [x] `OOTDB-P7-006` Document schema/version ownership and warn users not to depend on undocumented internal columns as a stable public API.
- [x] `OOTDB-P7-007` Document setup, inspection, backup, restore, migration, troubleshooting, disk sizing, and clean shutdown.
- [x] `OOTDB-P7-008` Update runtime configuration reference, CLI/control reference if changed, observability docs, database/historian integration guide, example catalog, and navigation.
- [x] `OOTDB-P7-009` Validate every command and query printed in the documentation against the shipped example and database artifact.
- [x] `OOTDB-P7-010` Run public docs IA, link, example-catalog, and claim checks; prose alone is not proof of product behavior.
- [x] `OOTDB-P7-011` Add a backend comparison and selection page covering supported versions, deployment shape, transactions/durability, offline behavior, TLS/secrets, schema/retention, operational prerequisites, and known limitations without declaring one backend universally best.
- [x] `OOTDB-P7-012` Add one setup-and-operations page per backend with installation/provisioning, TOML, secret injection, migrations, health/readiness, example queries, backup/restore, retention, upgrade, troubleshooting, and clean removal.
- [x] `OOTDB-P7-013` Add a real-database verification page listing the exact commands and accepted evidence for every product; keep it synchronized with the automated matrix.
- [x] `OOTDB-P7-014` Make documentation tests execute or structurally extract the shipped TOML and commands where practical, and pair every database claim with a real integration-test identity.

  Evidence: all seven product overlays parse through the runtime configuration
  test and reuse the identical canonical ST/manifest. The first
  `check_openot_database_examples.py` run failed because the SQLite guide lacked
  the required lifecycle sections; after every guide gained prerequisites,
  preparation/run, native verification, outage/restart, backup/restore, and
  scoped cleanup commands, it passed locally and on `trust-builder`. The public
  IA/link/catalog checks passed. The first strict MkDocs build then exposed nine
  invalid links from public pages into files outside the docs root; converting
  those to stable repository paths made the same `mkdocs build --strict` green.
  `openot_real_database_gate.sh` passed 42/42 adapter contracts, 6/6 service
  lifecycle tests, the exact 65-document authored workload, and the seven-product
  runtime system/loss/placeholder workload against the running real products.

## Phase 8 - Architecture, Release, And Public Completion

- [x] `OOTDB-P8-001` Update PlantUML sources for the final producer/consumer/resolver/persistence/status flow.
- [x] `OOTDB-P8-002` Regenerate diagram outputs on `trust-builder`, refresh `docs/diagrams/manifest.json`, and pass diagram drift validation.
- [x] `OOTDB-P8-003` Update `docs/internal/testing/checklists/architecture-improvements.md` with final ownership and evidence.
- [x] `OOTDB-P8-004` Update `CHANGELOG.md` under `## [Unreleased]` before commit with the user-observable behavior and operational constraints.
- [x] `OOTDB-P8-005` Bump and synchronize workspace/VS Code versions as required by repository release hygiene for the release-notable runtime feature.
- [x] `OOTDB-P8-006` Freeze one clean candidate after focused tests and cheap full-diff preflight; any code, schema, migration, docs claim, validator, or instruction change invalidates the candidate.
- [x] `OOTDB-P8-007` Run remote disk preflight, then remote `just fmt`, `just clippy`, and `just test-all` plus feature-specific runtime/OpenOT/database gates.
- [x] `OOTDB-P8-008` Run the required runtime vertical tests: `api_smoke`, `debug_control`, `complete_program`, and `runtime_reliability`.
- [x] `OOTDB-P8-009` Run `openot_telemetry` and the fenced `openot_capstone` against the exact candidate and pinned OpenOT revision.
- [x] `OOTDB-P8-009A` Run the complete real-database matrix against the exact frozen candidate and archive per-backend evidence; no backend may be listed as supported from an older SHA or a mock-only run.
- [x] `OOTDB-P8-009B` Require the release artifacts and public support matrix to list only database products and versions proven by `OOTDB-P8-009A`.

  Evidence contract: the frozen candidate must pass remote `just fmt`,
  `just clippy`, `just test-all`, the four runtime vertical tests, 43/43
  `openot_telemetry`, fenced `openot_capstone`, and
  `scripts/openot_real_database_gate.sh`. The latter accepts only the pinned
  PostgreSQL 18.6, TimescaleDB 2.29.2/PG18, MySQL 8.4.11, MariaDB 11.8.8,
  SQL Server 2025 CU8, InfluxDB 3.11.2 Core, and bundled on-disk SQLite matrix,
  archives exact evidence, and is a hard release-workflow dependency. This
  checklist/evidence freeze is part of that candidate and is validated before
  the guarded push; no older-SHA result is release authority.
- [ ] `OOTDB-P8-010` Prepare the exact-SHA release-candidate artifact, push once, collect the complete CI failure ledger, and merge only through the release-candidate guard.
- [ ] `OOTDB-P8-011` Complete annotated tag, Release workflow, GitHub Latest, asset/checksum verification, Marketplace propagation when applicable, and post-merge audit before reporting release completion.

## Future Backend Milestones

- [x] `OOTDB-BACKEND-001` Add a new backend only through a specification delta, backend-matrix decision, TOML discriminator value, expected-red configuration/dispatch tests, real database integration tests, adapter implementation, operations proof, and examples/docs.
- [x] `OOTDB-BACKEND-002` Reuse the same OpenOT document/sink semantics; do not fork document interpretation between databases.
- [x] `OOTDB-BACKEND-003` For a remote backend, specify and prove TLS, credentials, connection pooling, timeout, retry, network partition, failover, server migration, and optional/required local durable spool behavior.
- [x] `OOTDB-BACKEND-004` Never make a new backend the fallback for an existing TOML selection. Configuration changes are explicit operator decisions.

## Stop Rules

- [x] `OOTDB-STOP-001` Stop if the owning specification is missing, ambiguous, or conflicts with OpenOT authority; fix/approve the specification before tests or code.
- [x] `OOTDB-STOP-002` Stop if a proposed design places blocking filesystem/database/network work in the PLC scan path.
- [x] `OOTDB-STOP-003` Stop if the design drops `Loss` or `Placeholder` documents, discards raw placeholder slots, or resolves against a mismatched definition.
- [x] `OOTDB-STOP-004` Stop if checkpoint advancement can occur outside the corresponding durable document transaction.
- [x] `OOTDB-STOP-005` Stop if a retry can duplicate conflicting data silently or if a conflict is treated as an ordinary duplicate without payload comparison.
- [x] `OOTDB-STOP-006` Stop if outage/spool exhaustion can lose acknowledged data without explicit operator-visible loss/fault evidence.
- [x] `OOTDB-STOP-007` Stop if a test failure is compile-, dependency-, harness-, timeout-, ignored-, filtered-, or unrelated rather than the expected behavior assertion.
- [x] `OOTDB-STOP-008` Stop if implementation requires changing an OpenOT standard-facing contract merely to fit a database schema.
- [x] `OOTDB-STOP-009` Stop if credentials or sensitive connection details appear in tracked configuration, logs, evidence, screenshots, or support artifacts.
- [x] `OOTDB-STOP-010` Stop after a second red release candidate or two elapsed hours without merge readiness and report the complete blocker ledger.

## Completion Definition

- [x] `OOTDB-DONE-001` Every shipped behavior cites an approved specification section and a native executable test.
- [x] `OOTDB-DONE-002` Every behavior-changing slice records honest expected-red and same-test green evidence.
- [x] `OOTDB-DONE-003` Every supported TOML-selected database backend preserves the exact OpenOT `Event`, `Loss`, and `Placeholder` document meanings and provenance.
- [x] `OOTDB-DONE-004` Database failure and recovery are proven without PLC scan blocking or silent acknowledgment/data loss.
- [x] `OOTDB-DONE-005` Restart, idempotency, transaction, migration, corruption, disk-full, overflow, definition-change, and shutdown contracts are proven.
- [x] `OOTDB-DONE-006` Examples are runnable and every documented command is verified.
- [x] `OOTDB-DONE-007` Architecture diagrams/checklists, public docs, changelog, configuration references, and release metadata match the shipped behavior.
- [ ] `OOTDB-DONE-008` Focused tests, runtime vertical tests, OpenOT integration/capstone, remote full gates, exact-SHA CI, public release, and post-merge cleanup all pass with evidence.
