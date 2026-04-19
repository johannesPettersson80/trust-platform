# `trust-harness`

`trust-harness` is the standalone deterministic executor for programmable cycle
driving.

## Invocation Model

`trust-harness` is not a traditional subcommand CLI. It reads newline-delimited
JSON requests from `stdin` and writes newline-delimited JSON responses to
`stdout`.

## Example Session

```bash
cat <<'EOF' | trust-harness
{"cmd":"load","source":"PROGRAM Main\nVAR\n  q : BOOL;\nEND_VAR\nq := TRUE;\nEND_PROGRAM\n"}
{"cmd":"get_output","name":"q"}
EOF
```

## What it is for

- deterministic docs/examples
- CI loops
- agent repair loops
- programmable scan-cycle control without full runtime startup

## Current Commands

- `load`
- `reload`
- `cycle`
- `set_input`
- `get_output`
- `set_access`
- `get_access`
- `bind_direct`
- `set_direct_input`
- `get_direct_output`
- `advance_time`
- `run_until`
- `restart`
- `snapshot`

## Related

- [Harness Protocol](../harness/protocol.md)
- [Deterministic Harness concept](../../concepts/deterministic-harness.md)
