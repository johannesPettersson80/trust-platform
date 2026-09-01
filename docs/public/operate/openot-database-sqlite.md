# Operate OpenOT with SQLite

Select `sqlite` and a bundle-relative path; truST creates a missing final parent
with mode `0700` and rejects a group/world-writable database directory. WAL and
full synchronous durability are owned by the adapter. No credential is used.

Use the repository's `examples/openot_database/sqlite/` example for
build, inspection, integrity check, backup, and reset commands. Monitor file,
WAL, free-space, lag, loss, and runtime persistence status. Back up through the
SQLite online-backup API or after clean shutdown; restore both documents and
checkpoint from one consistent copy. Run `PRAGMA integrity_check` before
restarting. A fresh database is initialized directly as schema generation 1.
Any markerless, incomplete, or differently marked pre-release database is
rejected without modification; back it up and recreate it.

Retention and compaction are operator-owned. Do not delete individual audit
rows or checkpoint state. Before a future released schema upgrade exists, use
only generation 1; development databases from earlier builds are not migrated.
