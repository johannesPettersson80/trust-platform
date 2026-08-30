# MariaDB OpenOT persistence

## Prerequisites

Provision MariaDB 11.8 LTS separately with TLS required, database `openot`, user
`openot_logger`, a CA file, and the `mariadb` CLI. It uses `backend = "mysql"`
but requires its own product proof. The source remains `../workload/Main.st`.

## Prepare and run

```bash
export TRUST_OPENOT_DATABASE_URL='mysql://openot_logger:FROM_SECRET_STORE@db.example:3306/openot'
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
mariadb --ssl-verify-server-cert --ssl-ca="$example_root/certs/openot-database-ca.pem" -h db.example -u openot_logger -p openot -e 'select version()'
mariadb --ssl-verify-server-cert --ssl-ca="$example_root/certs/openot-database-ca.pem" -h db.example -u openot_logger -p openot -e \
  'select document_kind,event_name,count(*) from openot_documents group by 1,2 order by 1,2'
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
