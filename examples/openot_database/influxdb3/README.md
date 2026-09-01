# InfluxDB 3 OpenOT persistence

## Prerequisites

Provision the reviewed InfluxDB 3 Core version with HTTPS, database `trust_logging`, an
API token, and CA. Install `curl` and `sqlite3`. The mandatory bounded SQLite
spool preserves full documents/checkpoints around the remote point API. The ST
source is shared `../workload/Main.st`.

## Prepare and run

```bash
export TRUST_OPENOT_INFLUX_HOST='https://db.example:8181'
export TRUST_OPENOT_INFLUX_TOKEN='FROM_SECRET_STORE'
example_root=$(mktemp -d /tmp/trust-openot-influxdb3.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/certs" "$example_root/history"
cp ../workload/Main.st "$example_root/src/Main.st"
cp runtime.toml "$example_root/runtime.toml"
cp /approved/path/openot-influx-ca.pem "$example_root/certs/openot-influx-ca.pem"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

## Verify

```bash
curl --fail --cacert "$example_root/certs/openot-influx-ca.pem" \
  -H "Authorization: Bearer $TRUST_OPENOT_INFLUX_TOKEN" \
  --get "$TRUST_OPENOT_INFLUX_HOST/api/v3/query_sql" --data-urlencode 'db=trust_logging' \
  --data-urlencode 'q=select event_name,count(*) from event_log group by 1 order by 1'
sqlite3 "$example_root/history/trust-logging-influx-spool.sqlite3" \
  'select delivered,count(*) from logging_delivery_spool group by delivered;'
```

## Outage and restart

Stop InfluxDB while truST runs. The spool must durably accept until its explicit
limit, expose `remote_pending`, and drain in order after server/process restart.
Never call it caught up while `remote_pending > 0`.

## Backup and restore

Use vendor tooling for Influx data and SQLite online backup for the spool.
Restore both before starting truST; never discard a nonempty spool.

## Clean up

Stop truST, verify the spool has no undelivered row, remove only the disposable
database/token and temporary project, then stop the disposable Influx service.
