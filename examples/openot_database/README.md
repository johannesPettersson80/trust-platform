# OpenOT database examples

These examples run one canonical Structured Text workload against every
supported database product. The source and expected coverage manifest live in
`workload/`; each product directory contains only its authoritative
`runtime.toml` and product-specific operating instructions. This prevents an
example from gaining or losing an event merely because its database changed.

The ST workload emits typed values, all four sampling policies, audited parameter
changes, four state-model categories, alarms, interlocks, the complete
condition lifecycle, typed messages, recipe/batch/material records,
operator/security records, and an electronic signature. It contains no SQL
calls. System records are runtime-authored, loss is consumer-authored from
authoritative or inferred gaps, and placeholders are resolver-authored when a
definition cannot safely resolve a record; ST deliberately cannot forge those
document classes. The release verification pairs this workload with reviewed
runtime lifecycle, forced-overflow, and safe definition-mismatch fixtures so
all three document variants are retrieved from every database.

To prepare a runnable project, run these from the repository root: fetch the
OpenOT ST library once (the pinned revision, beside the repository in
`../open-ot-ref`), copy the workload, its `io.toml` and exactly one backend TOML,
and name the library in the project's `trust-lsp.toml`. The producers truST
generates from the workload's `oot` attributes are the library's function blocks.

```bash
bash scripts/checkout_openot_ref.sh
example_root=$(mktemp -d /tmp/trust-openot-example.XXXXXX)
mkdir -p "$example_root/src"
cp examples/openot_database/workload/Main.st "$example_root/src/Main.st"
cp examples/openot_database/workload/io.toml "$example_root/io.toml"
cp examples/openot_database/sqlite/runtime.toml "$example_root/runtime.toml"
printf '[dependencies]\nOpenOT = { path = "%s" }\n' "$(cd ../open-ot-ref/st/iec61131 && pwd)" > "$example_root/trust-lsp.toml"
trust-runtime build --project "$example_root" --sources src
trust-runtime run --project "$example_root"
```

This copies `sqlite`, which needs no server; choose one of `sqlite`, `postgresql`,
`timescaledb`, `mysql`, `mariadb`, `sqlserver`, or `influxdb3`, and follow that
product's README. Remote examples reference environment variables for
credentials and require CA files; no tracked file contains a secret.

The complete, release-gating real-product procedure is documented in
`docs/public/operate/openot-database-verification.md`.
