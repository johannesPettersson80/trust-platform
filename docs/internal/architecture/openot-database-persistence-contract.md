# OpenOT database persistence architecture decision

Status: accepted for specification and test implementation

## Decision

truST owns OpenOT database persistence as a supervised runtime-host worker
after OpenOT document resolution. The PLC scan publishes only to the existing
shared-memory carriage. The worker consumes the resulting `Event`, `Loss`, and
`Placeholder` documents and dispatches them to exactly one adapter selected by
`runtime.openot.persistence.backend`.

The first implementation remains hosted by `trust-runtime`. Its interfaces
must allow later extraction into a separately supervised process, but we will
not add a daemon or general database plugin system until operational evidence
shows that process isolation is worth the deployment cost.

The narrow ownership boundaries are `DocumentSource`, `DocumentSink`,
`CheckpointStore`, `RetryPolicy`, and status projection. A sink owns its client,
schema, migrations, transaction, and backend error mapping. It does not own
OpenOT resolution or retry policy. Portable runtime core receives no database
dependency.

The requested first-release adapter matrix is:

| TOML value | Product proof | Adapter approach | Durable acknowledgement |
|---|---|---|---|
| `sqlite` | SQLite | SQL adapter | document batch and checkpoint in one transaction |
| `postgresql` | PostgreSQL | SQL adapter | document batch and checkpoint in one transaction |
| `timescaledb` | TimescaleDB extension on PostgreSQL | PostgreSQL adapter plus extension/hypertable checks | document batch and checkpoint in one transaction |
| `mysql` | MySQL and MariaDB tested independently | MySQL-protocol SQL adapter | document batch and checkpoint in one transaction |
| `sqlserver` | Microsoft SQL Server | TDS SQL adapter | document batch and checkpoint in one transaction |
| `influxdb3` | InfluxDB 3 | HTTP write/query adapter plus mandatory SQLite spool | atomic spool acceptance, then idempotent server delivery |

Real-version validation is release evidence rather than architectural proof.
The initial support policy is deliberately exact: the release may name only
the product versions in the dated real-product evidence. Until a second
version is run, the tested minimum and current version are the same version;
protocol compatibility is not inferred. Azure SQL remains unclaimed.

All adapters preserve an internal canonical document plus indexed provenance
and expose the same truST-owned descriptive typed read model. The public
database vocabulary is logging vocabulary (`logged_values`, `alarm_history`,
`message_log`, and the other objects frozen in specification Section 4), not an
`openot_*` implementation prefix. They share identities, projection semantics,
conflict detection, status semantics, and canonical contract tests, but keep
backend connection and durability details separate.

One backend-neutral `LoggingProjector` owns the OpenOT-to-read-model mapping.
It consumes typed `open_ot_document::Document` values plus the exact
hash-matched definition metadata and produces a canonical row, an event
envelope, and zero or more domain rows. The definition is an explicit input
because the document retains referenced IDs but does not duplicate all
human-facing definition names. Missing or mismatched historical definitions
fail migration closed. Adapters own DDL/type
mapping and durable writes; they do not parse OpenOT JSON or maintain separate
event-name dispatch tables. In relational products the canonical row, every
required domain row, and checkpoint share the existing sink transaction. This
avoids the lag and independent failure state of an asynchronous projector.
Each relational adapter owns one connection because the persistence worker is
serialized and permits only one in-flight transaction. A pool would add idle
connections and failure states without concurrency to serve. Reconnect replaces
that connection under the bounded worker retry policy; future parallel consumers
or queries require a specification delta before a bounded pool is introduced.

## OpenOT authority review

The decision was checked against pinned OpenOT revision
`137f0e765f085c262651f479be35298b836ac891`, specifically `docs/overview.md`,
`docs/carriage-contract.md`, `docs/definition-file.md`,
`docs/document-format.md`, `docs/source-high-water.md`, `spec/core.md`,
`spec/definition-file.md`, and `spec/doc-format.md` in the canonical sibling
checkout. The resulting database invariants are:

| OpenOT authority | Persistence consequence | Native proof |
| --- | --- | --- |
| carriage, definition, and document are distinct contracts | persistence consumes resolved `Document`, never raw slots as an event | consumer and canonical workload tests |
| `Seq` is local to `(RunId, SourceId)` | indexes preserve source-local order and make no cross-source total-order claim | source-order and multi-program tests |
| warm epochs resolve by absolute position against current/prior hashes | definition hash/relation/version remain in canonical JSON; mismatches remain placeholders | warm-definition and recreated-run tests |
| authoritative and inferred loss are different bases | both loss bases and their exact ranges remain queryable | forced-overflow and adapter conformance tests |
| placeholders retain typed reason and every raw slot | no adapter filters or guesses unresolved records | 37-document real-product conformance fixture |
| high-water closes silent-source loss | system records and loss reconciliation remain first-class | source-high-water and overflow tests |

No OpenOT source or standard-facing contract is modified by this product
feature.

## Candidate research

The supported set was compared before adapter acceptance. “Operational
burden” is relative to truST's first release, not a universal ranking.

| Candidate | Fidelity and durability | Offline/operations | Rust/build/supply-chain result | Decision |
| --- | --- | --- | --- | --- |
| SQLite | canonical text plus typed read tables; atomic canonical/projection/checkpoint transaction; exact `ULINT` overflow uses decimal text | embedded and strongest offline story; site owns file backup/integrity/retention | `rusqlite 0.40.2`, bundled SQLite C build; feature-scoped; permissive crate license | support local/single-controller deployments |
| PostgreSQL | internal canonical text plus typed relational read model, constraints, indexes, and transactions | central service, TLS/roles/backups required | pure-Rust `postgres 0.19.14` plus `postgres-native-tls 0.5.3`; feature-scoped | primary central relational store |
| TimescaleDB | ordinary canonical table plus typed public hypertables partitioned on non-null receive time while preserving nullable source event time | PostgreSQL operations plus extension upgrades and explicit retention/compression policy | reuses PostgreSQL client; server extension/license boundary remains operator-visible | support time-oriented relational deployment separately from PostgreSQL |
| MySQL | canonical text, binary identities, InnoDB transaction/checkpoint | central service, TLS, vendor-native backup | `mysql 28.0.0` with minimal Rust/rustls features; largest new Rust dependency group; feature-scoped | support MySQL 8.4 LTS |
| MariaDB | shared protocol adapter but separately verified JSON/collation/migration behavior | separate vendor lifecycle and backup policy | same client dependency, distinct real product proof | support MariaDB 11.8 separately through `backend = "mysql"` |
| SQL Server | canonical NVARCHAR JSON with `ISJSON` verification and TDS transaction | proprietary server operations and supported x86_64 runner required | `tiberius 0.12.3` plus Tokio compatibility; pure-Rust TLS/TDS; feature-scoped | support real Microsoft SQL Server; defer Azure claim |
| InfluxDB 3 | homogeneous descriptive measurements with native typed fields; point API cannot atomically bind every remote projection and checkpoint | remote time-series operations plus mandatory bounded local spool and per-part reconciliation | existing HTTP client plus feature-scoped `rusqlite`; no additional native client | support only with SQLite spool as durable acceptance authority |

### Query-model research correction

The initial JSON-only implementation met canonical fidelity but failed the
ordinary database-user usability requirement. Primary product documentation
supports a typed read-model correction:

- SQLite JSON is stored as ordinary text; generated columns exist but would
  couple the public contract to SQLite JSON expressions:
  <https://www.sqlite.org/json1.html> and <https://www.sqlite.org/gencol.html>.
- PostgreSQL views can encapsulate structures, but a PostgreSQL-only JSON view
  does not define the same behavior for the other products:
  <https://www.postgresql.org/docs/18/tutorial-views.html>.
- MySQL JSON indexing depends on matching generated-column expressions and has
  JSON unquoting/collation details that are inappropriate as a cross-product
  public contract:
  <https://dev.mysql.com/doc/refman/8.4/en/generated-column-index-optimizations.html>.
- SQL Server exposes JSON through computed columns or product-specific JSON
  indexing:
  <https://learn.microsoft.com/en-us/sql/relational-databases/json/store-json-documents-in-sql-tables?view=sql-server-ver17>.
- TimescaleDB recommends keeping common queried fields in ordinary columns
  because field access is more efficient than JSONB lookup, and recommends
  time-oriented hypertables:
  <https://docs.timescale.com/use-timescale/latest/schema-management/json/>.
- InfluxDB 3 recommends simple descriptive names, homogeneous tables, native
  typed fields, and avoiding wide sparse schemas:
  <https://docs.influxdata.com/influxdb3/core/write-data/best-practices/schema-design/>.
- InfluxDB documents that a failed batch can be partially written, so a spool
  cannot mark a multi-part projection complete solely from one failed request:
  <https://docs.influxdata.com/influxdb3/core/api/write-data/>.

Database-specific JSON views were rejected because OpenOT fields are a
repeatable typed array, extraction syntax and index behavior differ by product,
and InfluxDB is natively table/tag/field oriented. A generic EAV `event_fields`
table was rejected because ordinary value, alarm, and audit queries would still
require pivots and self-joins. Removing canonical storage was rejected because
future events, private extensions, raw placeholders, and replay would lose
their authority.

The release supply-chain gate must still run license/advisory/unused-dependency
tools on the exact candidate. Server-product licenses and deployment rights are
operator prerequisites; a passing client crate license scan does not grant a
database server license.

### Client supply-chain and platform boundary

The focused 2026-08-30 review ran `cargo deny check advisories licenses bans
sources` and the repository's policy-wrapped `cargo audit` on `trust-builder`.
Both passed. The first scan rejected Tiberius's optional Rustls 0.21 chain for
`RUSTSEC-2026-0098`, `RUSTSEC-2026-0099`, and `RUSTSEC-2026-0104`; the SQL
Server feature now uses Tiberius `native-tls`, retains required CA verification,
passes the real SQL Server 2025 test, and removes vulnerable
`rustls-webpki 0.101.7` from the lockfile. The existing exact `spin 0.9.8`
yanked-package exception remains repository-owned and was not introduced or
expanded by this feature.

`cargo machete 0.9.2` reports none of `rusqlite`, `postgres`,
`postgres-native-tls`, `mysql`, `tiberius`, `tokio-util`, or
`open-ot-document` as unused. It still reports pre-existing workspace findings
outside this slice (`tiverse-mmap` build-facing dependencies, and in this dirty
validation copy the untouched `glob` declarations in `trust-dev` and
`trust-plcopen`); those are not silently rewritten as OpenOT work.

The shipped runtime qualification target for this release is Linux. Bundled
SQLite requires a working C toolchain at build time. PostgreSQL/TimescaleDB and
SQL Server use the platform native-TLS implementation (OpenSSL development and
runtime support on Linux); MySQL/MariaDB use the selected Rustls client feature.
All seven products were compiled and exercised on x86_64 Linux. No Windows,
macOS, aarch64, Azure SQL, or adjacent server-version support claim follows from
that run; those targets must pass their own feature compile and real-product
matrix before being added to the public support table.

## Rationale

OpenOT carriage is intentionally compact and definition-dependent. Persisting
wire slots directly would either lose meaning or duplicate the resolver and
its epoch/loss rules. Consuming resolved documents preserves the three OpenOT
contract boundaries and gives every database one semantic input.

Canonical documents are appropriate replay and audit authority but are not a
usable database read model. The selected design is a transactional projection:
retain canonical input once, then store normal typed domain rows for ordinary
queries. This preserves extensibility without forcing users to learn OpenOT
field arrays or seven JSON dialects.

Explicit TOML selection is predictable and reviewable. URL inference and
fallback can silently write regulated history to the wrong system, so both are
forbidden.

SQLite provides the smallest local deployment and establishes the schema and
transaction contract first. PostgreSQL, TimescaleDB, MySQL/MariaDB, and SQL
Server cover common plant and enterprise relational deployments. InfluxDB 3
is retained for time-oriented operations, but its non-transactional point write
boundary requires a visible durable spool instead of pretending it has SQL
transaction semantics.

## Consequences and acceptance checks

- Database latency and failure cannot block the scan path.
- Each configured backend is a discrete adapter, not conditionals spread
  through the worker.
- Shared batching, retry, and status code depends only on sink contracts.
- Loss and placeholders are first-class persisted documents.
- Every known event is projected by one shared Rust owner into documented,
  descriptive typed tables; adapters do not duplicate semantic dispatch.
- Relational projection rows and checkpoint are immediately consistent because
  they share one transaction.
- Unknown future events remain visible in `event_log`, canonical and counted as
  unclassified rather than guessed or discarded.
- No source-local order is converted into an invented global order.
- No new module may approach 1,000 lines; adapter and migration ownership stays
  split when necessary.
- Repeated serialization, resolution, and identity logic must be factored once.
- A backend is not documented as supported until the real product passes the
  complete common manifest and failure/restart matrix.

## Rejected alternatives

- Writing from the PLC scan: violates timing and failure isolation.
- Extending OpenOT carriage with database concerns: puts product policy in the
  standards-facing contract.
- Persisting unresolved slots: loses semantic fidelity and definition safety.
- SQLite-only: does not meet the deployment requirement.
- One generic URL plus inferred driver: makes the effective backend ambiguous.
- Automatic fallback: can split history and conceal an outage.
- Mandatory SQLite spool for every remote SQL backend: adds an unrequested
  second durable system and changes acknowledgement semantics.
- Treating InfluxDB acknowledgement as a relational transaction: makes a
  durability claim its API cannot supply.
- Canonical JSON as the ordinary query API: preserves data but makes common
  value/alarm/message queries need OpenOT array and JSON-path knowledge.
- Per-product JSON views/generated columns: creates seven semantic
  implementations and no coherent InfluxDB contract.
- One generic EAV field table: moves complexity from JSON paths to pivots and
  joins without producing a user-friendly logging model.
- Typed projections without canonical storage: loses future extension fields,
  placeholder raw slots, deterministic replay, and corruption comparison.
- An asynchronous relational projector: adds avoidable lag and a second
  checkpoint/failure state despite the existing atomic sink transaction.
