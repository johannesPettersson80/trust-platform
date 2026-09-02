# Operate OpenOT with SQL Server

Select `sqlserver`, use a dedicated schema/account, and supply the TDS
connection string through the named environment variable. Encryption with CA
verification is mandatory; `TrustServerCertificate=true` is not accepted as
production proof.

Use the repository's `examples/openot_database/sqlserver/` example
for native queries, restart, backup, restore, and cleanup. The adapter stores
canonical JSON plus binary-collated identity fields in one transaction with
the checkpoint. It initializes schema generation 1 directly and rejects
incompatible pre-release schemas without changing them.

Monitor sessions, transaction latency, database growth, runtime retries/lag,
and backup jobs. Test CU/major upgrades and recovery on a disposable real SQL
Server before production. Azure SQL is not documented as verified support
until the same candidate passes a real Azure SQL service run.
