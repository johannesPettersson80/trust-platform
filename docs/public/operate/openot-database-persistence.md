# OpenOT database persistence

OpenOT persistence stores the semantic records produced by the truST runtime as
canonical OpenOT documents. It is intended for alarms, messages, values, state
changes, batch and recipe history, operator/security actions, electronic
signatures, explicit loss ranges, and unresolved placeholders. It is not a
replacement for the periodic historian: use `[runtime.observability]` when the
requirement is regular time-series sampling rather than event/audit semantics.

Persistence is disabled by default. The database is selected only in TOML:

```toml
[runtime.openot.persistence]
enabled = true
backend = "sqlite"

[runtime.openot.persistence.sqlite]
path = "history/openot.sqlite3"
```

Supported selectors are `sqlite`, `postgresql`, `timescaledb`, `mysql`,
`sqlserver`, and `influxdb3`. MariaDB uses the `mysql` selector and adapter but
is verified as a separate database product. Azure SQL is not claimed as
verified support until the real Azure service matrix passes.

## Choose a database

| Product | A good fit when | Operational boundary |
|---|---|---|
| SQLite | one runtime needs a local, zero-service audit store | protect and back up the database file and parent directory |
| PostgreSQL | several clients need transactional SQL and mature operations | run and monitor a TLS-enabled server |
| TimescaleDB | PostgreSQL semantics plus time-range operations are required | install and retain the TimescaleDB extension; plain PostgreSQL is not equivalent |
| MySQL | the site already operates MySQL 8.4 LTS | use the reviewed TLS/collation/schema settings |
| MariaDB | the site standard is MariaDB 11.8 LTS | select `mysql`, but follow MariaDB-specific backup and upgrade procedures |
| SQL Server | Microsoft SQL Server is the managed plant database | use encrypted TDS and a dedicated schema/account |
| InfluxDB 3 | event data must also be queried through InfluxDB SQL | the local SQLite spool is the durability authority during HTTP outages |

There is no universal best backend. SQLite is the easiest first deployment.
Choose a network database when existing operations, multi-client access,
retention tooling, or centralized backup outweigh the extra service and TLS
work.

## Delivery and failure behavior

The PLC scan writes only to the bounded OpenOT shared-memory ring. A supervised
host worker reads the ring, accounts for loss, resolves each record with the
generated `openot-definition.json`, and commits documents outside the scan
thread. A database outage does not stop PLC execution.

Generated event evaluation still consumes scan time. The canonical example is
a coverage stress workload and is intentionally not a cycle-time baseline.
Benchmark the exact production attribute set and ring capacity on the target;
the fact that database I/O is off-scan does not make authoring instrumentation
free.

Release qualification on the reviewed warm x86_64 runner requires each listed
product to exceed 100 canonical documents/second sustained capacity, 250
documents/second burst catch-up, and 500 ms p95 commit latency for the
37-document conformance batch. These floors compare release candidates; they
do not replace capacity testing with the real network, storage, retention,
backup load, and authored event volume of a plant deployment.

For SQL backends, a document batch and its absolute ring cursor commit in one
transaction. A failed transaction advances neither. Retrying the same canonical
document identity and content is idempotent; the same identity with different
content fails closed. If the producer outruns the bounded ring, the database
contains an explicit loss document rather than invented values.

InfluxDB 3 first commits canonical documents and the checkpoint to its local
SQLite spool using WAL and full synchronous durability, then delivers them to
InfluxDB in order. During an HTTP/server outage, inspect spool pressure and
restore connectivity before its bounded storage is exhausted. Locally durable
spool entries are reported as `remote_pending`; the service cannot report
`ready` until that counter returns to zero.
`max_bytes` bounds the spool's logical SQLite page footprint. A full spool
rolls back the incoming documents and checkpoint together and faults visibly;
allow additional filesystem headroom for WAL and allocation overhead.

The runtime `status` response exposes `openot_persistence` with state, selected
backend, schema version, document counters, cursor/head/lag, loss and unresolved
counts, projection-row and unclassified-event counters, Influx reconciliation
and pending-part counters, `remote_pending`, last success, and a redacted last
error. `pending` is
source-ring byte lag; `remote_pending` is the count accepted by a mandatory
local spool but not yet acknowledged by its server. `ready` requires both to be
zero; runtime health remains separate. The ordered `warnings` array uses stable
operator codes: `lag`, `retrying`, `placeholder`, `loss`, `spool_pressure`,
`migration_or_storage_fault`, and `shutdown_pending`. Use the adjacent counters
and redacted error for the exact cause; a warning is not a substitute for those
measurements.

## Configuration

Connection strings and tokens are named by TOML but read from environment
variables. Never store them in the project:

```toml
[runtime.openot.persistence]
enabled = true
backend = "postgresql"
batch_size = 256
flush_interval_ms = 250
queue_capacity = 4096
shutdown_timeout_ms = 5000
retry_max_attempts = 20
retry_initial_ms = 250
retry_max_ms = 30000
retry_multiplier = 2

[runtime.openot.persistence.postgresql]
connection_url_env = "TRUST_OPENOT_DATABASE_URL"
schema = "openot"
tls = "require"
ca_cert_path = "certs/openot-database-ca.pem"
```

The same shape applies to `timescaledb`. The `mysql` table uses `database`
instead of `schema`; `sqlserver` uses `schema`. InfluxDB 3 uses `host_env`,
`token_env`, `database`, `spool_path`, `max_bytes`, and `ca_cert_path`. See the shipped
`examples/openot_multi_program/runtime.*.toml` files for complete parse-tested
configurations.

## Inspect and operate

For SQL products, normal operator queries use descriptive public objects. For
example:

```sql
SELECT event_name, COUNT(*) AS event_count
FROM event_log
GROUP BY event_name
ORDER BY event_name;

SELECT value_name, value_type, exact_value, unit, quality, received_time
FROM logged_values
ORDER BY received_time DESC;

SELECT condition, lifecycle_action, condition_class, received_time
FROM alarm_history
ORDER BY received_time DESC;
```

`logging_records` and `logging_checkpoint` are internal durability/recovery
objects. They retain canonical JSON and the durable cursor, but application and
operator reports normally do not query them. InfluxDB 3 exposes the same public
measurement names and keeps its internal spool in local SQLite.

### Product-native checks

PostgreSQL and TimescaleDB:

```bash
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  'SELECT event_name,count(*) FROM openot.event_log GROUP BY 1 ORDER BY 1'
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  'SELECT extversion FROM pg_extension WHERE extname = '\''timescaledb'\'''
```

For TimescaleDB also verify `event_log`, `logged_values`, `alarm_history`,
`message_log`, and `state_history` in
`timescaledb_information.hypertables`. Configure retention/compression only
after the audit-retention owner has approved it; those policies are not created
implicitly by truST.

MySQL or MariaDB:

```bash
mysql --ssl-mode=VERIFY_CA --ssl-ca=certs/openot-database-ca.pem \
  "$TRUST_OPENOT_DATABASE_URL" -e \
  'SELECT event_name,count(*) FROM event_log GROUP BY 1 ORDER BY 1'
```

Check `SELECT VERSION()` and retain separate acceptance evidence for MySQL and
MariaDB. Sharing the adapter does not make them the same tested product.

SQL Server:

```bash
sqlcmd -N -S localhost -d master -Q \
  'SELECT event_name,COUNT_BIG(*) FROM openot.event_log GROUP BY event_name ORDER BY event_name'
```

Install the database CA in the client trust store first. truST's adapter always
requires CA-verified encryption.

InfluxDB 3:

```bash
curl --fail --cacert certs/openot-influx-ca.pem \
  -H "Authorization: Bearer $TRUST_OPENOT_INFLUX_TOKEN" \
  --get "$TRUST_OPENOT_INFLUX_HOST/api/v3/query_sql" \
  --data-urlencode 'db=openot' \
  --data-urlencode 'q=SELECT event_name,count(*) FROM event_log GROUP BY event_name ORDER BY event_name'
sqlite3 history/openot-influx-spool.sqlite3 \
  'SELECT delivered,count(*) FROM logging_delivery_spool GROUP BY delivered;'
```

To test recovery, stop only the selected database service, confirm runtime
status changes to `retrying` (or that the Influx spool pending count rises),
restart the same service, and wait for cursor/head equality. Do not change the
TOML backend during an outage; truST does not silently fail over.

Back up both documents and checkpoint consistently. For SQLite and the Influx
spool, use the SQLite online backup mechanism or stop the runtime cleanly before
copying. For server databases, use the vendor's transaction-consistent backup.
Restore into the same schema version and validate the document count, canonical
JSON, and checkpoint before reconnecting a producer. PostgreSQL, TimescaleDB,
MySQL, MariaDB, SQL Server, and the Influx spool use schema version 3.
SQLite uses schema version 4 to expose explicit, unambiguous provenance
columns in every public view. Retention is operator owned; do not delete
checkpoint state or remove documents that remain inside an audit retention
period.

The tables and columns are implementation-owned and versioned by truST. Do not
treat undocumented columns as a stable integration API; consume documented
public objects or a documented export boundary. Canonical JSON remains an
internal recovery authority, not the ordinary query model.

## Example and verification

The repository's `examples/openot_multi_program/` canonical example uses
the same Structured Text workload for SQLite, PostgreSQL, TimescaleDB, MySQL,
MariaDB, SQL Server, and InfluxDB 3. Each backend must pass its real vendor
server test, native query, outage/recovery check, and canonical-document
comparison on the exact release candidate before it is listed as released
support.
