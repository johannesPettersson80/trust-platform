# PostgreSQL OpenOT persistence

## Prerequisites

Provision the reviewed PostgreSQL version with TLS required, create database
`trust_logging` and least-privilege user `trust_logging_writer`, install `psql`, and place the
issuing CA at `certs/openot-database-ca.pem`. The shared source is
`../workload/Main.st`.

## Prepare and run

Run these from the repository root. The first line fetches the OpenOT ST library once (the pinned
revision, beside the repository in `../open-ot-ref`); the project's `trust-lsp.toml` names it, because
the producers truST generates from the workload's `oot` attributes are its function blocks.

```bash
bash scripts/checkout_openot_ref.sh
export TRUST_OPENOT_DATABASE_URL='host=db.example port=5432 user=trust_logging_writer dbname=trust_logging sslmode=require password=FROM_SECRET_STORE'
example_root=$(mktemp -d /tmp/trust-openot-postgresql.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/certs"
cp examples/openot_database/workload/Main.st "$example_root/src/Main.st"
cp examples/openot_database/workload/io.toml "$example_root/io.toml"
cp examples/openot_database/postgresql/runtime.toml "$example_root/runtime.toml"
printf '[dependencies]\nOpenOT = { path = "%s" }\n' "$(cd ../open-ot-ref/st/iec61131 && pwd)" > "$example_root/trust-lsp.toml"
cp /approved/path/openot-database-ca.pem "$example_root/certs/openot-database-ca.pem"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

## Verify

```bash
psql "$TRUST_OPENOT_DATABASE_URL" -c 'select version()'
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  'select event_name,count(*) from trust_logging.event_log group by 1 order by 1'
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  'select buffer_id,encode(run_id,'\''hex'\''),encode(cursor_abs,'\''hex'\'') from trust_logging.logging_checkpoint'
```

## Outage and restart

Stop the real server without changing TOML, confirm `retrying` while PLC
execution continues, restart it, and require `cursor_abs == head_abs` with no
backend fallback.

## Backup and restore

Use transaction-consistent `pg_dump`/`pg_restore` for schema `trust_logging`. Preserve
documents and checkpoint as one recovery unit and validate the schema version.

## Clean up

Stop truST, drop only the disposable example schema/database, and remove the
temporary project. Do not remove a shared PostgreSQL instance.
