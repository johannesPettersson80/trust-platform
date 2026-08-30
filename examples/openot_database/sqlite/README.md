# SQLite OpenOT persistence

## Prerequisites

Install `trust-runtime` and the `sqlite3` CLI. This example uses the shared
canonical source at `../workload/Main.st`; SQLite needs no server or credential.

## Prepare and run

```bash
example_root=$(mktemp -d /tmp/trust-openot-sqlite.XXXXXX)
install -d -m 700 "$example_root/src" "$example_root/history"
cp ../workload/Main.st "$example_root/src/Main.st"
cp runtime.toml "$example_root/runtime.toml"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

The database path is `history/openot.sqlite3`, resolved relative to the prepared
project. Stop the runtime cleanly after the workload completes.

## Verify

```bash
sqlite3 "$example_root/history/openot.sqlite3" 'PRAGMA integrity_check;'
sqlite3 -json "$example_root/history/openot.sqlite3" \
  'SELECT event_name,count(*) AS count FROM event_log GROUP BY 1 ORDER BY 1;'
sqlite3 "$example_root/history/openot.sqlite3" \
  'SELECT buffer_id,hex(run_id),hex(cursor_abs) FROM logging_checkpoint;'
```

Compare document families with `../workload/openot-coverage-manifest.json`;
release verification compares complete canonical JSON, not only summary counts.

## Outage and restart

Stop the runtime during an active batch and start it with the same project. The
transaction and WAL yield either the whole batch or none, and the checkpoint
catches up without duplicates.

## Backup and restore

Stop the runtime before copying files, or use SQLite's online `.backup` command.
Restore documents and checkpoint together and run `PRAGMA integrity_check`.

## Clean up

After stopping the runtime, remove only the disposable `$example_root` above.
Never delete a production database or WAL as an example reset.
