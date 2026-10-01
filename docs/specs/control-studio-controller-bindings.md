# Control Studio controller binding contract

This specification defines the runtime binding boundary used by Control Studio
parameter and transfer transactions. It complements the controller workflow
specification and the `Control.PID` algorithm specification.

## Nested scalar leaves

Bindings may name a writable scalar leaf through a qualified path rooted at a
compiled program instance. The path may cross nested function-block instances
and STRUCT values, for example:

```text
Main.Controller.ParametersValue.ProportionalGain
Main.Controller.StateValue.NominalOutput
```

The runtime shall resolve the path through the same `VariableStorage` and
`ValueReference` ownership used by PLC execution. Reads shall observe the
current leaf at status time. Applies shall stage all requested leaves and
commit them atomically at the declared owning task boundary. The post-boundary
observation shall read those same leaves after Structured Text execution.

Parameter and transfer bindings shall use the same path semantics, ownership
checks, mutability checks, IEC scalar checks, and I/O reference identity for
top-level and nested leaves. A nested path is not a second controller or a
transport-specific write path. Each declared leaf is written once by the
owning transaction, and ordinary task execution remains the source of any
subsequent state/output observation.

The binding boundary shall reject missing paths, non-scalar leaves, immutable,
external, `IN_OUT`, or addressed leaves where the existing binding contract
requires an unaddressed value, and paths whose program root has no unique task
owner. Existing transaction revision, manual-mode, forcing, rollback, and
post-ST confirmation rules remain in force.

Isolated actual-ST verification and search use these same scalar paths for
candidate parameters, initial states, events and observations. A parameter/state
binding accepted for live application must not be rejected merely because the
verification worker uses a different symbol traversal. I/O aliases retain their
existing coercion and ownership checks.

The isolated-run request and its checkpoint stream support the same maximum of
64 observed scalar channels, including controller parameters and retained states,
so a PID group's transfer check, with every member's retained states and a
decoupler's entries (Specification 35 §17.15), fits; the budget of 250000
channel samples per run still bounds a long run.
An accepted request must not lose its results because the checkpoint reader
uses a lower channel limit. Larger, duplicate or malformed channel maps remain
rejected by the shared execution contract.
