# Operate OpenOT with SQLite

Select `sqlite` and a bundle-relative path; truST creates a missing final parent
with mode `0700` and rejects a group/world-writable database directory. WAL and
full synchronous durability are owned by the adapter. No credential is used.

Use the repository's `examples/openot_database/sqlite/` example for
build, inspection, integrity check, backup, and reset commands. Monitor file,
WAL, free-space, lag, loss, and runtime persistence status. Back up through the
SQLite online-backup API or after clean shutdown; restore both documents and
checkpoint from one consistent copy. Run `PRAGMA integrity_check` before
restarting. truST migrates schema 1 to 2 transactionally and rejects newer or
corrupt schemas without recreating them.

Retention and compaction are operator-owned. Do not delete individual audit
rows or checkpoint state. For upgrade, back up first, start the newer runtime
once, verify schema/status/counts, and retain the backup until rollback is no
longer required. A runtime too old for the database fails closed.
