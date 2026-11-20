# TimescaleDB OpenOT persistence

## Prerequisites

Provision the reviewed TimescaleDB-on-PostgreSQL version with TLS and its
extension enabled. Create the database/user, install `psql`, place the CA at
`certs/openot-database-ca.pem`, and use shared `../workload/Main.st`.

## Prepare and run

Run these from the repository root. The first line fetches the OpenOT ST library once (the pinned
revision, beside the repository in `../open-ot-ref`); the project's `trust-lsp.toml` names it, because
the producers truST generates from the workload's `oot` attributes are its function blocks.

```bash
bash scripts/checkout_openot_ref.sh
export TRUST_OPENOT_DATABASE_URL='host=db.example port=5432 user=trust_logging_writer dbname=trust_logging sslmode=require password=FROM_SECRET_STORE'
example_root=$(mktemp -d /tmp/trust-openot-timescaledb.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/certs"
cp examples/openot_database/workload/Main.st "$example_root/src/Main.st"
cp examples/openot_database/workload/io.toml "$example_root/io.toml"
cp examples/openot_database/timescaledb/runtime.toml "$example_root/runtime.toml"
printf '[dependencies]\nOpenOT = { path = "%s" }\n' "$(cd ../open-ot-ref/st/iec61131 && pwd)" > "$example_root/trust-lsp.toml"
cp /approved/path/openot-database-ca.pem "$example_root/certs/openot-database-ca.pem"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

## Verify

```bash
psql "$TRUST_OPENOT_DATABASE_URL" -c "select extversion from pg_extension where extname='timescaledb'"
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  "select hypertable_name from timescaledb_information.hypertables where hypertable_name in ('event_log','logged_values','alarm_history','message_log','state_history') order by hypertable_name"
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  "select time_bucket('1 minute',received_time),count(*) from trust_logging.event_log group by 1 order by 1"
```

## Outage and restart

Stop/restart the actual TimescaleDB server, observe `retrying`, and require
ordered catch-up with Timescale still selected. Plain PostgreSQL is not proof.

## Backup and restore

Use Timescale-supported PostgreSQL procedures for the relational table and
hypertable. truST installs no retention/compression policy; operators own it.

## Clean up

Stop truST, remove only the disposable database/schema and temporary project.
Keep any shared extension/server intact.
