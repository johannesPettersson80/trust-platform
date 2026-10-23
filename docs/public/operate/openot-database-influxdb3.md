# Operate OpenOT with InfluxDB 3

InfluxDB 3 HTTP writes cannot atomically advance the source checkpoint, so
truST requires a bounded local SQLite spool. Canonical documents and checkpoint
commit there first; remote writes drain in order. `remote_pending > 0` means
locally durable but not remotely acknowledged, and prevents `ready`.

Use the repository's `examples/openot_database/influxdb3/` example
for token/CA setup, SQL query, spool inspection, outage/recovery, backup, and
cleanup. Set `max_bytes` above the spool schema footprint and leave filesystem
headroom for WAL/allocation. Full spool rolls back the incoming batch and
checkpoint and faults visibly.

The spool is initialized directly as schema generation 1. A markerless,
incomplete, or differently marked pre-release spool is rejected without
modification; back it up and recreate the development spool.

Back up both the Influx database and local spool consistently. Never delete or
replace a nonempty spool during troubleshooting. Monitor spool pages,
`remote_pending`, HTTP latency/errors, ring lag/loss, and free space. Test
server/spool upgrades and restore on the exact supported InfluxDB 3 product.
