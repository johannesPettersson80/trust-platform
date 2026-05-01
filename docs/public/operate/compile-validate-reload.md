# Compile, Validate, Reload / Hot Reload

This is the public hot-reload path for truST: diagnose first, then rebuild, then
reload only when the project is clean enough to run.

## For automated tools

The preferred machine-facing loop is:

1. `workspace.write`
2. `lsp.diagnostics`
3. `lsp.format`
4. `runtime.compile_reload`

`runtime.compile_reload` is the one-call repair loop because it:

- collects diagnostics
- blocks on errors
- builds bytecode when diagnostics are clean
- sends `bytecode.reload` to the runtime control endpoint

## Example JSON-RPC Session

```bash
cat <<'EOF' | trust-dev agent serve --project ./examples/memory_marker_counter
{"jsonrpc":"2.0","id":1,"method":"workspace.project_info","params":{}}
{"jsonrpc":"2.0","id":2,"method":"lsp.diagnostics","params":{}}
{"jsonrpc":"2.0","id":3,"method":"runtime.compile_reload","params":{}}
EOF
```

Important result fields:

- `errors`
- `warnings`
- `issues`
- `runtimeStatus`
- `runtimeMessage`
- optional `build`
- optional `reload`

Current `runtimeStatus` values are:

- `ok`
- `skipped`
- `error`

## When reload is skipped

Reload is intentionally blocked when diagnostics contain errors. That keeps
agents and editors from pushing obviously broken bytecode into a live runtime.
In other words, hot reload is guarded by diagnostics instead of replacing
bytecode in a broken runtime state.

## Editor Path

Humans usually take the same loop through:

- VS Code diagnostics
- formatting
- Runtime Panel / debugger

The point is the same even when the transport differs:
one consistent path from edit to validated running state.

## Related

- [Agent Quickstart](../start/agent-quickstart.md)
- [Build, Validate, Test](build-validate-test.md)
- [Agent API v1](../reference/agent-api/v1.md)
