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
- Attach-mode I/O and variable read/write/force/release parameter grammar,
  address and scalar admission, fail-closed snapshot behavior, queue ordering,
  target identity, and force projection follow
  [Runtime control I/O and variable mutation contract](11-runtime-engine.md#runtime-control-io-and-variable-mutation-contract).

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

#### Breakpoint evaluation, source resolution, and hook contract

A runtime breakpoint matches only the same file ID and a strictly overlapping
half-open source span; spans that merely touch at one endpoint do not match.
Breakpoints are inspected in registration order and the first eligible stopping
breakpoint owns the stop. The cycle-suppression identity is
`(file_id,start,end,generation)` and suppresses only an ordinary stop from that
exact identity. It does not suppress a distinct location, a later generation,
or logpoint output.

The hit counter increments with saturating arithmetic whenever location
identity matches and the exact stop has not been cycle-suppressed. Hit
conditions are applied to the incremented count before expression evaluation.
A missing evaluation context, evaluation failure, false Boolean, or non-Boolean
condition does not stop. It still retains the physical hit count. A true
Boolean condition proceeds to the ordinary stop.

A logpoint never returns a stop. It concatenates literal and expression
fragments in order. Boolean values use `TRUE`/`FALSE`; STRING/WSTRING and
CHAR/WCHAR use their unquoted text; arrays use `[<element-count>]`; structures
use `<type> {...}`; enums use `<type>::<variant>`; present and absent references
use `REF` and `NULL_REF`; instances and null use `Instance` and `NULL`.
Other values use their stable debug representation. An expression error is
rendered in place as `<error: ...>` without dropping surrounding literal text.
A live log receiver gets one owned record and no duplicate buffer entry; an
absent or closed receiver retains that same record in the local buffer.

Runtime source coordinates are zero-based byte coordinates. Line-start
construction always contains byte offset zero and adds the byte following every
LF, including a final LF; CR remains part of the preceding line. Offset
projection clamps an offset beyond the source to source end. Location
projection uses the location start.

Breakpoint resolution first rejects an unavailable line, clamps the requested
column to that line's byte end, and ignores statements from other file IDs. On
the requested line it selects the statement with the greatest start column not
after the request; if none exists, it selects the smallest start column after
the request. If no statement starts on that line, it selects the narrowest
statement containing the clamped offset, using inclusive runtime location
ends for compatibility. Otherwise it selects the globally earliest later
statement. An empty candidate set returns no location.

Every debug hook call records the supplied location and call depth before
making a stop decision. Context-bearing calls additionally discard frame
locations for frames no longer present and bind the current frame to the
statement. A pause, step, or breakpoint stop snapshots the exact storage and
runtime time at that boundary and recomputes watches before publishing the
stop. Stop publication always updates `last_stop` and the ordered local stop
buffer; a configured live receiver also gets one owned copy, while receiver
closure cannot discard the buffered stop. A non-target runtime thread never
consumes or blocks on another thread's pause or step.

`NoopDebugHook` has no runtime side effect, and the trait's default
context-bearing callback delegates exactly once to the context-free callback.
Debug tracing is disabled unless `ST_DEBUG_TRACE` is present, including when
its value is empty. When enabled, each record is prefixed
`[trust-runtime][debug]`. `ST_DEBUG_TRACE_LOG` selects the append-only log file
and takes precedence over the compatibility variable `ST_DEBUG_DAP_LOG`.
Opening, locking, writing, or flushing the optional trace file may report a
diagnostic but must never change debugger control flow.

#### Runtime debug state, mutation, and projection invariants

A newly created runtime debug control starts running with current thread `1`,
no target thread, breakpoint, stop, snapshot, watch, buffered event, queued
mutation, or active force. An explicit pause transitions a running control to
paused, records one pending `pause` reason, clears step state and the captured
snapshot, and optionally binds the requested thread. Pausing an already paused
control is ignored and does not replace the original pending reason or target.
Stop-on-entry follows the same one-pending-stop rule but records `entry` and is
not thread-targeted.

Continue always returns to running mode, removes every pending stop and step,
clears the target thread and captured snapshot, and wakes a waiting target.
Step In, Step Over, and Step Out replace any earlier step request, clear a
pending stop and captured snapshot, resume execution, and target the explicit
thread or otherwise the current thread. Step In stops at the next statement
after it is armed. Step Over uses that thread's last observed depth and stops
when execution returns to the same or a shallower depth. Step Out uses the
saturating predecessor of that depth and therefore remains defined at depth
zero. A non-target thread continues without consuming the target's pending
stop or step.

Replacing breakpoints for one file leaves other files unchanged, increments
that file's generation with saturation, resets every replacement breakpoint's
hit count/generation ownership to the new set, and preserves request order.
An empty replacement removes that file's breakpoints while still advancing
its generation. Clearing all breakpoints also clears the generation table and
the per-cycle duplicate-stop guard.

State-query snapshots are owned copies. Reading breakpoints, frame locations,
the latest stop, the runtime snapshot, or the active forces cannot allow a
caller to mutate debug-control state. Draining logs, stops, runtime events, or
queued mutations returns the complete current sequence in arrival order and
atomically leaves that sequence empty. Mutating a captured runtime snapshot is
possible only when one exists; absence returns `None` and does not invoke the
callback.

Runtime-event streaming is single-subscriber. With a live sender, each event
is delivered once and is not also buffered. When no sender is configured, the
event is buffered. If delivery discovers a closed sender, that sender is
removed and the same event is buffered before later events. Clearing the
sender makes subsequent events buffer. I/O snapshots are best-effort
telemetry: a configured live sender receives a coherent owned snapshot, while
no sender or a closed receiver produces no synthetic snapshot or runtime
fault. Registering a new sender replaces the previous sender.

Watch expressions retain their most recently evaluated value. Refresh marks a
sticky change flag when any watch moves between a value, a different value,
or evaluation failure. Reading that flag returns the accumulated edge and
clears it. Clearing watches removes both expressions and the pending edge.
Refreshing directly from storage replaces the stored snapshot with an owned
storage copy and exact supplied runtime time; later source-storage mutation
cannot rewrite that captured state.

Queued I/O writes preserve every request in arrival order. Pending global,
retained, instance, and local variable writes use their complete target as
identity: a later value for the same target replaces it in place, while
different target kinds, instance/frame IDs, or names remain distinct. Pending
lvalue writes preserve every request in arrival order. Variable and I/O forces
also replace the same exact target in place; release is exact and idempotent.
The runtime termination boundary clears all queued I/O, variable, and lvalue
writes plus all variable and I/O forces atomically.

DAP variable handles are positive session-local identifiers. Allocation
preserves the complete handle kind, does not alias two live handles, and clear
invalidates every old handle before restarting allocation at `1`. Scalar and
null values have `variablesReference = 0`; structs, arrays, instances,
non-null references, and scope roots allocate expandable handles. Primitive
type names use canonical IEC spelling, while a struct or enum uses its
declared public type name. A caller-supplied display or declared type overrides
derived presentation without changing expandability.

Entry and array projections preserve input/field/element order and stable
evaluate names. Instance-list entries use `<type>#<normalized-id>` as their
display name, the public type name as both value and type, and no evaluate
name. An I/O entry uses its declared symbol name when present and otherwise a
canonical uppercase direct address; its value is the runtime debug value,
`error: <detail>`, or `unresolved`. I/O scope availability is true only when a
captured snapshot contains at least one input, output, or memory entry.

`DebugSnapshot` expression evaluation and lvalue read/write use only the
captured storage. A supplied frame ID must exist and is active only for the
duration of that operation; an unknown frame returns `InvalidFrame` without
falling back to the current/global context. A snapshot write changes only the
owned snapshot until an independently authorized runtime mutation is queued.

`SourceLocation::new` preserves the exact file and byte offsets.
`DebugBreakpoint::new` creates an unconditional, zero-hit, zero-generation
breakpoint. Hit conditions use exact unsigned arithmetic: equality matches
only the target, at-least includes the target, and greater-than excludes it,
including at `0` and `u64::MAX`.

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

### DAP Session Lifecycle and Request Ordering

The adapter SHALL model one debugger connection as the following monotonic
session lifecycle:

`New -> Initialized -> StartPending -> Configured -> Active -> Terminated`.

`StartPending` records exactly one `launch` or `attach` request, including its
request sequence and arguments. `Active` is either a managed local launch or a
remote attachment; the two modes are mutually exclusive for the lifetime of
the connection.

The lifecycle obeys these rules:

- `initialize` is the first accepted session request. A successful call records
  the client's line and column coordinate conventions, returns the declared
  capabilities, and emits exactly one `initialized` event. A second
  `initialize` fails, emits no additional `initialized` event, and does not
  reset coordinates, pending start state, breakpoints, watches, runner state,
  or attachment state.
- `launch`, `attach`, and `configurationDone` before successful initialization
  fail with a response correlated to the rejected request. A malformed
  `initialize`, `launch`, or `attach` argument object fails closed; it is not
  replaced with default arguments and does not advance the lifecycle.
- Before configuration completes, the first valid `launch` or `attach` becomes
  the sole pending start. It emits no start response until the start is either
  executed or rejected. A second start request, whether it is the same or the
  other start kind, fails immediately and cannot replace or mutate the first
  pending request.
- Configuration requests, including breakpoint setup, may occur while a start
  is pending and cannot execute or discard that start. `configurationDone`
  first returns its own successful response, then executes the exact pending
  request and returns that request's response. Each response preserves the
  corresponding request sequence. Repeating `configurationDone` after the
  configured transition is idempotent and cannot repeat a launch, attach,
  runner start, pause-on-entry action, or control-server bind.
- For compatibility with clients that omit `configurationDone`, a pending
  start MAY be executed after the documented timeout or immediately before the
  first non-configuration request. This fallback executes the same recorded
  request exactly once, reports that compatibility path through an internal
  diagnostic event, and preserves the pending start response's request
  sequence. It never applies to a configuration request.
- A successful managed launch schedules `stopOnEntry` and runner startup only
  after its launch response has been emitted. Each scheduled action is consumed
  at most once. A failed reload or control-server bind emits a failed launch
  response and schedules neither action.
- A successful attach never reloads the local program, starts the managed
  runner, or binds a local control server. Attach failure leaves the adapter
  disconnected and does not create a partially active session.
- Once a launch is active, attach is rejected; once an attachment is active,
  launch and a second attach are rejected. Start-mode rejection has no effect
  on the active session.
- An unsupported request in a live session returns one failed response,
  correlated to the request, and changes no lifecycle state. Non-request DAP
  envelopes are ignored and cannot produce a response or event.
- `disconnect` and `terminate` each return their response before one
  `terminated` event, preserve the optional `restart` value in that event, and
  end the adapter connection. Repeated teardown processing cannot emit another
  termination event. Disconnect from an attached runtime detaches only; it
  does not stop, restart, reload, resume, or otherwise mutate that runtime
  unless `terminateDebuggee=true` is both explicitly requested and supported.

Lifecycle tests SHALL observe request correlation, response/event ordering,
state preservation, and side-effect counts. Inspecting only a final enum
variant does not prove that a displaced pending request or duplicated action
was avoided.

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
