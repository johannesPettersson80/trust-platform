# PostgreSQL OpenOT persistence

## Prerequisites

Provision the reviewed PostgreSQL version with TLS required, create database
`openot` and least-privilege user `openot_logger`, install `psql`, and place the
issuing CA at `certs/openot-database-ca.pem`. The shared source is
`../workload/Main.st`.

## Prepare and run

```bash
export TRUST_OPENOT_DATABASE_URL='host=db.example port=5432 user=openot_logger dbname=openot sslmode=require password=FROM_SECRET_STORE'
example_root=$(mktemp -d /tmp/trust-openot-postgresql.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/certs"
cp ../workload/Main.st "$example_root/src/Main.st"
cp runtime.toml "$example_root/runtime.toml"
cp /approved/path/openot-database-ca.pem "$example_root/certs/openot-database-ca.pem"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

## Verify

```bash
psql "$TRUST_OPENOT_DATABASE_URL" -c 'select version()'
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  'select document_kind,event_name,count(*) from openot.openot_documents group by 1,2 order by 1,2'
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  'select buffer_id,encode(run_id,'\''hex'\''),encode(cursor_abs,'\''hex'\'') from openot.openot_checkpoint'
```

## Outage and restart

Stop the real server without changing TOML, confirm `retrying` while PLC
execution continues, restart it, and require `cursor_abs == head_abs` with no
backend fallback.

## Backup and restore

Use transaction-consistent `pg_dump`/`pg_restore` for schema `openot`. Preserve
documents and checkpoint as one recovery unit and validate the schema version.

## Clean up

Stop truST, drop only the disposable example schema/database, and remove the
temporary project. Do not remove a shared PostgreSQL instance.
