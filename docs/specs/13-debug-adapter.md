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

### Run Control

#### Continue

- `ContinueRequest` resumes all threads.
- Any pending pause request is cleared.
- A `StoppedEvent` is emitted only if a breakpoint, step, or pause condition is hit after resuming.

#### Pause

- `PauseRequest` is honored only if execution is currently running.
- The adapter **must** respond to the request before emitting `StoppedEvent` with reason `pause`.
- If already paused, the adapter returns success and does not emit another pause event.

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

Current attach limitation: `setVariable` / `setExpression` are not supported in attach mode
(read-only variables).

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

### Stack Trace and Navigation

1) `StackTraceRequest` returns stack frames for the current thread.
2) The **top frame** location is the current statement location.
3) For multi-file projects, when execution enters a function in another file, the top frame’s
   `source.path` must reflect that file, and the editor should navigate there.

### Variables / Evaluate

- `VariablesRequest` and `ScopesRequest` return locals, globals, retain, and instance scopes.
- `EvaluateRequest` in `hover` or `watch` context must not have side effects. Calls are rejected.
- `setVariable` and `setExpression` are allowed only when paused.

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
  (IEC 61131-3 Ed.3, §6.5.2.3; DEV-023)
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
