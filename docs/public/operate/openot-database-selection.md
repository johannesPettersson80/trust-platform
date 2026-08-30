# Choose an OpenOT database

The `runtime.openot.persistence.backend` TOML value is the only backend
selector. truST never infers a backend from a URL, installed client, or service
reachability, and never falls back after a connection failure.

| Selector | Verified product | Deployment | Atomic authority | Offline behavior | Main limitation |
| --- | --- | --- | --- | --- | --- |
| `sqlite` | bundled SQLite 3 | local file | SQLite transaction | continues until local disk/limit fails | single-host operations |
| `postgresql` | PostgreSQL 18.6 | central server | server transaction | retries; ring remains bounded | server/TLS operations required |
| `timescaledb` | TimescaleDB 2.29.2 on PG18 | central server | PostgreSQL transaction | same retry boundary as PostgreSQL | extension lifecycle is operator-owned |
| `mysql` | MySQL 8.4.11 | central server | InnoDB transaction | retries; ring remains bounded | product-specific TLS/collation operations |
| `mysql` | MariaDB 11.8.8 | central server | InnoDB transaction | retries; ring remains bounded | verified separately from MySQL |
| `sqlserver` | SQL Server 2025 CU8 | central server | TDS transaction | retries; ring remains bounded | Azure SQL is not yet claimed |
| `influxdb3` | InfluxDB 3.11.2 Core | central server plus local spool | local SQLite spool transaction | accepts until bounded spool is full | remote delivery is not atomic with HTTP write |

All remote products require CA-verified TLS and environment-sourced secrets.
SQLite is the simplest isolated deployment. Choose a server product when site
operations, central querying, backup, or retention tooling justify the added
service. TimescaleDB and InfluxDB are useful for time-oriented queries, but
OpenOT remains an event/audit stream rather than a high-frequency waveform
historian.

Follow the product page for setup and lifecycle, and the
[real-database verification matrix](openot-database-verification.md) before
claiming support for a release candidate.
