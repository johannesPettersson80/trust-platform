# Phase 18 control-client focused evidence

Scope: `trust-runtime ctl` target resolution, request construction, bounded
TCP/Unix exchange, response correlation and classification, and successful
status/health/task rendering.

## Expected-red evidence

Remote isolated checkout:
`/home/johannes/projects/trust-platform-p18-parser-proof-20260723` on
`trust-builder`.

```text
cargo test -p trust-runtime --bin trust-runtime \
  ctl::tests::control_exchange_uses_one_json_line_and_rejects_invalid_responses \
  -- --exact --nocapture --test-threads=1
```

Failed because `{"ok":true}` without a response ID was accepted:

```text
missing or mismatched response ID must fail: Object {"ok": Bool(true)}
```

```text
cargo test -p trust-runtime --test control_cli_command \
  ctl_returns_nonzero_for_malformed_success_results \
  -- --exact --nocapture --test-threads=1
```

Failed because an `ok=true` status envelope with an empty result exited zero and
printed raw JSON.

## Focused green evidence

```text
cargo test -p trust-runtime --bin trust-runtime ctl::tests:: \
  -- --nocapture --test-threads=1
```

Result: 5 passed, 0 failed.

```text
cargo test -p trust-runtime --test control_cli_command \
  -- --nocapture --test-threads=1
```

Result: 7 passed, 0 failed.

```text
cargo test -p trust-runtime --test ads_cli_command \
  ads_server_json_rejection_exits_nonzero_and_preserves_error \
  -- --exact --nocapture --test-threads=1
```

Result: 1 passed, 0 failed.

No broad workspace test, `just fmt`, `just clippy`, or `just test-all` command
was run for this batch.
