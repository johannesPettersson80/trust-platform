# SQL Server OpenOT persistence

## Prerequisites

Provision the reviewed SQL Server 2025 product with forced encryption and a
CA-verifiable certificate. Create the database/login, install `sqlcmd`, place
the CA at `certs/openot-database-ca.pem`, and use `../workload/Main.st`. Azure
SQL is intentionally not claimed without a separate real-service run.

## Prepare and run

Run these from the repository root. The first line fetches the OpenOT ST library once (the pinned
revision, beside the repository in `../open-ot-ref`); the project's `trust-lsp.toml` names it, because
the producers truST generates from the workload's `oot` attributes are its function blocks.

```bash
bash scripts/checkout_openot_ref.sh
export TRUST_OPENOT_DATABASE_URL='server=tcp:db.example,1433;user=trust_logging_writer;password=FROM_SECRET_STORE;database=trust_logging;TrustServerCertificate=false'
example_root=$(mktemp -d /tmp/trust-openot-sqlserver.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/certs"
cp examples/openot_database/workload/Main.st "$example_root/src/Main.st"
cp examples/openot_database/workload/io.toml "$example_root/io.toml"
cp examples/openot_database/sqlserver/runtime.toml "$example_root/runtime.toml"
printf '[dependencies]\nOpenOT = { path = "%s" }\n' "$(cd ../open-ot-ref/st/iec61131 && pwd)" > "$example_root/trust-lsp.toml"
cp /approved/path/openot-database-ca.pem "$example_root/certs/openot-database-ca.pem"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

## Verify

```bash
sqlcmd -N -S db.example -d trust_logging -Q 'select @@version'
sqlcmd -N -S db.example -d trust_logging -Q \
  'select event_name,count_big(*) from trust_logging.event_log group by event_name order by event_name'
sqlcmd -N -S db.example -d trust_logging -Q \
  "select encrypt_option from sys.dm_exec_connections where session_id=@@spid"
```

## Outage and restart

Stop/restart the real SQL Server process, observe retry state without PLC
failure, and require exact catch-up with the SQL Server backend still selected.

## Backup and restore

Use SQL Server transaction-consistent backup/restore and preserve the OpenOT
schema and checkpoint. Validate schema and `ISJSON(canonical_json)=1` afterward.

## Clean up

Stop truST, drop only the disposable example database/login, and remove the
temporary project. Never delete a shared SQL Server instance.
