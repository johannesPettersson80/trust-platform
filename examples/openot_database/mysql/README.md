# MySQL OpenOT persistence

## Prerequisites

Provision the reviewed MySQL 8.4 LTS product with TLS required, database
`trust_logging`, user `trust_logging_writer`, and a CA file. Install the MySQL CLI. This
overlay runs unchanged shared `../workload/Main.st`.

## Prepare and run

Run these from the repository root. The first line fetches the OpenOT ST library once (the pinned
revision, beside the repository in `../open-ot-ref`); the project's `trust-lsp.toml` names it, because
the producers truST generates from the workload's `oot` attributes are its function blocks.

```bash
bash scripts/checkout_openot_ref.sh
export TRUST_OPENOT_DATABASE_URL='mysql://trust_logging_writer:FROM_SECRET_STORE@db.example:3306/trust_logging'
example_root=$(mktemp -d /tmp/trust-openot-mysql.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/certs"
cp examples/openot_database/workload/Main.st "$example_root/src/Main.st"
cp examples/openot_database/workload/io.toml "$example_root/io.toml"
cp examples/openot_database/mysql/runtime.toml "$example_root/runtime.toml"
printf '[dependencies]\nOpenOT = { path = "%s" }\n' "$(cd ../open-ot-ref/st/iec61131 && pwd)" > "$example_root/trust-lsp.toml"
cp /approved/path/openot-database-ca.pem "$example_root/certs/openot-database-ca.pem"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

## Verify

```bash
mysql --ssl-mode=VERIFY_CA --ssl-ca="$example_root/certs/openot-database-ca.pem" -h db.example -u trust_logging_writer -p trust_logging -e 'select version()'
mysql --ssl-mode=VERIFY_CA --ssl-ca="$example_root/certs/openot-database-ca.pem" -h db.example -u trust_logging_writer -p trust_logging -e \
  'select event_name,count(*) from event_log group by 1 order by 1'
```

## Outage and restart

Stop/restart the same MySQL server, observe `retrying`, and require ordered
catch-up without changing the TOML discriminator or silently selecting SQLite.

## Backup and restore

Use a transaction-consistent InnoDB backup and restore `logging_records` with
`logging_checkpoint`; never reset one independently of the other.

## Clean up

Stop truST, drop only the disposable `trust_logging` database/user, and remove the
temporary project. Do not remove a shared MySQL service.
