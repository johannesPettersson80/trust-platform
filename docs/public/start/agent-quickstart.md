# Agent Quickstart / `agent serve`

The entry command is `trust-dev agent serve`, so this is also the right
page when you are searching for `agent serve`.

## What an agent needs first

At minimum, an agent should know how to:

1. inspect project shape
2. read and write files
3. run diagnostics
4. preview formatting
5. build or test
6. close the diagnose -> build -> reload loop

## Start `agent serve`

```bash
trust-dev agent serve --project ./my-plc
```

Transport details:

- JSON-RPC 2.0
- stdio only in v1
- one request per line
- one response per line

## Five-minute Example

### 1. Describe the agent API

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"agent.describe","params":{}}' \
  | trust-dev agent serve --project ./examples/memory_marker_counter
```

### 2. Inspect the project

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"workspace.project_info","params":{}}' \
  | trust-dev agent serve --project ./examples/memory_marker_counter
```

### 3. Read diagnostics

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"lsp.diagnostics","params":{}}' \
  | trust-dev agent serve --project ./examples/memory_marker_counter
```

### 4. Preview formatting without mutating disk

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"lsp.format","params":{"path":"src/Main.st"}}' \
  | trust-dev agent serve --project ./examples/memory_marker_counter
```

### 5. Close the write -> validate -> reload loop

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"runtime.compile_reload","params":{}}' \
  | trust-dev agent serve --project ./examples/memory_marker_counter
```

## Methods most agents use first

- `agent.describe`
- `workspace.project_info`
- `workspace.read`
- `workspace.write`
- `lsp.diagnostics`
- `lsp.format`
- `runtime.build`
- `runtime.test`
- `runtime.compile_reload`

## Deterministic execution

Use the harness APIs when you need programmable cycle control:

- [Harness protocol](../reference/harness/protocol.md)
- `harness.load`
- `harness.cycle`
- `harness.set_input`
- `harness.get_output`
- `harness.run_until`

## Related

- [Agent API overview](../reference/agent-api/overview.md)
- [Agent API v1](../reference/agent-api/v1.md)
- [Compile, Validate, Reload](../operate/compile-validate-reload.md)
