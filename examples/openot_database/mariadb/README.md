# MariaDB OpenOT persistence

## Prerequisites

Provision MariaDB 11.8 LTS separately with TLS required, database `trust_logging`, user
`trust_logging_writer`, a CA file, and the `mariadb` CLI. It uses `backend = "mysql"`
but requires its own product proof. The source remains `../workload/Main.st`.

## Prepare and run

```bash
export TRUST_OPENOT_DATABASE_URL='mysql://trust_logging_writer:FROM_SECRET_STORE@db.example:3306/trust_logging'
example_root=$(mktemp -d /tmp/trust-openot-mariadb.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/certs"
cp ../workload/Main.st "$example_root/src/Main.st"
cp runtime.toml "$example_root/runtime.toml"
cp /approved/path/openot-database-ca.pem "$example_root/certs/openot-database-ca.pem"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

## Verify

```bash
mariadb --ssl-verify-server-cert --ssl-ca="$example_root/certs/openot-database-ca.pem" -h db.example -u trust_logging_writer -p trust_logging -e 'select version()'
mariadb --ssl-verify-server-cert --ssl-ca="$example_root/certs/openot-database-ca.pem" -h db.example -u trust_logging_writer -p trust_logging -e \
  'select event_name,count(*) from event_log group by 1 order by 1'
```

## Outage and restart

Stop/restart this MariaDB product, observe retrying, and prove ordered catch-up.
A passing MySQL run does not satisfy this procedure.

## Backup and restore

Use MariaDB-native transaction-consistent backup/restore, preserving documents,
checkpoint, binary identity collation, and schema version together.

## Clean up

Stop truST, drop only the disposable database/user, and remove the temporary
project. Keep any shared MariaDB service.
