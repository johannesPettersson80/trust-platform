# Verify real OpenOT databases

Release support is accepted only when the same canonical ST workload and
coverage manifest pass through the production adapter against the named real
database product. A protocol mock, compile-only run, plain PostgreSQL in place
of TimescaleDB, MySQL in place of MariaDB, or local SQL Server in place of Azure
SQL is not evidence.

The current reviewed matrix runs on the x86_64 `trust-builder` and pins:

| Product | Reviewed version/image |
| --- | --- |
| PostgreSQL | 18.6, `postgres:18.6` |
| TimescaleDB | 2.29.2 on PG18, `timescale/timescaledb:2.29.2-pg18` |
| MySQL | 8.4.11, `mysql:8.4.11` |
| MariaDB | 11.8.8, `mariadb:11.8.8` |
| SQL Server | 2025 CU8 / 17.0.4075.5, `mcr.microsoft.com/mssql/server:2025-CU8-ubuntu-22.04` |
| InfluxDB | 3.11.2 Core, `influxdb:3.11.2-core` |

SQLite uses the bundled SQLite library and a real mode-restricted on-disk file;
it is the seventh product in the matrix even though it has no container image.

For each product, retain redacted TOML, immutable image digest, architecture,
server/client version, TLS/readiness proof, migration version, canonical JSON
comparison, document/checkpoint counts, outage/reconnect status snapshots, and
clean teardown output. The native executable identities are:

- `openot_persistence::contract_tests` for migrations, transactions,
  idempotency, canonical JSON, checkpointing, and product-native assertions;
- `openot_database_example_persists_same_real_st_workload_to_every_network_backend`
  for the shared ST-to-ring-to-document-to-database workload;
- `runtime_system_loss_and_placeholder_documents_round_trip_through_every_real_product`
  for all nine pinned runtime system events, both loss bases, a raw-slot
  placeholder, direct manifest comparison, and exact seven-product retrieval;
- `every_real_network_backend_migrates_v1_and_rejects_newer_schema` for every
  implementation-history migration and newer-version refusal;
- the product lifecycle tests for forced disconnect, restart, catch-up, and
  cursor/head reconciliation.

The exact test commands, after exporting the documented URL/host/token and CA
environment variables from a secret store, are:

```bash
# Runs the complete matrix below, records redacted candidate/product metadata,
# and writes checksummed logs below target/openot-real-database-evidence/.
scripts/openot_real_database_gate.sh

cargo test -p trust-runtime --features openot-real-database-tests --lib \
  openot_persistence::contract_tests -- --test-threads=1
cargo test -p trust-runtime --features openot-real-database-tests \
  --test openot_telemetry \
  openot_database_example_persists_same_real_st_workload_to_every_network_backend \
  -- --exact
cargo test -p trust-runtime --features openot-real-database-tests \
  --test openot_database_system_documents \
  runtime_system_loss_and_placeholder_documents_round_trip_through_every_real_product \
  -- --exact
```

The same script is the body of the required weekly/manual
`OpenOT real databases` workflow and a hard dependency of the tag-triggered
Release workflow. Its self-hosted runner provisioner must start the pinned real
products and issue ephemeral secrets/CA material; the workflow always tears
down that disposable state and retains the redacted evidence artifact for 30
days. A missing runner, secret, CA file, product, artifact, or teardown is a
failed gate, not permission to skip a backend.

Required environment variable names are
`TRUST_TEST_OPENOT_POSTGRES_URL`/`_CA`,
`TRUST_TEST_OPENOT_TIMESCALE_URL`/`_CA`,
`TRUST_TEST_OPENOT_MYSQL_URL`/`_CA`,
`TRUST_TEST_OPENOT_MARIADB_URL`/`_CA`,
`TRUST_TEST_OPENOT_SQLSERVER_URL`/`_CA`, and
`TRUST_TEST_OPENOT_INFLUX_HOST`/`_TOKEN`/`_CA`. Restart tests additionally
receive the six `TRUST_TEST_OPENOT_*_CONTAINER` names. Never paste their values
into tracked files or retained command logs.

Run feature-gated tests with secrets supplied by the ephemeral runner, not
embedded in command logs. Stop and restart the actual named container/server,
without changing TOML. Acceptance requires `cursor_abs == head_abs`, zero
unexpected unresolved/loss documents, zero Influx `remote_pending`, and exact
manifest retrieval. Teardown removes only disposable state created by the run.

Minimum versions are not claimed from this current-version matrix. A release
lists only the exact product/version range proven on its exact candidate SHA.
