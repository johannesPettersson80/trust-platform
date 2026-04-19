# Automate With CLI / CI / agents

Use this page when you want shell, CI, or agent-driven workflows.

## First Success Loop

Use a small shipped project first:

- `examples/memory_marker_counter`

Run:

```bash
trust-runtime build --project examples/memory_marker_counter --sources src
trust-runtime validate --project examples/memory_marker_counter
trust-runtime test --project examples/memory_marker_counter --output human
```

## First `agent serve` Request

Ask the runtime what the agent surface supports:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"agent.describe","params":{}}' \
  | trust-runtime agent serve --project ./examples/memory_marker_counter
```

Then inspect the project:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"workspace.project_info","params":{}}' \
  | trust-runtime agent serve --project ./examples/memory_marker_counter
```

## What Success Looks Like

- build produces bytecode without errors
- validate succeeds
- tests report a real result
- `agent.describe` and `workspace.project_info` return JSON-RPC responses

## If It Fails

- build/validate/test issues: go to [Build, Validate, Test](../operate/build-validate-test.md)
- method/contract questions: go to [Agent Quickstart](agent-quickstart.md) or
  [Agent API v1](../reference/agent-api/v1.md)
- deterministic execution needs: go to
  [Harness Protocol](../reference/harness/protocol.md)

## Next

- [Agent Quickstart](agent-quickstart.md)
- [Agent API overview](../reference/agent-api/overview.md)
- [Harness Protocol](../reference/harness/protocol.md)
- [CI/CD](../operate/ci-cd.md)
