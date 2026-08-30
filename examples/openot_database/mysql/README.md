# MySQL OpenOT persistence

## Prerequisites

Provision the reviewed MySQL 8.4 LTS product with TLS required, database
`openot`, user `openot_logger`, and a CA file. Install the MySQL CLI. This
overlay runs unchanged shared `../workload/Main.st`.

## Prepare and run

```bash
export TRUST_OPENOT_DATABASE_URL='mysql://openot_logger:FROM_SECRET_STORE@db.example:3306/openot'
example_root=$(mktemp -d /tmp/trust-openot-mysql.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/certs"
cp ../workload/Main.st "$example_root/src/Main.st"
cp runtime.toml "$example_root/runtime.toml"
cp /approved/path/openot-database-ca.pem "$example_root/certs/openot-database-ca.pem"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

## Verify

```bash
mysql --ssl-mode=VERIFY_CA --ssl-ca="$example_root/certs/openot-database-ca.pem" -h db.example -u openot_logger -p openot -e 'select version()'
mysql --ssl-mode=VERIFY_CA --ssl-ca="$example_root/certs/openot-database-ca.pem" -h db.example -u openot_logger -p openot -e \
  'select document_kind,event_name,count(*) from openot_documents group by 1,2 order by 1,2'
```

## Outage and restart

Stop/restart the same MySQL server, observe `retrying`, and require ordered
catch-up without changing the TOML discriminator or silently selecting SQLite.

## Backup and restore

Use a transaction-consistent InnoDB backup and restore `openot_documents` with
`openot_checkpoint`; never reset one independently of the other.

## Clean up

Stop truST, drop only the disposable `openot` database/user, and remove the
temporary project. Do not remove a shared MySQL service.
