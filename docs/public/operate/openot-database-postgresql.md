# Operate OpenOT with PostgreSQL

Provision a least-privilege role and database, require TLS, install the reviewed
CA in the bundle, and place the libpq connection string only in the environment
variable named by TOML. truST owns the selected schema and its migrations.

Use the repository's `examples/openot_database/postgresql/` example
for queries, checkpoint inspection, outage/restart, backup, restore, and clean
schema removal. Readiness requires a successful TLS connection, compatible
schema 3, and zero lag. During outage the service retries with bounded backoff;
the PLC continues and the ring remains bounded, so prolonged outage can become
explicit loss.

Monitor PostgreSQL connections, transaction latency, database growth, runtime
lag/loss/retry counters, and backups. Upgrade PostgreSQL under vendor guidance,
then run the exact adapter matrix before reconnecting production. Do not grant
the logger delete privileges or use undocumented columns as a public API.
