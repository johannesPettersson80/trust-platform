# Operate OpenOT with MySQL or MariaDB

The `mysql` selector owns one protocol adapter, but support is verified and
operated separately for MySQL 8.4 LTS and MariaDB 11.8 LTS. Require encrypted
transport, provide a reviewed CA, keep the URL in the named environment
variable, and use a least-privilege InnoDB database.

Follow the separate `examples/openot_database/mysql/` or
`examples/openot_database/mariadb/` example. Each
contains native version/query, restart, backup, restore, and cleanup guidance.
Binary document identities and complete canonical JSON prevent collation from
turning conflicting payloads into ordinary duplicates.

Monitor connection/transaction latency, InnoDB growth, retry/lag/loss, and
backup health. Test vendor upgrades and restores against the same product; a
MySQL run is not MariaDB proof. Both products initialize the same schema
generation 1 directly and reject incompatible pre-release databases without
altering them.
