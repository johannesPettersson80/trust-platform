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
path = "history/openot.sqlite3"
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
schema = "openot"
tls = "require"
ca_cert_path = "certs/openot-database-ca.pem"

[runtime.openot.persistence.timescaledb]
connection_url_env = "TRUST_OPENOT_DATABASE_URL"
schema = "openot"
tls = "require"
ca_cert_path = "certs/openot-database-ca.pem"

[runtime.openot.persistence.mysql]
connection_url_env = "TRUST_OPENOT_DATABASE_URL"
database = "openot"
tls = "require"
ca_cert_path = "certs/openot-database-ca.pem"

[runtime.openot.persistence.sqlserver]
connection_url_env = "TRUST_OPENOT_DATABASE_URL"
schema = "openot"
tls = "require"
ca_cert_path = "certs/openot-database-ca.pem"

[runtime.openot.persistence.influxdb3]
host_env = "TRUST_OPENOT_INFLUX_HOST"
token_env = "TRUST_OPENOT_INFLUX_TOKEN"
database = "openot"
spool_path = "history/openot-influx-spool.sqlite3"
max_bytes = 1073741824
ca_cert_path = "certs/openot-influx-ca.pem"
```

SQLite `path`, InfluxDB `spool_path`, and remote `ca_cert_path` values are
resolved relative to the bundle root unless absolute. They MUST not be empty and their parent directories MUST
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

For SQLite, PostgreSQL, TimescaleDB, MySQL/MariaDB, and SQL Server, document
rows and the corresponding consumer checkpoint MUST commit in one database
transaction. A failed transaction advances neither. A crash leaves either the
whole batch and checkpoint visible or neither; replay is resolved by the
identity and payload rules above.

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
cursor/head lag, last successful commit time, and a redacted last error.
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

The initial product exposes status through the existing structured runtime
control/observability boundary and direct database inspection. It adds no
stable document-query API and no raw-SQL passthrough.

## 8. Security and operations

Examples use non-secret environment-variable names and local development
accounts only. Deployments SHOULD use a least-privilege role limited to the
owned schema/database, protected filesystem permissions, authenticated TLS,
and independently managed backups. Backup, restore, integrity checking,
retention, Timescale compression/retention, Influx spool sizing, migration,
and clean shutdown procedures MUST be documented per shipped backend.

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
compare the canonical documents, not only row counts.
