# OpenOT Database Persistence Product Contract

Specification ID: `SPEC_OPENOT_DATABASE_PERSISTENCE_001`

Status: normative truST product specification.

This document defines durable persistence of resolved OpenOT documents by the
truST runtime host. It is a truST product contract outside IEC 61131-3 and
therefore creates neither an IEC requirement nor an IEC deviation.

The words MUST, MUST NOT, SHOULD, and MAY are normative.

## 1. Scope and authority

Persistence consumes the resolved `open_ot_document::Document` stream defined
by the OpenOT dependencies pinned at
`137f0e765f085c262651f479be35298b836ac891`. It MUST preserve every `Event`, `Loss`, and
`Placeholder` document, including unknown extension fields and raw placeholder
slots. It MUST NOT reinterpret wire records, bypass definition-hash checking,
or turn an unresolved record into a guessed event.

Database work is a host concern after shared-memory carriage, validation, loss
accounting, definition-epoch selection, and document construction. Connection,
migration, serialization, retry, spool, and database I/O MUST NOT run in the
PLC scan cycle or `OpenOtTelemetrySubsystem::publish` path. Database failure
MUST NOT stop PLC execution; it changes persistence health and may lead to
explicit OpenOT loss when bounded storage is exhausted.

This feature is distinct from `[runtime.observability]` periodic JSONL history.
It is not a waveform historian, an ST SQL API, a raw-SQL product API, or an
editable audit store.

## 2. Configuration contract

Persistence is disabled by default. The only backend selector is the TOML
`backend` discriminator:

```toml
[runtime.openot.persistence]
enabled = true
backend = "sqlite"
batch_size = 256
flush_interval_ms = 250
queue_capacity = 4096
shutdown_timeout_ms = 5000
retry_initial_ms = 250
retry_max_ms = 30000
retry_multiplier = 2
retry_max_attempts = 20

[runtime.openot.persistence.sqlite]
path = "history/trust-logging.sqlite3"
```

The recognized backend values are `sqlite`, `postgresql`, `timescaledb`,
`mysql`, `sqlserver`, and `influxdb3`. The selected value MUST construct only
that adapter. truST MUST NOT infer a backend from a URL, probe installed
clients, or fall back to another adapter after configuration, startup, or
connection failure. A recognized backend omitted from the current build MUST
fail startup with a named unavailable-backend error.

When `enabled = false` or the persistence table is absent, no persistence
worker, spool, migration, or database connection is created. When enabled,
`backend` and exactly one matching backend table are required. Unknown fields,
unknown backends, a missing selected table, and any unselected backend table
MUST be rejected before services start.

Common numeric limits MUST be positive and representable by their runtime
types. Defaults are the values shown above. `batch_size` MUST NOT exceed
`queue_capacity`; `retry_initial_ms` MUST NOT exceed `retry_max_ms`; and
`retry_multiplier` MUST be in the range 1 through 16, and
`retry_max_attempts` MUST be positive. Exhausting that many consecutive
transient attempts faults persistence; one successful worker pass resets the
consecutive-attempt budget.

Backend tables are:

```toml
[runtime.openot.persistence.postgresql]
connection_url_env = "TRUST_OPENOT_DATABASE_URL"
schema = "trust_logging"
tls = "require"
ca_cert_path = "certs/openot-database-ca.pem"

[runtime.openot.persistence.timescaledb]
connection_url_env = "TRUST_OPENOT_DATABASE_URL"
schema = "trust_logging"
tls = "require"
ca_cert_path = "certs/openot-database-ca.pem"

[runtime.openot.persistence.mysql]
connection_url_env = "TRUST_OPENOT_DATABASE_URL"
database = "trust_logging"
tls = "require"
ca_cert_path = "certs/openot-database-ca.pem"

[runtime.openot.persistence.sqlserver]
connection_url_env = "TRUST_OPENOT_DATABASE_URL"
schema = "trust_logging"
tls = "require"
ca_cert_path = "certs/openot-database-ca.pem"

[runtime.openot.persistence.influxdb3]
host_env = "TRUST_OPENOT_INFLUX_HOST"
token_env = "TRUST_OPENOT_INFLUX_TOKEN"
database = "trust_logging"
spool_path = "history/trust-logging-influx-spool.sqlite3"
max_bytes = 1073741824
ca_cert_path = "certs/openot-influx-ca.pem"
```

SQLite `path`, InfluxDB `spool_path`, and remote `ca_cert_path` values are
resolved with native platform path semantics relative to the bundle root unless
absolute. Shipped examples MUST use a Windows- and Linux-compatible TCP runtime
control endpoint rather than a Unix-domain socket. Persistence paths MUST not
be empty and their parent directories MUST
not be group/world writable unless an explicit future security policy allows
it. Remote credentials MUST be obtained through the named environment
variables. Inline connection URLs, passwords, and tokens are not accepted.
Secret values MUST be redacted from errors, logs, status, and support output.
Remote production connections MUST use authenticated TLS; an explicit
development-only TLS relaxation MAY be introduced only by a later specification.

InfluxDB `max_bytes` is required and bounds the spool's logical SQLite page
footprint. Startup fails if the schema alone exceeds it. A transaction that
would exceed it rolls back both documents and checkpoint and reports durable
capacity exhaustion as a non-retryable persistence fault. SQLite WAL and
filesystem allocation overhead still require additional free disk; operators
MUST alert on filesystem capacity before that logical limit is reached.

TimescaleDB uses the PostgreSQL protocol but remains an explicit TOML value so
startup can require the TimescaleDB extension and create/verify its hypertable.
The `mysql` adapter supports both actual MySQL and MariaDB servers; product
compatibility is claimed separately for each server and version tested.

## 3. Ownership and execution

The runtime host supervises a persistence worker outside the scan thread. The
worker is composed from narrow responsibilities:

- `DocumentSource` yields resolved documents and source cursor information;
- `DocumentSink` validates/migrates and durably commits document batches;
- `CheckpointStore` binds durable acknowledgement to those commits;
- `RetryPolicy` calculates bounded retry timing without interpreting documents;
- status projection reports state and counters without exposing secrets.

The product-owned document source MUST read the same shared-memory carriage
configured by `runtime.openot.path`. For each poll it MUST take a coherent
control-block snapshot, feed every decoded record to OpenOT loss accounting,
select the current or immediately-prior hash-bound definition with
`open_ot_definition::resolve_record`, and construct canonical documents with
`open_ot_document`. A durable checkpoint may suppress a replayed record from a
new database transaction, but it MUST NOT suppress that record from rebuilding
loss-accounting state after process restart. Receive time is assigned by the
host consumer; producer source time remains the distinct time carried by the
record.

The definition used by the running program MUST be emitted as a bundle-owned
artifact during compilation and loaded by the persistence source. Startup MUST
verify its full content hash and its carriage hash against the control block.
During the existing initial cold-start state where the control block still
carries the all-zero hash, the compiled current bundle definition is the sole
permitted zero-hash projection definition. That alias ends when the producer
publishes a nonzero hash and MUST never select a prior or arbitrary catalog
entry.
When the control block names a different current or prior hash, the source MUST
retain the record as a `Placeholder`; it MUST NOT resolve it using a convenient
but mismatched definition. A later milestone may add a definition registry,
but database configuration is not permitted to name an unrelated definition
file.

The first release runs this worker as a supervised runtime-host service. The
interfaces MUST permit extraction to a dedicated process without changing the
document or sink contracts, but this release does not add a generic plugin
framework or a separate public daemon.

## 4. Document fidelity and schema

Every backend MUST store the canonical complete serialized document plus
indexed columns sufficient to select document kind, buffer, run, source,
epoch, sequence or loss range, source timestamp, receive timestamp, event type,
and definition hash where present. Indexed columns are projections; the
canonical document remains semantic authority. A projection mismatch is
corruption and MUST NOT be silently repaired during reads.

Canonical storage alone is not an acceptable user query surface. truST MUST
also project each document into descriptive, typed, read-only logging tables
owned by this product specification. Ordinary value, alarm, message, state,
batch, operator, audit, signature, system, loss, and unresolved-record queries
MUST NOT require knowledge of OpenOT field arrays, JSON paths, registry keys,
or internal table names.

### 4.1 Ownership and names

OpenOT owns the semantic input document. truST owns the database schema,
migrations, typed projection, public names, and query examples. No OpenOT
carriage, definition, registry, or document contract is changed by this read
model.

`runtime.openot.persistence` remains the correct configuration namespace
because it selects persistence of the OpenOT document stream. Database object
names MUST describe what a database user can query and MUST NOT use `openot_`
as a blanket prefix.

The internal relational objects are:

| Object | Contract |
| --- | --- |
| `logging_schema` | singleton migration version owned by truST |
| `logging_records` | append-only canonical documents and indexed provenance |
| `logging_checkpoint` | singleton durable carriage checkpoint |
| `logging_delivery_spool` | InfluxDB-only local delivery state |

Internal objects are not a stable integration API. A least-privilege reporting
role SHOULD receive `SELECT` on the public read model and no direct privilege on
internal objects.

The stable public read-model objects are:

| Object | Contents |
| --- | --- |
| `event_log` | searchable envelope for every resolved event |
| `logged_values` | `ValueChanged` and `ParameterChange` values |
| `alarm_history` | alarm/interlock activation, clearing, and lifecycle |
| `message_log` | message template, severity, and typed arguments |
| `state_history` | process, mode, ISA-88, and PackML transitions |
| `batch_history` | batch state changes |
| `recipe_history` | recipe loads and approvals |
| `material_additions` | batch material quantities and units |
| `operator_activity` | actions, login/logout, and access failures |
| `audit_log` | audited parameter changes and regulated audit events |
| `electronic_signatures` | signature-to-event relationships |
| `system_events` | logger/runtime diagnostic events |
| `data_loss` | authoritative and inferred missing ranges |
| `unresolved_records` | fail-closed placeholders and their reason |

These names and documented columns are a stable read-only database contract.
truST MAY add nullable columns compatibly. Removing, renaming, changing the
meaning/type of a documented column, or moving a documented event to a
different table requires a specification change and migration. Users MUST NOT
insert, update, or delete these objects directly.

### 4.2 Common public columns

Every public row derived from a record MUST contain:

- `record_id`: the deterministic truST identity string;
- `event_time`: a database-native UTC timestamp suitable for ordinary queries;
- `event_time_ns`: the exact OpenOT source nanoseconds when present;
- `received_time`: the database-native UTC receive timestamp;
- `received_time_ns`: the exact receive nanoseconds;
- `source`: the resolved human-facing source name, nullable only when OpenOT
  could not resolve one;
- `source_id`, `source_path`, and `source_hierarchy`;
- `buffer_id`, `run_id`, `epoch_id`, and `sequence` where applicable;
- `definition_hash`;
- `time_unsynced`, `synthetic_record`, and `partial_payload`.

`event_time` is never synthesized from receive time. When source time is
absent, `event_time` and `event_time_ns` are `NULL` and receive time remains
separate. Exact unsigned identifiers and nanoseconds MUST remain lossless even
where the database lacks an unsigned 64-bit integer type.

`event_log` adds `event_type_id`, `event_name`, and
`has_unclassified_fields`. It receives exactly one row for every resolved
event, including future events that do not yet have a domain projection.

### 4.3 Typed values

`logged_values` MUST expose:

- `value_name`, `value_type`, `unit`, `quality`, and `semantic_role`;
- current lanes `boolean_value`, `signed_value`, `unsigned_value`,
  `number_value`, and `text_value`;
- matching `previous_boolean_value`, `previous_signed_value`,
  `previous_unsigned_value`, `previous_number_value`, and
  `previous_text_value` lanes;
- `exact_value` and `previous_exact_value` as lossless human-readable
  representations;
- `is_audited`, `actor`, `reason`, and `authorization_result`.

Exactly one current typed lane MUST be non-`NULL`, selected by `value_type`.
Previous lanes are all `NULL` when OpenOT has no previous value; otherwise
exactly the matching previous lane is non-`NULL`. `BOOL` uses the Boolean lane;
signed IEC integers use the signed lane; unsigned IEC integers use the unsigned
lane; `REAL`/`LREAL` use the number lane; bounded `STRING` uses the text lane.
`DATE_AND_TIME` uses an exact unsigned-nanosecond representation plus a UTC
timestamp column when representable. Unsupported byte/private-extension
payloads remain canonical and MUST NOT be guessed into a value lane.

The SQL products map signed values to `BIGINT`, unsigned values to an exact
20-digit decimal where supported, floating values to the native 64-bit
floating type, and Boolean/text values to native Boolean/text types. SQLite
MUST store unsigned values beyond signed 64-bit range as canonical decimal text
rather than convert them to `REAL`; `exact_value` makes this exception visible
and lossless. InfluxDB MUST use native signed, unsigned, float, Boolean, and
string fields. A convenience numeric column MUST be `NULL` rather than silently
round an integer outside its exact range.

`ParameterChange` produces both a `logged_values` row with `is_audited = true`
and an `audit_log` row. Those two rows share `record_id`; this intentional
projection duplication does not duplicate the canonical event.

### 4.4 Domain tables

The remaining public tables MUST expose these domain fields in addition to the
common columns:

| Object | Required domain columns |
| --- | --- |
| `alarm_history` | `condition`, `condition_class`, `lifecycle_action`, `correlation_id`, `severity`, `severity_label`, `cause`, `actor`, `reason`, `comment`, `shelve_seconds`, `previous_priority`, `new_priority` |
| `message_log` | `message_template`, `severity`, `severity_label`, `arg1_type`..`arg4_type`, and typed/display `arg1`..`arg4` values |
| `state_history` | `state_machine`, `state_category`, `previous_state`, `previous_state_label`, `new_state`, `new_state_label` |
| `batch_history` | `batch_id`, `recipe_id`, `previous_state`, `new_state`, `new_state_label` |
| `recipe_history` | `action`, `recipe_id`, `recipe_version`, `batch_id`, `actor`, `authorization_result` |
| `material_additions` | `batch_id`, `material_id`, `quantity`, `exact_quantity`, `unit` |
| `operator_activity` | `action`, `action_id`, `actor`, `workstation`, `role`, `authorization_result`, `reason`, and documented context references |
| `audit_log` | `action`, `target`, `actor`, `reason`, `authorization_result`, previous/current typed values, and workstation when present |
| `electronic_signatures` | `action_id`, `actor`, `meaning`, `authorization_result`, `signed_source_id`, and `signed_sequence` |
| `system_events` | `event_name` plus typed documented system-event counters/identities; event-specific fields that are not yet public remain canonical |
| `data_loss` | `first_sequence`, `last_sequence`, `lost_count`, and `basis` |
| `unresolved_records` | `event_type_id`, `reason`, and a safe diagnostic summary; raw slots remain internal canonical data |

Known OpenOT event IDs, not event-name strings or JSON paths, select the
projector. A known event missing a required field, containing an impossible
type for that field, or producing a projection that disagrees with the
canonical document is corruption. The whole commit MUST fail before checkpoint
advancement.

A future unknown event still receives an `event_log` row and remains complete
in `logging_records`. It increments `unclassified_events` and sets
`has_unclassified_fields = true`; it MUST NOT be discarded or guessed into a
domain table. Private extension fields remain in the canonical document until
a later specification assigns them a public typed column.

### 4.5 Projection ownership and atomicity

One backend-neutral truST `LoggingProjector` MUST map a typed
`open_ot_document::Document` plus the exact definition metadata selected by the
document's `definition_hash` to the canonical row, event envelope, and zero or
more typed domain rows. This definition lookup is required because document
fields may carry resolved reference names while stable numeric IDs, declared
value types, and other referenced metadata remain definition-owned; older
canonical rows may also retain a numeric reference. The projector MUST
cross-check either representation against the hash-matched definition and
reject a known event when that definition is unavailable or the reference is
absent. It MUST NOT invent a name from a numeric ID or use a convenient
mismatched definition.
Adapters MUST NOT independently interpret OpenOT JSON. Backend code owns only
DDL/type mapping, parameter binding, transaction/error handling, and
product-specific verification.

For SQLite, PostgreSQL, TimescaleDB, MySQL, MariaDB, and SQL Server, canonical
row, `event_log` row, domain rows, and checkpoint MUST commit in the existing
single sink transaction. A failure or process crash exposes all of them or none
of them. Duplicate replay compares the canonical payload and creates no
duplicate public rows. Each public row is keyed by `record_id` (and a stable
ordinal only where one event legitimately projects repeated values).

No asynchronous relational projector or trigger is introduced. Query tables
MUST be immediately consistent with a successful durable commit.

### 4.6 InfluxDB 3 physical model

InfluxDB 3 MUST use the same descriptive public table names as measurements.
Each measurement MUST be homogeneous: commonly filtered identity/source fields
are tags; measured/domain values are native typed fields; OpenOT source time is
the point timestamp; exact receive time remains a field. The implementation
MUST avoid one wide sparse measurement and MUST NOT store the normal query
surface as one JSON field.

The mandatory local SQLite spool remains the durable acceptance authority. A
spool document owns a deterministic set of delivery parts: one internal
`logging_records` line and its public projection lines. Each part has a stable
idempotent point identity. `accept_partial` and `no_sync` MUST remain disabled.
Because an error response may follow a partial remote write, retry and
reconciliation MUST verify every expected part before marking the spool
document delivered. Persistence is not `caught_up` while any part is absent.

### 4.7 TimescaleDB physical model

TimescaleDB MUST use actual typed time-oriented public tables rather than a
detached time index over canonical JSON. `event_log`, `logged_values`,
`alarm_history`, `message_log`, and `state_history` are hypertables. They use
non-null `received_time` as the `TIMESTAMPTZ` partition dimension because
OpenOT source `event_time` is intentionally nullable when a producer has no
source clock. Each hypertable uses `(received_time, record_id)` as its unique
key, as TimescaleDB requires; source-time queries continue to use the separate
`event_time` column. The remaining lower-volume domain objects and non-event
records remain ordinary relational tables unless a later measured workload and
specification revision promotes them.

### 4.8 Migration and reconstruction

The first typed-read-model schema is version 3. Migration from schema v2 MUST:

1. rename legacy `openot_schema`, `openot_documents`, and
   `openot_checkpoint` objects to their `logging_*` names without dropping or
   rewriting the canonical rows;
2. create the descriptive public objects;
3. load a trusted definition catalog containing every `definition_hash`
   referenced by the stored canonical records;
4. stream every existing canonical document through the same Rust
   `LoggingProjector` used for new writes and the exactly matching definition;
5. backfill public rows idempotently;
6. compare canonical identities, expected projection identities, row counts,
   and typed values;
7. advance the schema version only after the complete backfill validates.

An interrupted migration MUST be safely repeatable. It MUST never delete the
only canonical copy. An unprojectable known event or missing historical
definition fails closed with its `record_id` and `definition_hash`; the runtime
MUST NOT advance schema version or silently produce incomplete public rows.
InfluxDB reconstruction uses the durable local spool/canonical authority and
remains degraded until every remote projection part reconciles.

The projector MUST be deterministic and replayable so future read-model
versions can be rebuilt from `logging_records` without changing OpenOT or PLC
execution.

Schemas and migrations are owned by truST. Users MUST NOT rely on undocumented
columns as a stable API. Stored history is append-only through truST. A backend
MUST reject an unknown newer schema version and corrupt migration metadata; it
MUST NOT drop, recreate, truncate, or downgrade durable state automatically.

Event and placeholder idempotency identity is
`(buffer_id, run_id, source_id, seq)`. Loss identity is
`(buffer_id, run_id, source_id, epoch_id, first_seq, last_seq, basis)`.
Reinserting an identical canonical payload is a counted duplicate. The same
identity with a different canonical payload is a conflict and MUST fault the
worker rather than hide the discrepancy.

Source-local sequence order MUST be preserved within `(buffer_id, run_id,
source_id)`. No total order is promised across sources. Source and receive
timestamps remain separate; receive time MUST NOT be presented as source time.

## 5. Delivery, checkpoints, and restart

The delivery guarantee is idempotent at-least-once. truST does not claim
exactly-once delivery across process, filesystem, and server failures.

For SQLite, PostgreSQL, TimescaleDB, MySQL/MariaDB, and SQL Server, canonical
rows, all required public projection rows, and the corresponding consumer
checkpoint MUST commit in one database transaction. A failed transaction
advances neither. A crash leaves either the whole batch, projections, and
checkpoint visible or none of them; replay is resolved by the identity and
payload rules above.

InfluxDB 3 does not provide that relational transaction boundary. Therefore
its configured local SQLite spool is mandatory and is the durable acceptance
authority: documents and spool checkpoint commit atomically there before
delivery. Server acknowledgement then marks the spool batch delivered.
Replay uses a deterministic point identity and timestamp and MUST compare the
canonical payload through the verification path. InfluxDB persistence MUST NOT
report caught up until every accepted spool entry is server-acknowledged.

Before the producer publishes its first non-zero definition hash, the initial
run-0/epoch-0 zero-hash carriage snapshot MUST bind only to the exact compiled
current bundle definition. A zero hash in any later run or epoch, or any
non-zero mismatch, MUST continue to fail closed as an unresolved placeholder.

On restart, the worker resumes from its durable checkpoint. It MUST detect and
report a recreated ring, changed buffer identity, cold producer run, stale
checkpoint, cursor older than `OldestAbs`, and current/prior definition epoch.
Ring overwrite MUST produce queryable authoritative or inferred `Loss`
documents; checkpoint recovery MUST never imply completeness across a gap.
Missing or mismatched definitions remain `Placeholder` documents.

Warm definition change keeps the current and immediately prior definition
selection rules supplied by OpenOT. A cold run remains distinct even when
source identifiers or sequence numbers repeat.

## 6. Outage, bounds, and shutdown

The in-memory queue is bounded by `queue_capacity`. Retries use capped
exponential delay from the configured initial, maximum, and multiplier values.
Catch-up preserves source-local order. There is no infinite tight retry loop.

Relational adapters do not silently insert an additional SQLite spool in the
first release. Their durable boundary is the database transaction; documents
not yet committed remain subject to the finite shared-memory and memory-queue
limits. InfluxDB 3 requires the explicit durable spool described above. A
future optional spool for another backend requires a specification delta and
new TOML surface.

When a database is unavailable, the worker enters `retrying`; growing lag or
storage pressure enters `degraded`; exhausted durable capacity, corruption,
identity conflict, or retry policy exhaustion enters `faulted`. Any resulting
ring overwrite remains explicit as OpenOT loss. No uncommitted document is
reported as committed.

Shutdown drains for at most `shutdown_timeout_ms`, then reports the exact
pending count and exits without advancing beyond the last durable commit.
Disk-full, permission, corrupt database/spool, malformed stored document, and
definition corruption are actionable failures and MUST NOT trigger automatic
state deletion.

Initial resource targets are: at most 4,096 queued documents by default, 256
documents per transaction, 250 ms maximum ordinary flush delay, and 5 seconds
shutdown drain. On the reviewed warm x86_64 release runner with each database
on its reviewed local TLS endpoint, every supported product MUST demonstrate at
least 100 canonical documents/second sustained capacity, at least 250 canonical
documents/second burst catch-up, and p95 commit latency no greater than 500 ms
for the 37-document conformance batch. These are release-qualification floors,
not PLC scan-time or arbitrary deployment guarantees. Benchmarks MUST publish
the observed rates and environment before production-readiness claims.
Operators MUST size the ring
and Influx spool for their outage window; truST MUST expose lag before unread
ring data is overwritten.

## 7. Lifecycle and observability

The lifecycle states are `disabled`, `starting`, `ready`, `catching_up`,
`degraded`, `retrying`, `faulted`, and `stopped`. Persistence health is distinct
from PLC runtime health.

Status MUST expose backend name, schema version, documents read, committed,
duplicated, retried, source-ring pending bytes, required remote-delivery pending
documents, rejected, unresolved, loss-range count, lost record count,
unclassified-event count, projection rows committed, cursor/head lag, last
successful commit time, and a redacted last error.
`projection_rows_committed` counts newly committed rows in the descriptive
public read model, excluding internal canonical/checkpoint rows.
`unclassified_event_count` counts retained future event records whose fields
could not be assigned to a known typed domain without guessing.
`pending_part_count` counts durable InfluxDB delivery parts not yet reconciled;
it is zero for atomic relational backends. `reconciled_part_count` is the
cumulative number of such parts confirmed remotely during this runtime.
Status MUST also expose deterministic warning codes derived without database
I/O: `lag` for a nonzero cursor lag, `retrying` after a retry, `placeholder`
for unresolved documents, `loss` for any loss range, `spool_pressure` for
required remote-delivery backlog, `migration_or_storage_fault` for a faulted
startup/commit error, and `shutdown_pending` when stopped or faulted with local
or remote work outstanding. Multiple applicable warnings are all returned in
that order. These codes are operator hints; counters and `last_error` retain
the exact evidence.
`ready` means the selected backend is reachable, compatible, migrated, and has
no required remote delivery outstanding. `catching_up`, remote spool backlog,
or unresolved loss cannot be represented as complete.

The product exposes status through the existing structured runtime
control/observability boundary and exposes the documented public database
objects as a stable read-only query contract. It adds no PLC-language query
API, runtime raw-SQL passthrough, or promise that undocumented internal columns
are stable.

## 8. Security and operations

Examples use non-secret environment-variable names and local development
accounts only. Deployments SHOULD use a least-privilege role limited to the
owned schema/database, protected filesystem permissions, authenticated TLS,
and independently managed backups. Backup, restore, integrity checking,
retention, Timescale compression/retention, Influx spool sizing, migration,
and clean shutdown procedures MUST be documented per shipped backend.
Documentation and examples MUST query the descriptive public objects without
JSON extraction. Raw canonical inspection is an advanced integrity/recovery
procedure, not the ordinary value/alarm/message workflow.

Opening a backup or database with a newer schema fails closed. Automatic
rollback is not promised; rollback requires restoring a compatible backup.
Retention MUST NOT delete rows required by an undelivered checkpoint or hide a
known loss range.

## 9. Supported backend proof

A backend is supported only after its adapter, migrations, failure/restart
tests, example, operations documentation, and full canonical OpenOT coverage
manifest pass against the real named product. Compile-only tests, mocks,
protocol substitutes, and a different compatible server do not establish a
product claim.

The intended first-release matrix is SQLite, PostgreSQL, TimescaleDB, MySQL,
MariaDB through the `mysql` adapter, SQL Server, and InfluxDB 3. Exact minimum
and current supported versions remain release evidence and MUST be frozen from
real test results before publication. Azure SQL is not claimed merely from SQL
Server proof.

The shared canonical workload MUST cover all supported OpenOT value types and
sampling policies; audited parameter changes; process, mode, ISA-88, and PackML
states; alarms and interlocks; the full condition lifecycle; typed messages;
batch, recipe, and material events; operator, login/logout, and security
events; electronic signatures; runtime/system records; loss; placeholders;
provenance; and multiple programs. Every backend example MUST retrieve and
compare the canonical documents and MUST independently query every required
typed public projection, not only row counts.
