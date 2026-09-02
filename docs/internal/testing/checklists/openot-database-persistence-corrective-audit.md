# OpenOT Database Persistence Corrective Audit

Status: active; release blocked
Owner: runtime/OpenOT product persistence
Authority: `docs/specs/33-openot-database-persistence.md`
Trigger: post-merge review of candidate `041a08d86991158683c7f0303af9e780f2b2f0b3`

This board audits the complete implementation after release review found that
the accepted candidate violated normative lifecycle and projection contracts.
No existing checked row is accepted as proof by itself. A row closes only with
the owning specification, an executable assertion at the closest boundary,
and current evidence from the corrected exact candidate.

The initial-schema correction is now owned by
`openot-database-initial-schema-corrective-checklist.md`. References below to
migration or different backend schema versions are historical evidence from
unreleased development builds and do not define the product contract.

## Confirmed release blockers

- [x] `OOTDB-AUDIT-P1-001` Remote database unavailability during initial open
  does not fail PLC startup. The supervised worker owns connection, migration,
  retry, and recovery from the first attempt onward.
- [x] `OOTDB-AUDIT-P1-002` A continuously busy worker performs bounded sink
  maintenance after successful non-empty commits so the InfluxDB delivery
  spool cannot starve while new records keep arriving.
- [x] `OOTDB-AUDIT-P1-003` InfluxDB `logged_values` contains the complete typed
  read model: current and previous typed lanes, both exact values, audit flag,
  actor, reason, and authorization result.
- [x] `OOTDB-AUDIT-P1-004` Shutdown drains records accepted before the shutdown
  request and required remote delivery until caught up or the configured
  deadline. Setting the stop flag must not skip the final poll or maintenance.
- [x] `OOTDB-AUDIT-P1-005` Reconnect preserves every cumulative status counter,
  including read, committed, duplicated, retried, rejected, unresolved, loss,
  projection, unclassified, and reconciled-part counters.
- [ ] `OOTDB-AUDIT-P1-006` The mandatory real-database job has a registered
  runner and uses repository-owned, tested provisioning and teardown. A release
  cannot be tagged while its required runner label has no online runner.
- [x] `OOTDB-AUDIT-P1-007` Completion state is internally consistent. The
  dedicated board said implementation was complete while the umbrella board
  still had implementation, examples, diagrams, full gates, and done rows open;
  neither board may authorize release until their evidence agrees.
- [x] `OOTDB-AUDIT-P1-008` Real InfluxDB proof enforces token authentication as
  well as CA-verified TLS. Supplying a token to a server that accepts anonymous
  writes does not prove the normative authenticated-connection contract.
- [x] `OOTDB-AUDIT-P1-009` Tag `v0.24.67` is not publishable: it points at the
  defective merge and its release is queued on a nonexistent runner. Cancel the
  incomplete release and publish only a new version containing the corrected
  exact candidate.
- [x] `OOTDB-AUDIT-P1-010` InfluxDB projection parity covers every documented
  column, not merely measurement existence. The current state, batch, recipe,
  material, operator, audit, signature, and unresolved encoders omit stable
  fields that their backend-neutral rows and relational adapters retain.
- [x] `OOTDB-AUDIT-P1-011` Common provenance is lossless on every public row.
  The current unresolved projection discards source path/hierarchy and actual
  record flags, while SQLite manufactures empty paths and true flags in its
  view. Influx event measurements also omit the documented named event/receive
  time columns and exact event nanoseconds. Public queries must not return
  invented provenance or require backend-specific interpretation.
- [x] `OOTDB-AUDIT-P1-012` `last_success_time_ns` means the last successful
  durable commit as specified. The current service refreshes it on every idle
  poll and therefore reports recent write success even when no commit occurred.
- [x] `OOTDB-AUDIT-P1-013` SQLite public views expose one unambiguous instance
  of each documented column. Most current views select the common `record_id`
  and then `p.*`, producing a duplicate `record_id` column that SQLite silently
  renames for `SELECT *`; this is not a clean stable user query contract.
- [x] `OOTDB-AUDIT-P1-014` Status document/loss counters have defined unique
  semantics across failed commits and reconnect replay. The worker currently
  increments read, unresolved, and loss counters before durability, and a new
  consumer can reconstruct already-durable loss documents after reconnect;
  preserving raw worker totals can therefore double-count work or report rows
  that never committed.
- [x] `OOTDB-AUDIT-P1-015` Status reports a schema version only after the
  selected backend has opened and its migration/version check succeeded. The
  current service hard-codes `Some(3)` while still starting or retrying, so it
  claims a compatible schema that it has not reached.
- [x] `OOTDB-AUDIT-P1-016` Bounded shutdown is idempotent and cannot erase a
  timeout fault. The current second `shutdown` call (including `Drop` after an
  explicit call) has no thread handle and overwrites `faulted` with `stopped`.
- [x] `OOTDB-AUDIT-P1-017` Influx maintenance is bounded per worker pass. The
  current flush loads every pending spool part and builds one unbounded HTTP
  body, so a long outage can turn the configured 1 GiB durable spool into an
  equally unbounded memory/request spike. Delivery must advance in deterministic
  bounded chunks and leave the remaining backlog visible.
- [x] `OOTDB-AUDIT-P1-018` Operator errors are both secret-safe and actionable.
  The current control projection replaces every commit/connection failure with
  the same generic sentence and does not emit the referenced detailed local
  diagnostic, so an operator cannot distinguish DNS, TLS, authentication,
  migration, disk, or reconciliation failures from status.

## Complete contract audit

- [x] `OOTDB-AUDIT-001` Reconcile this board, the umbrella implementation
  checklist, and the typed-read-model board. Reopen every claim contradicted by
  source, tests, or current exact-candidate evidence.
- [x] `OOTDB-AUDIT-002` Audit disabled/default behavior, TOML discriminator,
  exact selected-table validation, numeric bounds, environment-only secrets,
  TLS, identifiers, and native path resolution.
- [x] `OOTDB-AUDIT-003` Audit definition artifact validation, shared-memory
  source ownership, cold-start hash rules, prior epoch handling, loss accounting,
  placeholders, receive/source time, and checkpoint replay behavior.
- [x] `OOTDB-AUDIT-004` Audit worker batching, pending-batch retry, idempotency,
  checkpoint atomicity, retry classification/budget/reset, reconnect, queue
  bounds, sustained input, catch-up, and ring overwrite reporting.
- [x] `OOTDB-AUDIT-005` Audit lifecycle/status transitions, all cumulative
  counters, deterministic warning codes, redaction, fault classification,
  control-boundary exposure, and bounded drain semantics.
- [x] `OOTDB-AUDIT-006` Audit backend-neutral projection coverage for every
  specified domain and IEC value type, including unknown events, exact unsigned
  values, previous values, provenance, audit fields, loss, and placeholders.
- [x] `OOTDB-AUDIT-007` Audit schema/migration/transaction/query parity for
  SQLite, PostgreSQL, TimescaleDB, MySQL, MariaDB, SQL Server, and InfluxDB 3.
- [x] `OOTDB-AUDIT-008` Audit InfluxDB spool capacity, restart, part identity,
  partial reconciliation, authentication, continuous delivery, and shutdown.
- [x] `OOTDB-AUDIT-009` Audit filesystem permissions, disk-full/corruption/newer
  schema behavior, secret handling, TLS identity, backups, retention, and
  least-privilege guidance.
- [x] `OOTDB-AUDIT-010` Audit all examples and documentation against the same
  real PLC workload, every logging domain/value type, public table names,
  JSON-free ordinary queries, and Linux/Windows-safe configuration paths.
- [x] `OOTDB-AUDIT-011` Audit CI registration, pinned products, clean-runner
  provisioning, teardown on failure, redacted evidence, performance floors,
  exact-SHA binding, and release dependency wiring.
- [x] `OOTDB-AUDIT-012` Complete a SOLID/KISS/DRY review. Split the 990-line
  lifecycle module before adding substantial tests or responsibilities; keep
  orchestration, retry accounting, drain policy, and test fixtures narrow.

## Post-code detailed audit evidence

The required fresh audit was restarted after the final production refactor.
It reviewed the complete changed-file manifest, normative specification,
lifecycle and reconnect paths, every schema/encoder, migration behavior,
examples, platform paths, runner security, workflow/release wiring, and
SOLID/KISS/DRY boundaries. It found and repaired these additional defects:

- the Nginx TLS proxy image lacked an immutable digest;
- runner teardown accepted symlinked state/marker paths;
- InfluxDB loss/unresolved rows omitted exact receive/event nanosecond fields,
  and loss rows omitted the common `sequence` projection;
- public migration/query guidance still described every backend as schema v3;
- workspace package entries in `Cargo.lock` remained at `0.24.67`;
- the all-feature Windows warning gate rejected Unix-only schema dispatcher
  ownership; and
- reconnect accumulation violated the clippy/SOLID argument-count gate.

Each behavioral defect used an expected assertion red followed by the same
focused test green. The final post-fix audit evidence includes 49/49 OpenOT
persistence library tests, 4/4 runner contract tests, the example/docs
contract, YAML and path-hygiene checks, all-database-feature Windows
cross-compilation with `RUSTFLAGS=-Dwarnings`, and focused all-target clippy.
The lifecycle module is 583 lines with 586 lines of isolated tests. Exact
seven-product, runtime-vertical, diagram, and broad proof are now green on the
remote builder. Exact committed-SHA CI, merge, and release proof remain owned
by the still-open workflow rows below.

## Corrective workflow

- [x] `OOTDB-AUDIT-WF-001` Finish and publish the bounded severity-ranked defect
  ledger before changing production behavior.
- [x] `OOTDB-AUDIT-WF-002` Update the normative specification for every missing
  or ambiguous contract.
- [x] `OOTDB-AUDIT-WF-003` Add the smallest native test for each defect and
  record its expected assertion failure before production changes.
- [x] `OOTDB-AUDIT-WF-004` Implement the complete reviewed defect batch and
  update diagrams, architecture checklist, changelog, and synchronized version.
- [x] `OOTDB-AUDIT-WF-005` Run the same focused tests green, then the complete
  real PLC/runtime and seven-product database matrix on the remote builder.
- [x] `OOTDB-AUDIT-WF-005A` After all code changes are frozen, perform a second
  independent detailed audit of the complete diff against the normative
  specification, every backend schema/encoder, lifecycle/error path, examples,
  CI, and release wiring. Reopen code if that audit finds any discrepancy.
- [ ] `OOTDB-AUDIT-WF-006` Freeze one exact candidate; run format, clippy,
  test-all, runtime vertical, OpenOT integration/capstone, diagram, example,
  provisioning, security, and release-candidate gates.
- [ ] `OOTDB-AUDIT-WF-007` Commit and push once, collect all CI and automated
  review results, repair a complete failure ledger as one batch if necessary,
  guarded-merge, publish the replacement release, verify assets/checksums and
  Latest, then complete the post-merge audit.
