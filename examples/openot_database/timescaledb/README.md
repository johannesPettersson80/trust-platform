# TimescaleDB OpenOT persistence

## Prerequisites

Provision the reviewed TimescaleDB-on-PostgreSQL version with TLS and its
extension enabled. Create the database/user, install `psql`, place the CA at
`certs/openot-database-ca.pem`, and use shared `../workload/Main.st`.

## Prepare and run

```bash
export TRUST_OPENOT_DATABASE_URL='host=db.example port=5432 user=openot_logger dbname=openot sslmode=require password=FROM_SECRET_STORE'
example_root=$(mktemp -d /tmp/trust-openot-timescaledb.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/certs"
cp ../workload/Main.st "$example_root/src/Main.st"
cp runtime.toml "$example_root/runtime.toml"
cp /approved/path/openot-database-ca.pem "$example_root/certs/openot-database-ca.pem"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

## Verify

```bash
psql "$TRUST_OPENOT_DATABASE_URL" -c "select extversion from pg_extension where extname='timescaledb'"
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  "select hypertable_name from timescaledb_information.hypertables where hypertable_name='openot_time_index'"
psql "$TRUST_OPENOT_DATABASE_URL" -c \
  'select time_bucket(60000000000,receive_time_ns),count(*) from openot.openot_time_index group by 1 order by 1'
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
