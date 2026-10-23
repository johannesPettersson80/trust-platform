# OpenOT database persistence example

This project is the canonical OpenOT workload for every supported persistence
backend. The Structured Text is identical for every run; select the database
only by choosing the corresponding TOML file.

The example logs:

- a templated message with four typed arguments and process, operating-mode,
  ISA-88, and PackML state transitions;
- `BOOL`, every supported signed and unsigned integer width, `REAL`, `LREAL`,
  and bounded `STRING` values;
- on-change, REAL deadband, periodic, and REAL hysteresis sampling declarations;
- an audited setpoint change with actor, reason, authorization, unit, and
  semantic role;
- alarm and interlock activation/clear plus acknowledgement, confirmation,
  shelving, suppression, service state, comment, reset, and priority change;
- recipe load/approval, material addition, and batch state;
- operator action, login, logout, security failure, and electronic signature.

There are no SQL calls or OpenOT opcodes in the application programs. truST
generates and drains these producer instances into one serialized ring:

```text
Filler.OotProducer
BatchControl.OotProducer
OperatorAudit.OotProducer
SignatureAudit.OotProducer
TypedValues.OotProducer
ConditionLifecycle.OotProducer
```

`examples/openot_multi_program/openot-coverage-manifest.json` is the
machine-readable inventory binding each event family, value type, sampling
policy, state model, condition class, message argument, and database product to
this one workload. The integration gate rejects a manifest for any other pinned
OpenOT revision or with an incomplete top-level inventory.

## Select a backend

| Product | Configuration | Secret environment variables |
|---|---|---|
| SQLite | `runtime.toml` | none |
| PostgreSQL | `runtime.postgresql.toml` | `TRUST_OPENOT_DATABASE_URL` |
| TimescaleDB | `runtime.timescaledb.toml` | `TRUST_OPENOT_DATABASE_URL` |
| MySQL | `runtime.mysql.toml` | `TRUST_OPENOT_DATABASE_URL` |
| MariaDB | `runtime.mariadb.toml` | `TRUST_OPENOT_DATABASE_URL` |
| SQL Server | `runtime.sqlserver.toml` | `TRUST_OPENOT_DATABASE_URL` |
| InfluxDB 3 | `runtime.influxdb3.toml` | `TRUST_OPENOT_INFLUX_HOST`, `TRUST_OPENOT_INFLUX_TOKEN` |

MySQL and MariaDB intentionally use the same `backend = "mysql"` adapter but
have separate runnable configurations and real-product verification.

To run a non-default configuration, copy the selected file to a temporary
project copy as `runtime.toml`; do not paste a password or token into TOML.
For example:

```bash
cp -a examples/openot_multi_program /tmp/trust-openot-postgresql-example
cp /tmp/trust-openot-postgresql-example/runtime.postgresql.toml \
  /tmp/trust-openot-postgresql-example/runtime.toml
trust-runtime build --project /tmp/trust-openot-postgresql-example --sources src
trust-runtime run --project /tmp/trust-openot-postgresql-example
```

For SQLite, run the checked-in default directly:

```bash
trust-runtime build --project examples/openot_multi_program --sources src
trust-runtime run --project examples/openot_multi_program
sqlite3 examples/openot_multi_program/history/trust-logging.sqlite3 \
  'SELECT event_name, COUNT(*) FROM event_log GROUP BY 1 ORDER BY 1;'
```

The build emits `openot-definition.json` beside the bytecode. Persistence uses
that exact definition to resolve the ring records into canonical OpenOT event,
loss, and placeholder documents. The database checkpoint advances in the same
durable transaction as its documents. See the public OpenOT database
persistence guide for backend setup, TLS, queries, restart, backup, and outage
behavior.

This is a deliberately broad conformance workload, not a production scan-time
template. It instruments many event families in one resource and can take a
long time per VM scan in an unoptimized development build. Measure the smaller
set of attributes required by the real machine against its cycle-time budget;
database commits remain on the separate host persistence thread.
