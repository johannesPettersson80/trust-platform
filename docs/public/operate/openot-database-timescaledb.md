# Operate OpenOT with TimescaleDB

TimescaleDB uses PostgreSQL transport and transactions but has separate schema
ownership: startup verifies the extension and creates the approved hypertable
projection. Plain PostgreSQL is not a substitute.

Use the repository's `examples/openot_database/timescaledb/` example
for extension/hypertable checks, time-range queries, restart, backup, and
cleanup. TLS, secrets, retry, readiness, and checkpoint rules match the
PostgreSQL adapter. Retention and compression policies are not installed by
truST; approve them against audit requirements before applying them.

Back up relational documents, checkpoint, and Timescale catalog/projection
consistently. Test extension upgrades and restore on a disposable real server,
then rerun the canonical workload. A missing/incompatible extension fails
startup rather than silently degrading to plain PostgreSQL.
