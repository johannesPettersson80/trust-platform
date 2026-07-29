# Debug Adapter

Status: Draft

### Scope

This specification defines the expected behavior of the Structured Text (ST) debug adapter and
runtime debug hooks for VS Code using the Debug Adapter Protocol (DAP). It covers breakpoints,
run control, stepping, source mapping, and multi-file navigation.

This document is implementation-agnostic but aligns with the DAP definitions in
`Debug Adapter Protocol specification` (see References).

### References (Normative)

- DAP base and request/response/event shapes: `Debug Adapter Protocol specification`
  - `Request`, `Response`, `Event`
  - `InitializeRequest`, `InitializedEvent`
  - `LaunchRequest`
  - `AttachRequest`
  - `SetBreakpointsRequest`, `Breakpoint`, `BreakpointLocationsRequest`
  - `ContinueRequest`, `PauseRequest`, `NextRequest`, `StepInRequest`, `StepOutRequest`
  - `StoppedEvent`
  - `StackTraceRequest`, `ScopesRequest`, `VariablesRequest`, `EvaluateRequest`
  - `DisconnectRequest`, `TerminateRequest`

### Terms

- **Adapter**: `trust-debug` process handling DAP requests.
- **Runtime**: `trust-runtime` process executing ST code.
- **Statement**: A single executable ST statement with a source location.
- **Location**: `(file_id, start_offset, end_offset)` in source text.
- **Task**: IEC task representing a cyclic execution unit.

### Source Mapping

1) Every executable statement **must** be assigned a location at the **first non-trivia token** in
   its syntax node. The location span covers the full statement text range.
2) Each source file loaded in a debug session has a unique `file_id` and is registered in the
   adapter with its path and full text.
3) The adapter converts runtime locations to `(line, column)` for DAP using 1-based coordinates
   when `linesStartAt1` / `columnsStartAt1` are true.
4) A breakpoint source path relative to the selected project resolves against
   that project root and binds to the already registered source/file ID. It
   must not resolve outside the project root.

### Breakpoints

#### SetBreakpoints

- `SetBreakpointsRequest` replaces all breakpoints for the given source.
- Passing an empty list clears all breakpoints for that source in both adapter and runtime.
- Breakpoints are **statement-based** and resolved to the first statement whose location is at or
  after the requested `(line, column)`.
- Column snapping:
  - If the client omits a column, the adapter snaps to the first non-whitespace column on that line.
  - If a column is provided but points before the first non-whitespace column, the adapter snaps
    forward to that first column.

#### Breakpoint Locations

- `BreakpointLocationsRequest` returns the set of valid statement start positions in the requested
  range.

#### Cyclic Tasks

- In cyclic tasks, a breakpoint in a statement that executes every scan **will stop every scan**
  until the breakpoint is cleared or a hit condition/condition filters it.
- Users should use hit counts or conditional breakpoints for one-shot behavior.
- A breakpoint installed after launch, including while cyclic execution is
  already running, applies at the next matching statement boundary. It does not
  retroactively stop a statement or scan that completed before installation.

### Run Control

#### Continue

- `ContinueRequest` resumes all threads.
- Any pending pause request is cleared.
- A `StoppedEvent` is emitted only if a breakpoint, step, or pause condition is hit after resuming.

#### Pause

- `PauseRequest` is honored only if execution is currently running.
- The adapter **must** respond to the request before emitting `StoppedEvent` with reason `pause`.
- If already paused, the adapter returns success and does not emit another pause event.
- A statement-boundary pause suspends the current runtime cycle. Operator dwell
  time while stopped is excluded from the cycle watchdog and output-commit
  deadlines; resume continues with the remaining active-execution budget.
- No new scan or output commit starts solely because the debugger is waiting.
  Pause does not apply safe state. An independent stop, fault, or active-execution
  watchdog breach retains its normal runtime semantics. See
  `docs/specs/11-runtime-engine.md#debug-and-resource-pause-interaction`.

#### Stop on Entry

- `LaunchRequest` with `stopOnEntry=true` results in a pause as soon as the first statement boundary
  is reached.

#### Attach / Detach (Production)

- `AttachRequest` connects to a **running** runtime instance.
- Attach must **not** restart or reload the runtime.
- Attach must observe the existing execution state (running/paused/faulted).
- If attach occurs while the runtime is paused, the adapter should immediately emit a
  `StoppedEvent` reflecting the paused state.
- `DisconnectRequest` / `TerminateRequest` must not alter runtime execution unless the user
  explicitly requests termination.

Attach arguments (adapter-specific):
- `endpoint` (required): control endpoint, e.g. `unix:///tmp/trust-runtime.sock` or `tcp://127.0.0.1:9000`
- `authToken` (optional): control auth token (same value used by `trust-runtime ctl`)

Attach requires `runtime.control.debug_enabled=true`. If disabled, the adapter must report an
error and remain disconnected.

The runtime control role matrix in
`docs/specs/11-runtime-engine.md#runtime-control-authorization` governs attach
operations. Debug reads require Viewer; pause/resume require Operator; step,
breakpoint mutation, evaluation, write, force, and release require Engineer.
Viewer and Operator mutation attempts must fail before queued writes or force
state changes. Only Admin may enable the debug surface or change control mode;
Engineer authority permits use of an already enabled surface, not activation of
that surface. Runtime role denials carry the stable control error code
`insufficient_role`; the adapter must preserve that authorization category when
reporting the failed request.

Current attach limitation: arbitrary `setVariable` / `setExpression` variable writes are not
supported in attach mode. Live Values I/O operations are the supported write path while attached:
`stIoWrite`, `stIoForce`, and `stIoRelease` forward to the runtime control endpoint and must surface
runtime authorization/capability errors honestly.

For a managed local debug session, Live Values writes and forces to a declared
`REAL` or `LREAL` address parse the submitted text as that IEC floating-point
type. The adapter accepts only finite semantic values. It rejects `NaN`,
positive infinity, negative infinity, values that overflow the declared type,
and integer encodings that attempt to supply non-finite IEEE bit patterns. A
rejection returns a failed DAP response before the value is queued, forced, or
written to the process image; the previous value and force state remain
unchanged. Attach-mode validation remains governed by the runtime control
endpoint and is not covered by this managed-session rule.

#### Live Values Mutation Lifetime

- A successful write is queued for the next scan boundary and is consumed
  once.
- A successful force remains active across scans, pause/resume, and a
  non-terminating detach until an authorized release or a runtime clearing
  boundary.
- Release removes the force but does not write a replacement value.
- Deliberate stop, fault handling, and warm or cold restart clear queued writes
  and active forces before safe-state handling or restarted execution.
- Authentication or authorization changes apply to later commands and do not
  silently clear an existing force. The force remains visible to authorized
  clients until release or a clearing boundary.

These are truST debugger/runtime lifecycle rules outside the IEC language
execution model, not IEC deviations.

#### Runtime control I/O surface

- An I/O snapshot reports the force marks captured with that snapshot's scan.
  A force accepted after scan `n` cannot retroactively decorate scan `n`; it
  appears only when a later snapshot records it.
- Each `%I`, `%Q`, and `%M` process-image area is bounded to byte offsets
  `0..=16_777_215`. Control write and force requests at or above
  `16_777_216` reject before allocation or mutation.
- Pause, debug-state read, warm-restart request, and queued I/O write remain
  independently routed operations. A composite routing behavior lock does not
  replace their deeper state-machine, mutation-lifetime, or restart proof.

### Stepping Semantics

The following are required semantics for DAP step requests:

1) **Step In** (`stepIn`):
   - Resume execution and stop at the **next executed statement**.
   - If the next statement is a call, stepping **enters** the callee and stops at the first statement
     inside the called function/method.

2) **Step Over** (`next`):
   - Resume execution and stop at the next statement in the **current frame**.
   - Calls are executed without entering the callee.

3) **Step Out** (`stepOut`):
   - Resume execution and stop at the next statement **after returning** to the caller.

Stepping is statement-granular, not instruction-granular.

### Stopped Events

- `StoppedEvent.reason` **must** match the cause:
  - `breakpoint` for active breakpoints,
  - `step` for stepping commands,
  - `pause` for explicit pause requests,
  - `entry` for stop-on-entry.

### Runtime Debug-Control Execution

Statement locations use the first non-trivia token. Breakpoint resolution
prefers the innermost containing statement and otherwise selects the next
statement at or after the requested position. Runtime resolution returns that
statement start, and a breakpoint hit pauses before the statement, emits one
`breakpoint` stop carrying the current breakpoint generation, and resumes only
after an explicit continue.

After continue, an unchanged breakpoint remains eligible when its statement is
executed in a later scan, but one breakpoint identity emits at most one stop per
scan even when several VM debug-map entries overlap its source range. A
breakpoint installed after launch or while cyclic execution is running becomes
eligible at the next matching statement boundary; execution completed before
installation remains unstopped.

Step In stops at the next executed statement. Step Over ignores deeper frames
and stops at the next statement at the starting frame depth. Step Out ignores
the current and deeper frames and stops after returning to the caller. Each
step resumes from a paused state and pauses again at exactly that boundary.

The runtime updates the active frame location at each statement boundary.
Watch-change state is recomputed for each stop from the current storage, and
resuming a paused multi-task scan preserves the configured task execution
order; a pause must not restart or reorder the remaining tasks.

A conditional breakpoint whose expression is false does not pause; a true
condition follows ordinary breakpoint stop behavior. A hit-count breakpoint
stops exactly when its accepted threshold is reached. A logpoint evaluates its
registered fragments, appends one formatted output record, and never pauses
execution solely because the logpoint was reached.

Debug-control event delivery is loss-resistant at the local process boundary.
If a registered runtime-event receiver has been dropped, the event is retained
in the in-memory runtime-event buffer. If a registered log receiver has been
dropped, the formatted logpoint record is retained in the in-memory debug log
buffer. Receiver failure must not discard the first fault or log record.

### Stack Trace and Navigation

1) `StackTraceRequest` returns the adapter's current captured stack projection
   only after confirming that the requested thread ID is present in the current
   `ThreadsResponse` projection. An absent ID fails with an unknown-thread error
   and no stack body; it never substitutes another thread or a synthetic Main
   frame. The current adapter does not claim distinct per-task frame stacks for
   multiple simultaneously projected task threads.
2) The **top frame** location is the current statement location.
3) For multi-file projects, when execution enters a function in another file, the top frame’s
   `source.path` must reflect that file, and the editor should navigate there.

### Variables / Evaluate

- `VariablesRequest` and `ScopesRequest` return locals, globals, retain, and instance scopes.
- `EvaluateRequest` in `hover` or `watch` context must not have side effects.
  Explicitly whitelisted pure standard functions, conversions, and
  split-date/time helpers are accepted; user-defined, unknown, and impure calls
  are rejected.
- `setVariable` and `setExpression` are allowed only when paused. Attached runtimes use the
  side-effecting Live Values I/O custom requests (`stIoWrite`, `stIoForce`, `stIoRelease`) instead
  of `setExpression` for I/O writes and forcing.

### Variable Visibility

Debugger scopes and variable visibility follow IEC variable sections and access
rules.

**Rules**:
- Local scopes include variables declared in the active POU’s `VAR`,
  `VAR_TEMP`, `VAR_INPUT`, `VAR_OUTPUT`, and `VAR_IN_OUT` sections.
  (IEC 61131-3 Ed.3, Tables 13–14; §6.5.1–6.5.2)
- Global scopes include `VAR_GLOBAL`, `VAR_EXTERNAL`, `VAR_ACCESS`, and
  `VAR_CONFIG` symbols resolved to their declared names, not raw access paths.
  (IEC 61131-3 Ed.3, §6.5.2.2, Tables 13–16)
- Instance scopes expose the variables declared in the instance’s FB/CLASS
  `VAR` sections, respecting access specifiers. (IEC 61131-3 Ed.3, §6.5.2.3)
- Access specifiers are not enforced for debugger inspection yet;
  `PRIVATE`/`PROTECTED`/`INTERNAL` members may be visible.
  Debugger inspection is a truST tooling surface and does not change
  source-level IEC access enforcement.
- Directly represented variables (`AT %I/%Q/%M`) are presented by symbolic
  name; the address may be shown as metadata, not as a separate scope.
  (IEC 61131-3 Ed.3, §6.5.5, Table 16)

### Safe Points

Debugger safe points align with Structured Text statement boundaries. The
runtime may pause only before executing a statement and never within
expression evaluation. (IEC 61131-3 Ed.3, §7.3.3.1, Table 72)

### Reload / Hot Reload

- `stReload` replaces runtime sources and revalidates breakpoints.
- If the session was paused before reload, it remains paused after reload.
- Source compilation and runtime reload are fail-closed. A compile,
  bytecode-validation, resource-validation, or retained-state preparation
  error returns a failed response and preserves the previously running program
  and its live state. The adapter must not emit a successful reload or replace
  its current metadata on that path.
- The runtime transaction and state-preservation boundary is normative in
  `docs/specs/11-runtime-engine.md#671-online-change-transaction`.

#### Reload Trigger Policy (Required)

To avoid breaking step-in/step-out and multi-file navigation, reloads must follow these rules:

1) **No reload on editor focus**:
   - Opening a file or changing the active editor must **not** trigger `stReload`.
   - This includes stepping into a function in another file.

2) **Allowed reload triggers**:
   - Explicit user action (e.g., command: “Reload Runtime”).
   - Optional: save events for ST files (if enabled), but **never** on focus change.

3) **Program path correctness**:
   - The `program` argument of `stReload` must always reference the **configuration entry**
     file (the same one used in `LaunchRequest`), not the currently focused file.

4) **Reload must not override step stops**:
   - If a `stepIn/stepOver/stepOut` stop just occurred, reload must **not** emit a pause stop
     that replaces the step stop or changes the top frame.
   - If reload happens while paused, it must preserve the existing top frame until the user resumes.

### DAP Transport and Session Projection

#### DAP JSON wire schema

The adapter's serialized protocol types form a normative wire contract. They
SHALL obey all of the following rules:

- Every request, response, and event envelope carries `seq` plus a lowercase
  `type` discriminator. Requests require `command`; responses require
  `request_seq`, `success`, and `command`; events require `event`.
  `arguments`, response `message`/`body`, and event `body` are omitted when
  absent rather than emitted as JSON `null`.
- The canonical response correlation field is `request_seq`. For compatibility
  with previously emitted internal payloads, deserialization MAY also accept
  `requestSeq`; serialization MUST NOT emit that alias.
- DAP object fields use their protocol lower-camel-case spellings, including
  `threadId`, `frameId`, `variablesReference`, `sourceReference`,
  `allThreadsStopped`, `allThreadsContinued`, `sourceModified`, `endLine`, and
  `endColumn`. The DAP field named `type` is emitted as `type`, never as the
  Rust raw identifier spelling. Unsupported discriminator or enum spellings
  are rejected.
- Optional fields are omitted when absent. A numeric or Boolean zero value is
  still an explicit value and MUST NOT be discarded. Required identifiers,
  names, values, source objects, and collection fields fail deserialization
  when missing or of the wrong JSON shape.
- Initialize capabilities are flattened into the initialize response body.
  Unknown launch and attach arguments are preserved as a flat string-keyed JSON
  object so that the adapter can inspect product-specific arguments without
  changing the DAP envelope.
- The custom I/O snapshot body requires `inputs`, `outputs`, and `memory`.
  `scan` is optional. Each entry requires `address` and textual `value`;
  `forced` defaults to `false` when absent and is always serialized so clients
  can distinguish an unforced sample from missing state.
- The custom variable snapshot body requires `globals` and `retain`.
  `locals` and `instances` default to empty and are omitted when empty;
  `frameId` and `paused` are optional. Instance entries require a stable
  numeric `id`, a display `name`, and their `vars`.
- Custom I/O write/force requests require `address` and textual `value`;
  release requires `address`. A custom variable mutation requires lowercase
  `scope`, `name`, and optional lowercase `action`; `value`, `instanceId`, and
  `frameId` remain optional so release and scoped instance operations have one
  unambiguous shape.
- Source, thread, stack, scope, variable, evaluate, and mutation payloads keep
  the DAP-required fields explicit. Optional source locations, child counts,
  evaluate names, type names, and frame selectors are omitted when absent.
- A source-breakpoint request requires `line`; its condition, hit condition,
  log message, and column are optional. A breakpoint result always carries
  `verified`. The verified and unverified constructors preserve the requested
  source position while differing only in verification state and optional
  failure message.
- `stReload` uses the optional lower-camel-case fields `program`,
  `runtimeIncludeGlobs`, `runtimeExcludeGlobs`, `runtimeIgnorePragmas`, and
  `runtimeRoot`. `setBreakpoints` requires `source`; `breakpointLocations`
  requires `source` and `line`; their optional ranges and legacy line lists are
  omitted when absent.

These wire rules are independent of handler semantics. A handler test that
constructs a Rust value without verifying its JSON projection does not prove
this contract.

The debug adapter SHALL preserve the following protocol and session contracts:

- Standard input/output transport uses DAP `Content-Length` framing, preserves
  a request's sequence in the response `request_seq` field, and round-trips one
  complete request without consuming bytes from a following frame.
- A successful `initialize` request emits the initialized event exactly once.
  Launching before initialization SHALL NOT manufacture that event.
- Continue followed immediately by pause produces an ordered pause stop.
  Pausing without an active storage frame targets the global runtime, and the
  DAP thread list projects configured runtime tasks without changing their
  identity.
- The managed debug runner uses the smallest positive configured task interval
  as its cycle-pacing deadline. It executes at least one cycle while active,
  does not hot-loop between deadlines, and remains responsive to stop requests.
- Stack, scope, and I/O-state projection SHALL use a captured runtime snapshot
  and SHALL NOT wait on the live runtime mutex after that snapshot is
  available. When no storage frame or source location exists, a synthetic Main
  frame remains inspectable for a known thread and its scopes fail gracefully.
  A thread ID absent from the current DAP thread projection is rejected with no
  stack body.
- Evaluate accepts side-effect-free value and type expressions, including
  names resolved through active `USING` namespaces. Impure or unknown calls are
  rejected. Primitive values use stable user-facing debugger formatting.
- Source discovery expands ordinary and nested brace globs. Hit-condition
  parsing accepts the documented comparison operators, while logpoint
  templates accept valid interpolation and reject malformed messages.
- Reload with no breakpoint requests clears prior breakpoints. Reload with
  retained requests re-resolves them against the new source before reporting
  them as valid.

These rules describe the DAP/session product boundary. Internal helper success,
source-text inspection, or a fabricated fallback for an unknown thread does
not establish this contract.

### Managed Runtime Debug Mutation and Active Reload

Managed-local and attached debugging SHALL preserve these mutation and reload
boundaries:

- Configured `REAL` and `TIME` values are admitted through typed I/O writes.
  Attach-mode `setExpression` and custom ST I/O force/release requests forward
  the same declared address, value, and force action to the remote runtime.
- Direct instance-field force works in both live and paused sessions. Local
  output and memory force/release use the declared process-image target;
  one-shot writes to output I/O are rejected instead of being reported as
  successful.
- Launch fails closed if its control endpoint cannot be bound. Project ADS
  bindings are validated before reload changes the active program.
- While the runner is active, reload SHALL NOT emit a pre-scan I/O snapshot.
  The first reported snapshot after reload is produced after a coherent scan
  of the replacement program.

Mutation forwarding, bind/configuration rejection, and post-reload snapshot
ordering are independently observable. Success in one partition SHALL NOT be
used as evidence for another.

### Required Improvements (Architecture + Behavior)

The following items are **required** to align the implementation with this specification and to
avoid the observed instability in multi-file debugging sessions. These requirements are derived
from the DAP references above and the current runtime/adapter architecture.

#### 1) Stop Reason Integrity

- The adapter **must not** emit `StoppedEvent{reason="breakpoint"}` if there are no active
  breakpoints at stop time.
- Pending stop reasons must be **cleared** on `continue`, `step*`, or breakpoint removal.

#### 2) Breakpoint Generation / Staleness Guard

- Breakpoint sets must be versioned. Each `SetBreakpointsRequest` increments a generation number
  and runtime stops must only be honored if they match the **current** generation.
- Clearing breakpoints (`SetBreakpointsRequest` with an empty list) must immediately invalidate
  any pending breakpoint stops.

#### 3) Reload Semantics

- `stReload` must preserve paused/running state explicitly:
  - If the session was running, it stays running after reload.
  - If the session was paused, it stays paused after reload with `StoppedEvent{reason="pause"}`.
- The state machine must include an explicit **Reloaded** transition to avoid ambiguity.

#### 4) Per‑Frame Source Mapping

- `StackTraceRequest` must report each frame with its **own** source location (file/line/column),
  not the top-of-stack location for all frames.
- When a function in another file is entered, the top frame must point to that file; caller frames
  must continue to show their original source locations.

#### 5) Pause/Continue Idempotency

- `PauseRequest` while already paused must be a no-op (no additional pause events).
- `ContinueRequest` must clear any adapter-side pause expectation and runtime pending pause.

#### 6) Stop-on-Entry Reason

- `stopOnEntry` must emit `StoppedEvent{reason="entry"}` (not `pause`) per DAP semantics.
- This reason must be distinct in logs and internal state to avoid confusion with manual pause.

#### 7) DAP Event Ordering

- For requests that cause a stop (pause/step), the adapter must **send the response first** and
  emit `StoppedEvent` **after** the response, matching DAP requirements.

#### 8) Multi‑Task Thread Model

- If multiple IEC tasks are configured, each must map to a distinct DAP thread ID.
- `step*` and `pause` must apply to the thread specified by the request.

#### 9) Cyclic Task Breakpoint Safety

- In cyclic tasks, a breakpoint hit must not starve `continue`:
  - If the breakpoint is cleared, the runtime must resume without re-triggering the old stop.
  - If the breakpoint remains, the adapter should support hit conditions to avoid infinite stops.
