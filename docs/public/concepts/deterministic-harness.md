# Deterministic Harness

The harness exists so scripts, tests, docs, and agents can drive cycle
execution deterministically instead of treating the runtime as a black box.

| Term | Meaning |
| --- | --- |
| `trust-harness` | Deterministic executor for cycle-by-cycle checks. |
| `NDJSON` | Newline-delimited JSON requests and responses. |
| Virtual time | Test-controlled time advanced by the harness instead of wall-clock time. |

## What it is

`trust-harness` is the smallest executable surface that can:

- load ST sources
- step cycles
- set inputs
- read outputs
- advance virtual time
- run until a condition matches

without starting the full runtime lifecycle.

## Why it exists

The harness is the fast path for:

- docs that need executable behavior
- CI loops
- deterministic example verification
- agent repair loops
- future local-model or website sandboxes

## Two Ways To Use It

1. direct CLI / NDJSON protocol via `trust-harness`
2. the same core operations through the agent contract methods:
   - `harness.load`
   - `harness.reload`
   - `harness.cycle`
   - `harness.set_input`
   - `harness.get_output`
   - `harness.advance_time`
   - `harness.run_until`

## Scope

The harness is ideal when you want deterministic cycle control and typed I/O
inspection. Use the full runtime when you need:

- web UI
- runtime-cloud
- runtime control endpoints
- browser IDE or HMI pages

## Why This Is A Core Platform Surface

The harness is not just a test utility. It is the smallest trustworthy
execution surface in the stack. That makes it valuable for:

- documentation that should show executable behavior instead of pseudocode
- CI jobs that need stable, machine-readable results
- agent loops that must write, validate, and repair deterministically
- future local-model or hosted sandbox scenarios where a full runtime would be too heavy

## Mental Model

Use the harness when you want to answer:

- "What happens in the next cycle if I flip this input?"
- "Did this repair actually fix the behavior?"
- "Can I prove this example still behaves like the docs say?"

Use the full runtime when you want to answer:

- "What does this look like in the runtime UI or HMI?"
- "How does this runtime behave under runtime control-plane management?"
- "How does this node participate in a larger distributed system?"

## Related

- [Harness Protocol](../reference/harness/protocol.md)
- [Compile, Validate, Reload](../operate/compile-validate-reload.md)
