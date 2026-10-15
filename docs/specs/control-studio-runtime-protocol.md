# Control Studio runtime protocol

This annex is part of Specification 35, VS Code Control Studio. It states the runtime operations that Control Studio uses: their requests, responses, checks, limits and recovery. Part A states the rules that these operations implement. This annex refers to those rules by number. A rule of Part A is stated here as a MUST even where the runtime does not yet meet it. The words MUST, MUST NOT, SHOULD and MAY are normative.

In this annex, *Studio* means Control Studio, the client in the VS Code extension. *Runtime* means the truST runtime that Studio talks to through the shared connection (A2.1). *Target* means the runtime that a study is bound to (A2.2).

## E0. Scope and common rules

### E0.1 Envelope, operations and access

Studio sends one JSON object for each request. It has a numeric `id`, a `type` that names the operation, and an optional `params` object. It may also send `auth`, and `request_id` or `correlation_id`. The runtime answers with the same `id`. A successful answer has `ok` set to true and a `result` object. A refused answer has `ok` set to false, an `error` text and an `error_code` (E0.4).

Each operation has one access role. A caller whose role is lower than the role of the operation is refused with `insufficient_role`. Viewer operations read state and observe signals. Engineer operations write a signal or a parameter, start or stop a live write, or start or cancel an isolated job. Recording operations observe and never write (E4.1).

| Operation | Access | Section |
|---|---|---|
| `status` | Viewer | E1.3 |
| `config.set` with the key `simulation.time_scale` | Engineer | E1.4 |
| `control.signals.inspect` | Viewer | E3 |
| `control.recording.start`, `control.recording.stop`, `control.recording.release`, `control.recording.status` | Viewer | E4 |
| `control.experiment.start`, `control.experiment.abort`, `control.experiment.finish` | Engineer | E5 |
| `control.experiment.status` | Viewer | E5 |
| `control.transfer.apply`, `control.transfer.cancel` | Engineer | E6 |
| `control.transfer.status` | Viewer | E6 |
| `control.parameters.apply`, `control.parameters.rollback`, `control.parameters.cancel` | Engineer | E7 |
| `control.parameters.status` | Viewer | E7 |
| `control.pid.apply`, `control.pid.rollback` | Engineer | E7.7 |
| `control.pid.status` | Viewer | E7.7 |
| `control.simulation.run.start`, `control.simulation.run.cancel`, `control.search.start`, `control.search.cancel` | Engineer | E8 |
| `control.simulation.run.status`, `control.search.status` | Viewer | E8 |

A Viewer operation MUST NOT change the state of a recording, an experiment, a transaction, a controller record or an isolated job. It MUST NOT clear a pending change, a rollback record or an identifier.

### E0.2 Identifiers and duplicate requests

Studio chooses each identifier before it sends the request, and it journals the identifier first (E11.2). The runtime checks an identifier only as the table shows.

| Operation | Identifier | Rules |
|---|---|---|
| Recording | `recording_id` | Required. 1 to 128 bytes. A start that reuses a held identifier is refused. |
| Experiment | `experiment_id` | Required. Not blank. The runtime keeps one experiment record. A new start replaces it. |
| Transfer | `transaction_id` | Required. 1 to 128 characters, for apply and cancel. A reused identifier is refused. |
| Parameter set | `transaction_id` | Required on apply. 1 to 128 characters. A reused or cancelled identifier is refused. |
| Isolated test run | `request_id` | Optional. 1 to 128 bytes, ASCII letters, digits and `-`. The runtime uses it as the `job_id`. When it is absent, the runtime generates the `job_id`. |
| Search | none | The runtime generates the `job_id`. Studio sends no identifier. |

1. A transfer or parameter identifier is held for the current session epoch. The runtime holds at most 1,024 such identifiers per epoch. A start after that limit is refused.
2. A transfer or parameter cancel for an identifier that the runtime does not hold fences it. The cancel answers as cancelled before dispatch. A later apply with that identifier is refused.
3. An experiment abort or finish for an unknown `experiment_id` is refused with `experiment_not_found`. It fences nothing.
4. A recording stop for an unknown `recording_id` is refused. A recording release for an unknown `recording_id` succeeds.
5. A repeated isolated test run with the same `request_id` and the same inputs returns the current status of that run. The same `request_id` with other inputs is refused.
6. A new session epoch releases every transfer and parameter identifier of the earlier epoch.
7. Studio MUST NOT repeat a start, apply, rollback or job start to resolve an unknown outcome. It reads the operation by its identifier (A8.5, E11.4).

### E0.3 Values and encodings

- `*_ms` fields are integer milliseconds of process time (A3.2). `runtime_time_ns` fields are integer nanoseconds of the runtime clock.
- A request number MUST be finite.
- A request integer MUST lie within ±(2^53 − 1). A signal integer outside that range is reported with quality `bad` (E3).
- A REAL value in a request MUST be exactly representable as single precision. A LREAL value MUST be exactly representable as double precision. The runtime refuses a REAL value that is not explicitly rounded to its runtime representation.
- A BOOL in a request is a JSON boolean. A BOOL signal value or transfer state value is reported as the number 0 or 1. A transfer mode value is a JSON boolean.
- Expected values are compared by value in their IEC type. JSON `2` and `2.0` denote the same REAL or LREAL value.
- A request with an unknown field, a missing required field or a value of the wrong JSON type is refused before any effect. The refusal uses the refusal code of the operation's family and carries the decoder's message.

### E0.4 Refusals

A refused request has no effect. The answer has `ok` set to false, an `error` text, and one `error_code` from the table below. The error text MUST name the failed check and the values involved (A11.1).

| Code | Used by | Meaning |
|---|---|---|
| `insufficient_role` | every operation | The caller's role is lower than the operation's role. |
| `missing_auth_token`, `invalid_auth_token` | every operation | The control token is missing or wrong. |
| `invalid_experiment` | experiments | The request is malformed, has an unknown field, or breaks a rule of E5.3. |
| `invalid_target`, `invalid_address`, `incompatible_binding` | experiments; `invalid_target` also `control.pid.*` | The target is not writable, the address is invalid, or the declared type does not match the variable. |
| `invalid_value`, `outside_limits`, `restoration_mismatch`, `bad_runtime_data` | experiments | A value cannot be decoded, a value lies outside its limits, the restoration value differs from the current value, or a feedback value is not finite. |
| `forced_signal`, `stale_scan`, `io_unavailable` | experiments | The target is forced, the snapshot is stale after a reload, or no complete scan is available. |
| `controller_mode_blocked` | experiments | The loop is not in the mode that the kind requires. |
| `abort_already_asserted` | experiments | The abort signal already reads its asserted value. |
| `experiment_active` | experiments and live writes | An experiment is running or restoring. |
| `transaction_pending` | experiments | A transfer or parameter transaction is pending. |
| `experiment_not_found` | experiments | No experiment record holds the identifier. |
| `experiment_state_unavailable` | experiments | The experiment state lock cannot be taken. |
| `physical_approval_required` | experiments and `control.pid.apply` | A physical write has no approval. |
| `invalid_transfer_request` | transfers | The transfer request is malformed or breaks a check of E6.5 or E6.6. |
| `invalid_parameter_request` | parameters | The parameter request is malformed or breaks a check of E7.3 or E7.5. |
| `invalid_controller_request` | `control.pid.*` | The PID request is malformed or sets an invalid field. |
| `controller_unavailable`, `controller_write_pending`, `stale_controller_revision`, `rollback_unavailable`, `rollback_revision_mismatch` | `control.pid.*` | The controller record is missing, a write is pending, or a revision does not match. |
| `signals_unavailable` | `control.signals.inspect` | The inspection request cannot be served. |
| `recording_unavailable` | recordings | The recording request cannot be served. |
| `invalid_simulation_run_request` | isolated test runs | The run request is malformed or breaks a limit or a rule of E8.4. |
| `invalid_search_request` | searches | The search request is malformed or breaks a limit or a rule of E8.5. |

A refused `config.set` request carries an `error` text and no `error_code`.

## E1. Identity and runtime facts

### E1.1 Terms

- **Scan**: one resource cycle.
- **Execution**: one run of a task. A task runs at the first scan at or after it is due (A4.3).
- **Sequence**: for a task, the number of its executions, counted from the start of the session epoch. For the background, the sequence is the scan number. A period in which the task did not run leaves a gap (E4.3).
- **Session epoch**: the unsigned integer `session_epoch` that `status` reports. The runtime assigns a new value when it ends the bindings of the running operations: at a program reload, a restart, a fault or a stop of execution.
- **Program revision**: an opaque string `sha256:<hex>`. The runtime derives it from the session epoch, a value fixed at process start and the compiled program. It changes whenever the session epoch changes. The VS Code debug session plays no part in it.
- **Binding revision**: an opaque string `sha256:<hex>` that the runtime derives from the complete binding of a request. Every field counts, including units and limits.
- **Loop revision**: the revision of the loop definition (B1.4), as Studio sends it. The runtime stores it and compares it for equality. It never interprets it.
- **Project revision**: an opaque string `sha256:<hex>` for the loaded source snapshot and the project's simulation configuration (E8.2).

Revisions and epochs are compared for equality only. The session epoch starts from a value unique to the runtime process, so a restarted runtime never repeats an earlier epoch; Studio still compares the program revision together with the session epoch. No debug session is part of any identity (A2.2).

### E1.2 Process time

Every sample, status and terminal record carries the runtime time of the scan or execution that it describes. Studio measures durations from successive runtime times (A3.2). Studio never compares the runtime clock with its own clock (A3.4).

### E1.3 Runtime facts

Studio reads these facts from the `status` operation (A2.3). It never takes them from editor settings. `status` takes no parameters. Its result has these fields.

| Field | Type | Meaning |
|---|---|---|
| `session_epoch` | integer | The session epoch (E1.1). |
| `simulation_enabled` | boolean | Whether simulation is enabled. |
| `simulation_only_io` | boolean | True when every registered I/O driver reports itself as simulated and no ADS or OPC UA client connection is configured. False until the first scan has published its I/O snapshot. |
| `simulation_time_scale` | integer | The configured speed `time_scale`, a whole number from 1 to 100, and 1 without simulation (spec 33 §6.1, items 2 and 5). |
| `simulation_achieved_time_scale` | number or null | The achieved speed: simulation time advanced divided by wall time elapsed over the last 5 s of wall time, the lowest of the resources; `null` until one second of wall time has been measured (spec 33 §6.1, item 3). |
| `program_revision` | string | The revision of the loaded program, the same value each family operation returns (A2.2, A2.3). |
| `tasks` | array | One `{name, interval_ms}` entry for each scheduled task. An event task has `interval_ms` 0. |
| `cycle_interval_ms` | integer | The resource cycle interval. |

The result has other fields that describe the runtime state, its drivers and its metrics. This annex does not use them.

Each family operation also returns the program revision that applies to it.

The runtime MUST decide whether a target is simulated or physical by the rule of A2.4. A target is simulated only when simulation is enabled and all attached I/O is simulated. A configured ADS or OPC UA client connection is not simulated I/O (A2.3). The runtime MUST apply that decision to every write it receives.

### E1.4 Simulation clock and speed

Specification 33 §6.1 defines the simulation clock and the speed. Its rules, as this annex uses them, are these.

1. Simulation time is the runtime's process time while simulation is enabled (A3.1). It advances by exactly one resource cycle interval for each executed cycle. It MUST NOT be read from the wall clock. Without simulation, process time follows the wall clock, and a cycle that starts late counts as an overrun.
2. The runtime paces cycles against the wall clock at the configured speed `time_scale`, a whole number from 1 to 100. At speed `s`, a cycle is due every resource cycle interval divided by `s` of wall time. The runtime MUST NOT run extra cycles to recover lost wall time.
3. A host that cannot keep the pace runs the simulation more slowly, with unchanged results. The achieved speed reports it (E1.3).
4. While the resource is paused, simulation time does not advance. A warm or cold restart continues simulation time from its last value. After a pause, a restart or a speed change, the schedule restarts at the next cycle.
5. `config.set` with the key `simulation.time_scale` changes the speed of a running simulation. The value is a whole number from 1 to 100. The new speed applies from the next cycle and never moves simulation time. The change is not persisted: it lasts until the runtime stops. A runtime without simulation MUST refuse the key. A successful request lists the key in `updated` and does not list it in `restart_required`. The project keeps its speed in `simulation.toml` (`[simulation] time_scale`), and the launch option `--time-scale` overrides that speed at launch.
6. Speed does not change results. Everything scheduled in simulation time gives the same values at equal simulation timestamps at any speed and under any host load. A write from outside the runtime, such as an operator command or a debugger write, takes effect at the first cycle after it arrives. Its simulation time therefore depends on the speed.

## E2. Symbol bindings and resolution

### E2.1 Symbols

A symbol is a dotted path. Its root is a global variable of the loaded program, usually a program instance. The path may pass through function block instances, including members that a block inherits through `EXTENDS`, and through structure fields. Example: `Main.Mixer.Speed.Value`.

An enumeration is read as its numeric value. Its binding names the enumeration type, with or without a namespace.

### E2.2 Types

| Operation | Permitted IEC types |
|---|---|
| Reads: inspection, recordings, isolated observations | BOOL, signed and unsigned integers, REAL, LREAL, and enumerations read as numbers |
| Transfer modes | BOOL |
| Transfer states, guards and outputs | REAL, LREAL, or BOOL with unit `1` and bounds 0 to 1 |
| Parameters | REAL and LREAL |
| Experiment targets | REAL, LREAL and integer types for a symbol. Any value type of E5.3 for an address. |
| Experiment activation and abort bindings | BOOL |
| Search axes and objective channels | REAL and LREAL |
| Isolated parameter overrides | REAL, LREAL, or a fixed configuration BOOL |

The `iec_type` of a binding MUST equal the name of the variable's type in upper case. Integer and enumeration gains are not supported (E7.8).

### E2.3 Ownership and mutability

- Every symbol in one operation belongs to one compiled program.
- Transfer members belong to the declared task, or to `$background`, and to no other owner.
- Parameter paths belong to programs owned exclusively by the declared task.
- The target and the mode symbol of an experiment MUST belong to the experiment's `task`.
- Constants, `IN_OUT` aliases, external variables and addressed variables are refused as write targets.
- Transfer modes and states are mutable, not external and not aliased.

### E2.4 Addressed and symbolic selectors

Each binding names exactly one selector, `address` or `symbol` (A5.1). A binding with both or with neither is refused.

- An `address` is an explicit I/O address. Addresses follow the existing non-overlap rules.
- A `symbol` names a program member. The runtime reads or writes its variable directly.

| Operation | Address selector | Symbol selector |
|---|---|---|
| Inspection and recordings | Read through the I/O snapshot. The address must match a bound entry exactly. | Read directly. |
| Parameters | Refused. | Unaddressed scalar members only. |
| Transfer mode | Refused. | Unaddressed only. |
| Transfer states | Unaddressed, or bound to `%M` only. | Unaddressed members. |
| Transfer outputs | Bound to `%Q` or `%M`. The held value rules of E6.6 apply. | Unaddressed members. |
| Transfer guards | Read through the I/O snapshot. | Read directly. |
| Experiment target | Written through the address. | Unaddressed variables only. |
| Experiment abort binding | Written through the address. | Refused. |
| Experiment relay feedback | Read through the address. | Refused. |
| Isolated parameter overrides and grid axes | Refused. | Writable unaddressed scalar members. |

### E2.5 Nested scalar leaves

A binding may name a scalar leaf through a path that crosses function block instances and structure values. Example: `Main.Mixer.Controller.ParametersValue.ProportionalGain`. The runtime resolves the path through the same variable storage that the program's own execution uses.

- A path is refused if it is missing, names a non-scalar, or names a member that is not writable for a write operation.
- Each declared leaf is written once, by its owning transaction. A nested path is not a second write path.
- Writes to several leaves are staged and committed together at the owning task boundary (E6.6, E7.4). A read observes the leaf's current value.

### E2.6 Resolution after a reload

- Each read resolves its symbol again at every scan against the program that is loaded at that moment.
- A symbol that moved reads as `missing`. It never reads as another variable's value.
- A forced variable gives its sample the quality `forced`.
- A changed type gives the quality `bad` with no value. An earlier value is never reused.
- The runtime MUST NOT write a symbol that was resolved against an old program. Queued writes that were resolved against the old program are dropped.
- A reload or restart ends the bindings of the old identity. Studio resolves the new identity before any further live action (E11.5).

## E3. Inspection: control.signals.inspect

Access: Viewer.

Request:

- `loop_revision`: string, required, 1 to 256 bytes.
- `bindings`: array, required, 1 to 32 entries. Each entry has:
  - `id`: string, required, 1 to 128 bytes, unique in the request;
  - `iec_type`: string, required, the upper-case type name (E2.2);
  - exactly one of `address` (string) or `symbol` (string, at most 256 bytes). An explicit `null` selector counts as absent.
- `expected_program_revision`: optional string. A value that differs from the current program revision is refused.

Response before the first complete scan:

- `program_revision`, `loop_revision`, `bindings` (an echo of the request), `available` set to false, and `reason` set to `Waiting for the next coherent runtime scan.`

Response with a scan:

- `program_revision`, `loop_revision` and `bindings` (an echo of the parsed bindings).
- `available`: true.
- `scan`: number, the resource cycle count.
- `runtime_time_ns`: number, the runtime time of that scan.
- `captured_at_ms`: number, the runtime wall-clock time of capture. It is for display only. Ordering uses `scan` and `runtime_time_ns`.
- `values`: array, one entry per binding. Each entry is a number or null. A BOOL is 0 or 1.
- `forced`: array of booleans, one entry per binding.
- `quality`: array of strings, one entry per binding: `good`, `bad`, `forced` or `missing`.

Arrays follow the order of `bindings`.

Refusals use `signals_unavailable`. The text names the failed check, for example `program revision changed`, `select 1 to 32 channels`, `addressed signal unavailable` or `symbolic signal type mismatch`.

Rules:

1. The runtime keeps one inspection watch for the runtime. Its identity is `loop_revision`, `program_revision` and the bindings in order. A request with the same identity returns the latest response and changes nothing. A request with another identity replaces the watch once it has been validated. Polling never adds a watch.
2. All values in one response come from one scan.
3. Quality is assigned in this order. `missing`: the symbol or address does not resolve. `bad`: the value is not a number or BOOL, is not finite, is an integer outside ±(2^53 − 1), or has the wrong type. `forced`: the value is forced and is otherwise valid. `good`: otherwise. A bad value stays bad when it is also forced.
4. A value whose quality is `bad` or `missing` is reported as null. A value is never reported as `good` when it is forced, bad or missing.
5. `stale` is not a runtime quality. Studio derives it from the wall-clock time since the last newer scan (A3.4).
6. An inspection is evidence for its own scan only. An editor poll is never evidence (B2).
7. A read of a loop's signal MUST be taken at the end of an execution of the loop's task (A5.2).

## E4. Recordings

### E4.1 Operations

`control.recording.start`, `control.recording.status`, `control.recording.stop` and `control.recording.release`. A recording observes. It never writes a signal and never excites the process. Recording operations are not live writes (E9.1). They use the Viewer role (E0.1).

### E4.2 Start

Request:

- `recording_id`: string, required (E0.2).
- `loop_revision`: string, required, not empty.
- `bindings`: array, required, 1 to 32 entries. Each entry has `id`, `iec_type` and exactly one selector, as in E3.
- `maximum_samples`: integer, required, 2 to 100,000 samples per channel.
- `maximum_duration_ms`: integer, required, 10 to 86,400,000 (24 h of process time).
- `task`: optional string, the name of a configured task, matched without regard to case. When it is absent, the recording samples at the end of every resource cycle.
- `record_every`: optional integer, 1 to 100,000. It is the number k of sequence values in one sample (E4.4). The default is 1.
- `program_revision`: required when any binding is a symbol. It MUST equal the current program revision.

The start is refused unless all of these hold:

- every binding is valid (E2.4), and the binding identifiers are unique;
- the bindings are at most 32, and the channels times the samples are at most 1,600,000;
- the duration is at most 24 h of process time, and `record_every` is at most 100,000;
- `task` names a configured task;
- the recording identifier is not held;
- the runtime holds fewer than two recordings. A recording counts until it is released, whether it runs or has ended;
- for symbolic bindings, `program_revision` equals the current program revision, and the inspection watch (E3) holds the same loop revision, program revision and bindings, with a sample of the latest scan;
- for any other binding, a `program_revision` that is given equals the current program revision, which Control Studio sends with every start so that a connected record names the program it ran on (A4.4).

The response has `recording_id` and `status` set to `recording`.

Refusals use `recording_unavailable`. Their texts name the limit or the failed check, for example `Recording channels are limited to 32.`, `Release a retained recording before starting another; the limit is 2 recordings at a time.` or `The program runs no task named {task}.`

### E4.3 Sampling

- A recording of a task MUST take one sample at the end of each execution of that task, after its Structured Text has run. The sample MUST be stamped with that execution's process time (A4.1).
- A recording without a task takes one sample at the end of each resource cycle (A4.1).
- Only executions of the named task are recorded. Its sequence counts its executions (E1.1). The sample limit therefore counts the task's executions, whatever the resource cycle.
- A period in which the task did not run leaves a gap in the sequence (A4.4).

### E4.4 Record period and quality

- With `record_every` equal to k, a sample covers k consecutive sequence values. Its value for a numeric signal is the mean of the k values, and for a BOOL signal the last of them (A4.2). Its time is the mean of the k execution times. Its `sequence` is the last of the k sequence values.
- A sample is complete only when all k executions ran. A block with a missing execution is a gap. When a recording stops, its last sample may cover fewer than k executions; it carries its own mean time and does not make the recording incomplete (A4.2).
- The quality of a sample for one signal is the worst of its k qualities, in this order: `bad` (a bad or missing value), `forced`, `good` (A4.2).
- The runtime does not report saturation or clipping. Control Studio finds them from the recorded values and the loop's declared limits (A4.2).

### E4.5 Completeness and acquisition evidence

- A recording is complete when it holds every execution of its interval, its sequence has no gap, its times strictly increase, and each sample carries its runtime time (A4.4).
- A missed execution, a non-increasing time, an absent timestamp or a runtime boundary makes the recording incomplete. The gap stays in the evidence.
- Incomplete evidence remains evidence. It is never used where continuous data is required (A4.4).
- The stop response gives the `acquisition` object: `completeness` (`complete` or `incomplete`), `missingScanRanges` (`{first, last}` entries), `scanRange` (`{first, last}` or null), `observedSampleCount`, `droppedObservedSamples` (always 0), `actualCycleTimingAvailable`, `freshnessAvailable` (always false), `method` (`runtime_scan_buffer`) and `reason` (text).

### E4.6 Status

Access: Viewer. Request: `recording_id` and `loop_revision` (both required), and optionally `from_sample`.

Response:

- `recording_id`, `state` (`recording`, `stopped` or `failed`), `elapsed_ms` (process time from the first to the last captured execution), `sample_count` (closed samples only), `complete`, and `reason` (text or null).
- `samples`: at most 2,000 samples from `from_sample` on. `from_sample` defaults to the end of the recording, which gives an empty page. Each sample has `scan`, `runtime_time_ns`, `captured_at_ms`, `values` (array, number or null), `forced` (array of booleans), `quality` (array of `good`, `bad`, `forced` or `missing`) and, when it is a gap, `gap` set to true.
- `next_sample`: the index for the next request. It is always present.

`failed` means that the recording stopped with stop reason `scan_reset` or `runtime_boundary`.

Status never changes the recording. A recording keeps running when clients disconnect. A client that reattaches follows the recording by `from_sample`.

### E4.7 Stop and release

Stop (Viewer). Request: `recording_id` and `loop_revision`, both required. The loop revision MUST match the recording's loop revision. A mismatch is refused.

Stop response: `recording_id`, `loop_revision`, `program_revision` (when the recording has one), `bindings` (an echo of `id`, `address`, `symbol` and `iec_type`), `stop_reason`, `samples` (all of them) and `acquisition` (E4.5). A stop is idempotent. A repeated stop returns the same result.

Release (Viewer). Request: the same fields as stop. Response: `recording_id` and `status` set to `released`. A recording that still runs is refused with the text `Stop the recording before releasing it.`. Release frees the recording and its slot. Release of an unknown `recording_id` succeeds.

Stop reasons:

- `stopped`: a stop request.
- `sample_limit`: `maximum_samples` is reached.
- `duration_limit`: `maximum_duration_ms` is reached.
- `scan_reset`: the scan sequence was reset or repeated.
- `runtime_boundary`: a program reload, a restart, a fault or a stop of execution ended the recording.

### E4.8 Evidence

A recording and each of its samples MUST carry an evidence source of A9: Simulated plant on a simulated target, and Physical plant on a physical target. The source is fixed when the recording starts.

### E4.9 Lifetime and limits

- A recording ends by a stop, by one of its limits, or by a runtime boundary. It reports its stop reason in the stop response.
- Recordings are held in the runtime's memory. A process restart ends them with the stop reason `runtime_boundary`.
- The recording limits are those of A7 and E12.2.

## E5. Experiments

### E5.1 Operations and purpose

`control.experiment.start`, `control.experiment.abort` and `control.experiment.finish` require Engineer access. `control.experiment.status` requires Viewer access. An experiment writes one affected signal, its target, and ends with an automatic restoration. Observation without excitation is a recording (E4), and the `observation` kind of this operation writes nothing.

The runtime MUST evaluate an experiment after each execution of the loop's task (A3.5). A trajectory point whose time has been reached MUST be written for the next execution, so it takes effect at most one execution after its time. A relay MUST decide its next level after every execution, from that execution's measurement.

### E5.2 Kinds and required mode

| Kind | Affected signal | Required mode (A6.5) |
|---|---|---|
| `observation` | none | none |
| `mv_step`, `output_step` | The target of the request (E5.3). The runtime does not check its role. | Manual |
| `mv_pulse` | As `mv_step` | Manual |
| `relay` | As `mv_step` | Manual |
| `prbs`, `multisine` | As `mv_step` | Manual |
| `setpoint_step` | The target of the request | Auto |
| `load_step` | The target of the request | Auto |
| `project_sequence` | A BOOL activation signal | none |

- The runtime executes the trajectory that the request gives. The trajectory is the complete plan of the experiment.
- An `mv_step` has one point and holds it until restoration.
- An `mv_pulse` has the points `(0, value)` and `(duration_ms / 2, restoration_value)`. Its return point occurs before the end.
- A `relay` has no trajectory. Its output follows the process value (E5.5).
- A `project_sequence` writes no plant input and no manual output. It carries no mode binding.
- A manual output that the program copies from the output is changed only in Manual (A6.5).

### E5.3 Start request

Access: Engineer. Unknown fields are refused with `invalid_experiment`.

| Field | Type | Rule |
|---|---|---|
| `experiment_id` | string | Required. Not blank (E0.2). |
| `source` | string | Required. `simulation` or `physical`. A `physical` source needs `physical_approval` (E10.2). |
| `kind` | string | Required. One of the kinds of E5.2. |
| `task` | string | Required. A configured task name, matched without regard to case. `$background` is the resource cycle. |
| `address` or `symbol` | string | Exactly one (E2.4). This is the affected signal. |
| `value_type` | string | Optional. One of BOOL, SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT, REAL or LREAL, in any case. It MUST match the runtime type of the signal. |
| `value` | number | Required. The level of the step, pulse or relay high. It is finite and lies within `minimum` and `maximum`. |
| `restoration_value` | number | Required. It MUST equal the current value of the target. Observation is excepted. |
| `minimum`, `maximum` | number | Required. Finite, with `minimum` below `maximum`. |
| `duration_ms` | integer | Required. 1,000 to 86,400,000 (1 s to 24 h of process time). |
| `expected_scan` | integer | Required. The runtime stores it as `reviewed_scan`. It does not compare it. |
| `mode_binding` | object | `{symbol, manual_value}`. Required for the kinds that need a mode (E5.2), and ignored for the others. `manual_value` is a boolean or a finite number. The mode symbol may be BOOL, numeric or an enumeration. |
| `physical_approval` | object | Optional. `{user_approved, topology_reviewed}`, both booleans. Both MUST be true for a `physical` source. |
| `trajectory` | array | Optional, default empty. Entries `{at_ms, value}`. At most 10,000 entries. |
| `relay_feedback` | object | Required for `relay` only, and refused for the other kinds. `{address, value_type, center, hysteresis, low_value, high_value}`. |
| `abort_binding` | object | Optional, for `project_sequence` only. `{address, value_type, asserted_value}`. The address is the only selector; `value_type` is BOOL. |

Trajectory rules: the first point is at 0; the times strictly increase; every time is below `duration_ms`; consecutive points are at least one period of the task apart; every value is within the limits.

Relay rules: `center` is finite; `hysteresis` is greater than 0; `low_value` is below `high_value`; `value` equals `high_value`; the feedback address is read, not written.

Success response: `experiment_id`, `status` set to `running`, `restoration` (`pending`, or `not_required` when no write was queued), and the echo `address` or `symbol`.

The start is checked in this order. The first failed check is the refusal.

1. The request decodes, with no unknown field (`invalid_experiment`).
2. The identifier is not blank; the duration is 1 s to 24 h; `minimum` and `maximum` are finite and ordered; `source` is `simulation` or `physical`; `kind` is known (`invalid_experiment`).
3. `source` is `physical` and either approval flag is false (`physical_approval_required`).
4. The task exists (`invalid_experiment`). The target is resolved. It is writable, and for a symbol its type is REAL, LREAL or an integer (`invalid_target`). A snapshot made stale by a reload is refused (`stale_scan`).
5. The value type is supported, and `value` and `restoration_value` decode (`invalid_value`). `value` lies within the limits (`outside_limits`).
6. A complete scan has been published (`io_unavailable`). The target is not forced (`forced_signal`).
7. `value_type`, if given, matches the runtime type (`incompatible_binding`). `restoration_value` equals the current value (`restoration_mismatch`).
8. Relay feedback is given only for `relay`, and its checks hold. An abort binding of a `project_sequence` is BOOL, is not the activation signal, is writable and not forced, and does not already read its asserted value (`abort_already_asserted`).
9. The trajectory and its spacing hold (`invalid_experiment`). A `mv_pulse` has its two points.
10. The mode that the kind requires is observed (`controller_mode_blocked`). A mode binding is present where the kind requires one (`invalid_experiment`).
11. No transfer or parameter transaction is pending (`transaction_pending`), and no other experiment runs or restores (`experiment_active`). The experiment lock is held (`experiment_state_unavailable`).
12. The session epoch has not changed since the target was resolved (`stale_scan`).

### E5.4 Execution

At each execution of the loop's task, the runtime does the following, in this order. Every stop has a stop reason (E5.7).

1. **Mode.** The runtime reads the mode at the start and after every execution. If the mode is not the required one, it stops with `mode_changed`. A mode that cannot be read stops with `bad_runtime_data`.
2. **Trajectory.** The runtime MUST write a trajectory point for the next execution once its time has been reached at an execution (A3.5). Times count from the process time of the first execution after the start. A later write to the same variable before that execution replaces the earlier one.
3. **Relay.** The relay compares the process value with its thresholds after every execution. The first level is high. The relay switches to low when the process value reaches or exceeds `center + hysteresis`. It switches to high when the process value reaches or falls below `center - hysteresis`. Otherwise it keeps its level. A new level takes effect at the next task boundary.
4. **Stop conditions.** The runtime stops the experiment when one of these holds:
   - the target, or the relay feedback, is forced: `signal_forced` or `relay_feedback_forced`;
   - the target or the process value is not a valid value: `bad_runtime_data` or `bad_relay_feedback`;
   - the target or a relay level lies outside the limits: `signal_outside_limits`;
   - the mode leaves the required one: `mode_changed`;
   - the abort signal of a project sequence reads its asserted value: `project_abort_asserted`; the abort signal is forced: `abort_signal_forced`; the abort signal is not a valid BOOL: `bad_abort_binding`;
   - the duration has elapsed: `duration_complete`.
5. **Abort and finish.** An abort or finish request takes effect at once. The status moves to its terminal state, the restoration is queued, and the next execution applies no point and no level.
6. **Restoration.** At the stop, the runtime writes `restoration_value` to the target. For a stop other than `duration_complete` or `finished_early`, an abort binding is asserted in the same step.
7. **Confirmation.** The restoration is confirmed when the target reads `restoration_value` at the end of an execution that is no later than the third execution after the restoring write (A7). Otherwise the status becomes `failed_to_restore` with the restoration reason `confirmation_timeout`.
8. **Timing.** Durations count in process time. The runtime keeps no wall-clock timer for an experiment (A3.2, A3.3).
9. **No controller write.** An experiment writes no controller parameter and calculates no gain.

A terminal experiment MUST NOT read `completed` until its restoration is confirmed (A7). The restoration of a symbol that a reload removed is not written (E9.3).

### E5.5 Abort, finish and abort binding

- `control.experiment.abort` takes `experiment_id`. It sets the status to `aborted` with the stop reason `user_abort`. It is idempotent: a record that is not running is returned unchanged. The response is the full status record.
- `control.experiment.finish` takes `experiment_id`. It sets the status to `completed` with the stop reason `finished_early`. The abort binding is not asserted.
- An unknown `experiment_id` is refused with `experiment_not_found`.
- The abort binding is a BOOL signal, named by `address`, with an `asserted_value`. Only `project_sequence` accepts it.
- The abort is asserted for every stop except `duration_complete` and `finished_early`. The runtime never chooses the actuator value. The program does.
- Status of the abort binding: `not_requested`, `pending`, `observed` or `failed`. `observed` means that the asserted value was read at a later execution. `failed` means that it was not observed by the restoration deadline.

### E5.6 Status

Access: Viewer. Request: `experiment_id`.

Response keys:

- `experiment_id`, `source`, `kind`, `task`, and `address` or `symbol`, as given.
- `status`: `running`, `completed`, `aborted`, `failed` or `failed_to_restore`.
- `stop_reason`: null while the experiment runs.
- `restoration`: `pending`, `not_required`, `restored` or `failed`.
- `restoration_reason`: null, `confirmation_timeout`, or the stop reason of a reload or restart.
- `elapsed_ms`, `duration_ms`, `reviewed_scan` and `started_scan`.
- `last_applied`: `{index, value, time_ms, scan, process_time_ms}` for the last trajectory point applied, or null. It holds the value read back after the execution that followed the write.
- `relay_feedback`: for a relay, `{address, center, hysteresis, lower_threshold, upper_threshold, low_value, high_value, high_active}`. Otherwise null.
- `abort_binding`: `{address, asserted_value, state, observed_scan}`, or null when the experiment declares none.

A status request MUST NOT change the record. A reload or restart ends the experiment at the boundary (E9.3).

### E5.7 Terminal states

| Stop reason | Status | Restoration |
|---|---|---|
| `duration_complete` | `completed` | Written, then confirmed (E5.4). |
| `finished_early` | `completed` | Written, then confirmed. |
| `user_abort` | `aborted` | Written, then confirmed. |
| `mode_changed`, `signal_forced`, `bad_runtime_data`, `signal_outside_limits`, `relay_feedback_forced`, `bad_relay_feedback`, `project_abort_asserted`, `abort_signal_forced`, `bad_abort_binding`, `physical_approval_required` | `failed` | Written, then confirmed. |
| Any stop, when the restoration is not confirmed by the deadline | `failed_to_restore` | `failed`, with reason `confirmation_timeout`. |
| `program_reloaded`, `runtime_restarted` | `failed` | A symbol target is not restored. An address target is restored. |

For a stop that writes the restoration, the restoration is `not_required` when no write was queued. The experiment record holds one result. A new start replaces it. No terminal record is relabelled.

### E5.8 Interlocks

- An experiment start is refused with `transaction_pending` while a transfer or parameter transaction is pending.
- An experiment start is refused with `experiment_active` while another experiment runs or restores.
- A transfer apply, a parameter apply or rollback, and a `control.pid.*` write are refused with `experiment_active` while an experiment runs or restores.
- Recordings, inspections, isolated runs, searches, and the stop, abort, cancel and release requests are never refused for these reasons.

## E6. Transfers

### E6.1 Operations and purpose

`control.transfer.status` requires Viewer access. `control.transfer.apply` and `control.transfer.cancel` require Engineer access. A transfer changes the loop's modes and declared states at one task boundary, and it observes the output (A6.3). It writes only the declared modes, states and tracking values. It never writes the controller output as a command. The controller remains the actuator writer. On a failed check, the transfer also writes each output back to the value it held at the boundary (E6.7). That is a restoration, not a command. Transfers are live writes (E9.1).

### E6.2 Binding

The status, apply and cancel operations take the same binding.

- `loop_id`: string, not blank, at most 128 characters.
- `task_name`: string. It names the one task that owns the program of every member, or `$background` for an unscheduled program.
- `mode`: `{symbol, manual_value}`. `symbol` is a mutable BOOL variable that is not external and not addressed. `manual_value` is a boolean.
- `additional_modes`: optional array of 0 to 8 modes, in the same form. They are written with `mode` at the same boundary.
- `states`: 1 to 64 distinct members.
- `guards`: 1 to 64 members.
- `output`: one member. It is the controller output that the transfer holds (A5.3).
- `additional_outputs`: optional array of 0 to 15 members.
- `tracking_outputs`: optional object that maps an output symbol to a feedback guard symbol.

A member is `{symbol, iec_type, unit, minimum, maximum}`. `iec_type` is REAL, LREAL or BOOL. The `unit` is not blank. `minimum` and `maximum` are finite and `minimum` does not exceed `maximum`. A BOOL member has unit `1` and bounds 0 and 1.

Rules:

- All members belong to one program and to the declared task (E2.3).
- Member symbols are distinct, without regard to case.
- Modes are unaddressed. A mode MUST NOT have an I/O binding.
- A state is unaddressed or bound to `%M`. States MUST NOT overlap one memory address.
- An output is unaddressed, or bound to `%Q` or `%M`.
- A guard may be unaddressed or addressed.
- The binding has no `tracking` field. Tracking is an apply field (E6.5).
- The binding revision is a hash of the runtime's serialization of the binding.

### E6.3 Status

Access: Viewer. Request: `binding`.

Response:

- `program_revision`, `binding_revision` and `runtime_time_ns`.
- `values`: an object with one entry for each member and each mode. Modes are booleans. A BOOL member is 0 or 1.
- `supported_tracking`: `["proportional", "parallel-pid-v1", "declared-state-v1", "declared-initialization-formulas"]`.
- `transaction_id`: the identifier of the last transaction, or null.
- `status`: the status of the last transaction, or `idle`.
- `reason`: text or null.
- `transaction`: the last transaction record (E6.8), or null.

Status values are observations. They are neither requested values nor write authority. Status MUST NOT claim that a queued change is applied, and it MUST NOT change the transfer record.

### E6.4 Tracking

The `tracking` object of an apply selects one of these kinds.

`proportional`: `{kind: "proportional", reference_symbol, manual_command_symbol, process_variable, gain_symbol, align_reference}`.

- `reference_symbol` and `manual_command_symbol` are states. `process_variable` and `gain_symbol` are guards.
- The gain is an exact value. The process variable is a measurement and is checked by bands (A6.4).
- At the boundary the manual command state is set to the held output, rounded to its IEC type.
- If `align_reference` is true, the reference is set to the process variable plus the held output divided by the gain. The result is rounded to the reference type, and its bounds are checked. The gain MUST be finite and non-zero for alignment. Otherwise the reference is the value that the request gives.
- With tracking, `values` holds the states that the transfer does not compute. A computed state that lies outside its bounds is rejected at the boundary with the reason `<symbol> outside declared bounds`.

`parallel-pid-v1` adds `proportional_gain_symbol`, `integral_gain_symbol`, `derivative_gain_symbol`, `integral_state_symbol`, `derivative_state_symbol`, `proportional_setpoint_weight` and `derivative_setpoint_weight` (each from 0 to 1), `cycle_seconds` (0 to 60) and `align_output`. Its controller law is the one of Part C.

`declared-state-v1` adds `initialization_values`, `initialization_formulas`, `bounded_state_symbols`, `additional_references`, `additional_commands` and `additional_measurements`, of at most 16 entries each. With `align_output`, at the boundary the reference and each additional reference are set to their process variables, rounded to their types, so that the first output is bumpless (A6.4); a later reference step without `align_output` writes the references `values` gives, as Apply's target change and the Restore's return of the references Apply found do (specification 35, B6.3, B6.4). A bounded state is a state that the program sets, such as a manual output that follows the output in Auto (A6.4). Its `initialization_formulas` are of the kinds `pid-integral`, `weighted-error`, `notch-history`, `model-input`, `model-output`, `model-input-is`, `scaled-measurement` and `decoupled-output`, at most 8 entries. Their meaning is that of Part C.

### E6.5 Apply

Access: Engineer. Request fields. Every field is required except `tracking`.

| Field | Type | Rule |
|---|---|---|
| `transaction_id` | string | 1 to 128 characters, not used before in this epoch. |
| `binding` | object | E6.2. |
| `expected_program_revision`, `expected_binding_revision` | string | Equal to the current revisions. |
| `expected_values` | object | Every member and mode of the binding. Measurements and outputs are included. |
| `values` | object | Exactly the states that the transfer does not compute. |
| `manual` | boolean | `true` leaves the loop in Manual. `false` leaves it in Auto. |
| `maximum_output_change` | number | Finite, zero or more. Required for every apply. |
| `source` | string | `simulation` or `physical`. |
| `physical_approved` | boolean | Required. It MUST be true for a `physical` source (E10.2). |
| `tracking` | object or null | Optional (E6.4). |

The checks run in this order. The first failed check is the refusal.

1. The runtime metadata and the parameters are available.
2. No experiment runs or restores: `experiment_active`.
3. The request decodes, with no unknown field.
4. `transaction_id` is valid and `source` is `simulation` or `physical`.
5. A `physical` source without `physical_approved` is refused with the text `physical review required`.
6. `maximum_output_change` is finite and zero or more.
7. The binding is valid (E6.2).
8. The tracking object is valid (E6.4).
9. The values are exactly the states that the transfer does not compute.
10. Each value is within its bounds, is exactly representable, and has the member's type. A REAL value MUST be explicitly rounded to single precision.
11. No other transfer or parameter transaction is pending.
12. No variable or I/O is forced.
13. The `transaction_id` is not used before in this epoch, and the epoch has room for another identifier (1,024 per epoch).
14. The snapshot is available.
15. The expected program revision, binding revision and values match.
16. The transfer status was read before this apply, and the binding has not changed since that read.
17. A transfer whose record is already applied is not applied again.

An accepted apply returns `status` set to `pending` and `confirmed` set to false.

### E6.6 Boundary check and commit

At the owning task's boundary, before the task executes, the runtime rechecks the following. A mismatch in any item rejects the whole set. Nothing is written, and the record becomes `rejected` with its reason.

- The program revision, the binding revision and the expected values.
- The binding: ownership, types, bounds and mutability of every member.
- Forcing: no mode, state or guard is forced.
- The I/O authority: when any attached I/O driver is not simulated, the physical approval MUST be present (E10.2). Without it, the transaction is rejected with the reason `attached I/O requires explicit physical review`.
- The tracking values are computed (E6.4).

If every item passes, the runtime writes all modes and states together, after input acquisition and before the owning task executes. The record keeps the initial values read at the boundary and the applied values.

The held value of an output is the value it had at the task boundary when the output is unaddressed. An addressed output MUST be held at the committed output image (A5.3).

The transfer MUST check that each measurement is fresh, of good quality and within its range (A6.4). A measurement is never required to stay unchanged.

### E6.7 Post-execution check and confirmation

After the owning task executes, the runtime reads the observed mode, states and output.

- The transaction is `failed` when a state or the mode changed, when the output is invalid or out of range, or when the output differs from its held value by more than `maximum_output_change` at the first execution. The record keeps its evidence.
- A failed check returns every member that the transaction wrote to the value it had before the transaction, in the same scan. It also returns each output to its value at the boundary. The rejected outputs are not published. The loop keeps the mode it had (A6.3). `restored` is true when every mode and state write-back succeeded.
- A passing check is not yet `applied`. The transaction stays `pending` with the confirmation `post-st`.
- The transaction becomes `applied` with the confirmation `output-image` when the cycle's output image commits successfully. This applies to addressed and unaddressed outputs alike.
- A failed output commit makes the transaction `failed`.
- `applied` is runtime and output-image evidence. It is not physical actuator feedback (A9).

The output-change check applies to every transfer, Manual transfers included.

### E6.8 Transaction record

The `transaction` object has these fields: `id`, `program_revision`, `binding_revision`, `status`, `reason`, `restored` (always present), `initial_values`, `applied_values`, `observed_values` and `confirmation`.

- `status` is `pending`, `applied`, `failed`, `rejected` or `cancelled`.
- `rejected` means that the transaction was refused before any write (E6.6).
- `failed` means that a write happened and that a later check or commit failed. A runtime boundary (E9.3) and a cycle fault also give `failed`.
- `cancelled` means that the transaction was cancelled before its commit (E6.10).
- The top-level `status` repeats the record's status, or is `idle`.

### E6.9 Output-change tolerance

`maximum_output_change` is the reviewed tolerance for the output change at the first execution after the transfer. A reviewed value replaces the allowance of A6.4. Studio computes or reviews the value. The runtime checks against the value it receives. For additional outputs, the tolerance is scaled by the ratio of the output spans.

### E6.10 Cancel

Access: Engineer. Request: `loop_id`, `transaction_id` and `expected_program_revision`, all strings. Response: `status`, `confirmed`, and `reason` when it applies. A cancel of an unknown identifier also returns `transaction_id`, `program_revision`, `binding_revision` and `before_dispatch`.

- A cancel for an identifier the runtime does not hold is answered `cancelled` with `confirmed` true and `before_dispatch` true. The identifier is then fenced (E0.2).
- An uncommitted transaction becomes `cancelled`, with the reason `cancelled before task-boundary commit`, and nothing is written.
- A committed transaction returns `pending` with `confirmed` false and the reason `transfer already committed; await its actual outcome`. The executed Structured Text is not undone.
- A terminal transaction returns its outcome.
- The `expected_program_revision` MUST match the revision of the last status or apply of the loop. Otherwise the cancel is refused.
- A cancel whose identifier does not match the record is refused with `invalid_transfer_request`. It never affects another transaction.
- A cancel grants no mode or state change, and it bypasses no check of E6.5 or E6.6.

### E6.11 Recovery

- Recovery is a new apply to the Manual or tracking state. It is reviewed separately and uses fresh evidence (E6.5).
- The runtime never sets the output to zero and never restores an obsolete state on its own.
- Closing Studio does not interrupt a queued transaction. The runtime owns it until it reaches a terminal state.

## E7. Parameter transactions and control.pid

### E7.1 Binding

The binding is `{loop_id, task_name, parameters, mode}`.

- `loop_id`: string, not blank.
- `task_name`: string. `$background` selects unscheduled programs.
- `parameters`: 1 to 64 members. Each is `{symbol, iec_type, unit, convention, minimum, maximum}`. `iec_type` is REAL or LREAL. `unit` and `convention` are not blank. `minimum` and `maximum` are finite, and `minimum` does not exceed `maximum`. Unknown fields are refused.
- `mode`: `{symbol, manual_value}`. `symbol` is a BOOL variable, and `manual_value` is a boolean.

Rules:

- Parameters are scalar members of program or function block instances. Aliases, addressed members and shared global members are refused.
- Every path belongs to a program owned exclusively by `task_name` (E2.3).
- Symbols are unique without regard to case, and the mode symbol is not a parameter.
- The binding revision covers the whole binding, including units, conventions and bounds.

### E7.2 Status

Access: Viewer. Request: `binding`. Response:

- `program_revision`, `binding_revision`, `active_revision` (a hash of the current values) and `values` (the actual values, read at the last completed scan).
- `status`: `idle`, `pending`, `applied` or `rejected`.
- `reason`: text or null.
- `previous_revision` and `previous_values`: the retained previous values, or null.
- `transaction_id`: the identifier of the last accepted apply or rollback, or null.
- `supports_request_id`: true.

Status MUST NOT claim that a queued change is applied. It MUST NOT change the record.

### E7.3 Apply

Access: Engineer. Request fields:

- `binding`, `transaction_id` (optional, 1 to 128 bytes, not used before in this epoch), `expected_program_revision`, `expected_binding_revision` and `expected_active_revision`, all strings;
- `expected_values` and `values`, each an object with exactly the declared symbols and number values;
- `source`: `simulation` or `physical`;
- `physical_approved`: boolean, required. It MUST be true for a `physical` source (E10.2).

Unknown fields are refused with `invalid_parameter_request`. The checks run in this order.

1. The source is `simulation` or `physical`.
2. A `physical` source needs `physical_approved` (`physical review required`).
3. The binding is valid (E7.1).
4. The `transaction_id` is valid, not used before, and the epoch has room (1,024 per epoch).
5. No parameter or transfer transaction is pending, and no variable or I/O is forced.
6. The current values are read.
7. Manual is observed now (`confirmed Manual mode required`). Custom Auto adoption is not supported.
8. The expected revisions and the expected values match the current state (`stale program, binding, or expected values`).
9. A status was read before the apply, and the binding has not changed since that read.
10. `values` has exactly the declared symbols. Every value is finite, within its bounds, and exactly representable in its IEC type. A REAL value is explicitly rounded to single precision.

An accepted apply returns `status` set to `pending` and `confirmed` set to false. A request for another task remains pending until that task runs.

### E7.4 Boundary and confirmation

At the owning task's boundary, before the task executes, the runtime rechecks the program identity, the binding, the expected values, ownership, declared types, bounds, Manual mode, the absence of forcing, the I/O authority and the approval (E10.2). A failed check commits nothing and records `rejected` with its reason.

If every check passes, the whole set is committed in one step before the owning task executes. A competing pending set is refused at acceptance.

After the owning task executes, the runtime reads every parameter.

- If every value equals the value written, the transaction is `applied`.
- If the program changed a written parameter, the transaction is `rejected`. Every parameter that still holds the value written by Studio returns to its previous value in the same step. The reason names the parameters that the program changed, in the form `controller changed parameter values before confirmation: <symbols>`.

### E7.5 Rollback

Access: Engineer. The request has the fields of apply except `values`, and it adds `previous_revision`. `previous_revision` MUST equal the hash of the retained previous values. Rollback restores the retained previous values through the same boundary checks and the same confirmation (E7.4).

- Rollback is refused with `rollback unavailable` when no previous values are retained.
- Rollback is refused with `stale rollback revision` unless every member still holds the value of the active revision.
- On a physical target, rollback needs `physical_approved` (E10.2).

### E7.6 Cancel

Access: Engineer. Request: `loop_id`, `transaction_id`, `expected_program_revision` and `expected_binding_revision`, all strings. Cancel does not pass the experiment interlock.

- A cancel for an identifier that the runtime does not hold fences it. Its response is `{transaction_id, program_revision, binding_revision, confirmed: true, before_dispatch: true}`. A later apply with that identifier is refused.
- A cancel of a queued transaction rejects it with the reason `parameter operation cancelled before task-boundary commit`. Nothing is written.
- A committed transaction is not undone. Its response is `{confirmed: false, reason: "parameter commit requires post-ST observation"}`. Its outcome is read through status. A restoration needs a separate rollback.
- A cancel whose binding or epoch does not match is refused.

### E7.7 control.pid

The `control.pid.*` operations address one Control.PID instance by its `instance_path`. A PID write changes the `ParametersValue` of that instance.

- `control.pid.status` (Viewer). Request: `instance_path`. Response: `active_revision`, `parameters` (an object of the LREAL fields as numbers and the enumeration fields as variant names), `pending_revision` (or null), `confirmed`, and `reason` (why the last write did not take effect, or null). Status only reads: it never clears a pending write or a rollback record.
- `control.pid.apply` (Engineer). Request: `instance_path`, `source` (`simulation` or `physical`), `expected_revision`, `parameters` (an object that may name a subset of the parameters) and `physical_approved` (boolean). Every field is required, and unknown fields are refused. The checks, in order: the source; the approval when the source is `physical` (`physical_approval_required`); the controller record (`controller_unavailable`); no pending write (`controller_write_pending`); the expected revision (`stale_controller_revision`); each name is a Control.PID parameter; each LREAL is finite; AntiWindup is `BackCalculation` or `Combined`; no experiment runs or restores (`experiment_active`). Response: `previous_revision` and `pending_revision`.
- `control.pid.rollback` (Engineer). Request: `instance_path`, `expected_active_revision`, `previous_revision` and `physical_approved` (boolean, optional; required on a physical target, `physical_approval_required`). The checks: the controller is available (`controller_unavailable`); a record exists and a previous revision is retained (`rollback_unavailable`); no write is pending (`controller_write_pending`); the active revision matches (`stale_controller_revision`); the previous revision matches (`rollback_revision_mismatch`); no experiment runs or restores. Response: `pending_revision`.
- No `control.pid.cancel` operation exists.

A `control.pid.*` write is a parameter change. It MUST be committed at the owning task's boundary and confirmed after that task executes (A5.2, A6.8). It MUST follow the checks of E7.3 to E7.5 and of E10: the mode, forcing, the I/O authority, the approval and one live write at a time (A6.5, A6.6, A2.4, A2.5). A bounds check of each value MUST apply. When the program changes a written value, the rule of E7.4 MUST apply, with its rejection and its restoration (A6.8). The PID adapter of `control.parameters.*` MAY implement `control.pid.*`.

### E7.8 Unsupported forms

- Integer and enumeration gains are refused. The only enumeration that `control.pid` accepts is AntiWindup.
- A parameter whose type is not REAL or LREAL is refused with `invalid_parameter_request`.
- A parameter in Auto is refused, because apply needs Manual (E7.3).
- Aliases and shared global ownership are refused with `invalid_parameter_request`.
- A candidate value maps to a parameter only through the mapping that the loop definition declares. A missing or ambiguous mapping disables adoption (Part B).

### E7.9 Authority

- Numerical results, validation labels and saved records never authorize a write. A write needs a fresh review of the live runtime's current revisions and values (E11.8).
- These operations do not establish physical-device qualification (E10.5).

## E8. Isolated simulations and searches

### E8.1 Admission

An isolated simulation runs the project's actual Structured Text against the configured simulated plant or the selected model (A2.6). It never writes to a target.

- The run executes on the target's host only when the host is simulated. The host is simulated when its simulation is active and `simulation_only_io` is true. The runtime does not compare a program revision for this check.
- A host that is not simulated reports the run as not ready. A start on it is refused. The text is `Isolated simulations do not run on a physical target's host; use a local analysis runtime.`
- When the target is physical, the run MUST execute on a local analysis runtime loaded from the workspace project. Its program revision MUST equal the target's (A2.6).- Isolated simulations run only on Linux. Elsewhere the status reports the run as not ready, with the reason `bounded search requires a native process-memory-limit adapter; currently Linux only`.
- An isolated run has no connection to any runtime. It receives no physical setting, no credential and no physical driver. Its environment is cleared. Live process values of the simulated target MAY be carried into it as values before its first scan (E8.3).
- Admission never depends on a debug session.

### E8.2 Project readiness

`control.simulation.run.status` and `control.search.status` take `{}`. A ready answer has `ready` set to true, `project_revision`, `source_basis` set to `loaded-runtime-sources`, and `execution` set to `isolated-simulation`. A search answer also has `limitations`. A run answer has `supports_request_id`. A not-ready answer has `ready` set to false and a `reason`.

- The snapshot is the source that the runtime has loaded, with at most 64 source files and 1,048,576 bytes of source text in total.
- The simulation configuration is at most 262,144 bytes. It is read from the project each time, so a change takes effect without a reload.
- `project_revision` is a hash of the canonical project path, the sources and the simulation configuration.

### E8.3 Shared execution rules

Test runs (E8.4) and searches (E8.5) share these rules.

- Each run starts at simulation time zero, with the configured plant and freshly compiled Structured Text. The runtime infers no equilibrium and runs no hidden preparation cycle.
- `initial_values` is an object `{symbol: number}`, applied before the first scan. The type comes from the variable.
- The parameter values of a run (a test run's `parameter_overrides`, E8.4; a search trial's `parameters`, E8.5) are written before the first scan. A program MAY set such a parameter during its first execution, its startup, as a program that designs or loads its own record at startup does. Its first execution is the first scan in which its task runs: a periodic task first runs one interval after the start, and a program without a task runs in every scan. The runtime then writes the run's value again at the task boundary after that scan, as Apply writes a complete record at a task boundary of a running program (E7), and from that boundary the value MUST hold to the end of the run. This adds no preparation cycle: the run's scans are those of its grid from time zero. A program that changes such a parameter after its first execution would not keep an applied candidate either, and the run fails with the reason `ST changed controller parameter <symbol>; candidate not adopted`.
- `events` is a list of `{at_ms, symbol, value}`. Events are ordered, unique by time and symbol, scan-aligned (each `at_ms` is a multiple of `step_ms`), and no later than `duration_ms`. An event applies at its scan.
- `observations` is a list of `{symbol, unit}`, with 1 to 64 entries. Symbols are unique. Each observed channel counts toward the limit of 64.
- A write targets a writable, scalar member. A duplicate or ambiguous write is refused.
- A BOOL value of 0 or 1 MAY start a project-owned sequence. An integer process value is coerced with exact and range checks. An integer gain is never written.
- A seed is an integer of at most 9,007,199,254,740,991.
- Each scan has a deadline of 100 ms, or the run's wall budget when that is smaller. A run whose scan misses its deadline fails.
- An unsupported source, configuration or model fails explicitly. The run never substitutes a model and never truncates its time grid.
- Samples lie on the exact simulated grid (A4.6). With `record_every`, samples average as in E4.4. A run keeps at most 100,000 samples (A7).
- The evidence of a completed run is `ST-executed`. This is the wire name of the evidence source Isolated simulation of A9. A9 replaces that name.
- A run's result has the `run` object: `id`, `evidence`, `parameters` (`{symbol: value}`), `conditions`, `channels` (`[{id, unit}]`) and `samples`.
- `conditions` has `initialState`, `events` (`[{atSeconds, channel, value}]`), `durationSeconds`, `sampleTimesSeconds`, `seed`, `modelRevision`, `programRevision`, `parameterSchemaRevision`, and `recordEvery` when it is greater than 1. It has `modelProvenance` and `simulationConfigRevision` when an override is given, and `feedbackRelay` when a relay is given.
- A sample is `{sequence, timeSeconds, values, quality}`. `values` is in channel order. `quality` holds one code per channel, from `good`, `bad`, `forced` and `missing`. It is omitted when every channel is good.
- A result proves no physical-device performance (E10.5).

### E8.4 Test runs

Operations: `control.simulation.run.start` and `control.simulation.run.cancel` (Engineer), and `control.simulation.run.status` (Viewer).

Start request:

- `expected_project_revision`: required. It MUST equal the snapshot revision, or the run is refused with `project changed since review`.
- `request_id`: optional (E0.2). When it is absent, the runtime generates the `job_id`.
- `execution`: required. It MUST be `isolated-simulation`.
- `step_ms`: 1 to 1,000.
- `duration_ms`: greater than 0, at most 86,400,000, and an exact multiple of `step_ms`.
- `seed`: an integer of at most 9,007,199,254,740,991.
- `max_total_cycles`: at most 10,000,000 scans, and at least the number of grid points.
- `wall_budget_ms`: 1 to 600,000 (10 min).
- `record_every`: optional, 1 to 100,000. The default is 1.
- `initial_values`, `events` and `observations`, as E8.3 defines them.
- `parameter_overrides`: optional, default `[]`, at most 64 entries.
- `simulation_override`: optional object `{model_revision, model_provenance, simulation_toml}`. Its fields are limited to 256, 4,096 and 262,144 bytes. `feedback_relay`: optional object, checked against its declared bounds.

A parameter override is `{symbol, iec_type, unit, convention, minimum, maximum, value}`. Its type is REAL or LREAL, or a fixed configuration BOOL. The value lies within the bounds and names a writable, scalar member. A BOOL override has unit `1` and bounds that contain 0 and 1, and its value is 0 or 1. It keeps its value for the whole run, from the task boundary after the program's first execution when the program sets it then (E8.3). Duplicate symbols and conflicts with `initial_values` or `events` are refused. The `parameterSchemaRevision` is a hash of the declarations (symbol, unit, minimum and maximum) in request order.

Success response: `job_id`, `status` set to `queued`, and `project_revision`. A repeated `request_id` with the same inputs returns the same fields.

Status request: `job_id`, `expected_project_revision`, and optionally `include_results`.

Status response: `job_id`, `project_revision`, `status`, `cancel_requested`, `reason`, `source_basis`, `io_provenance` (`isolated-simulation-no-physical-drivers`), `captured_sample_count` (the number of complete sample frames received) and `run` (the result, once the job is terminal). `include_results` set to true before the job is terminal is refused. The status carries no raw data before the job is terminal.

Job states: `queued`, `running`, `completed`, `cancelled` and `failed`.

Cancel request: `job_id` and `expected_project_revision`. It sets the cancel flag while the job is queued or running. The job becomes `cancelled` when its child process has ended. The response is the status body.

Runs that end early:

- A cancelled, timed-out or abnormally ended run keeps the coherent prefix of its complete samples in `run`. Its requested conditions and time grid stay frozen. A partial last sample is dropped, and no row is synthesized for it.
- Missing later observations stay missing. The runtime never synthesizes them.
- A run that fails before its first complete sample has no `run`.
- Cancellation ends in `cancelled`. A wall-budget expiry, a missed scan deadline or an abnormal exit ends in `failed`.
- A prefix is isolated-simulation evidence, never completed verification. A run that did not complete never reads as validated.
- A test run has no ranking and no score.

Job identity is scoped by purpose. A search request cannot inspect or cancel a test job, and a test request cannot inspect or cancel a search job.

### E8.5 Searches

Operations: `control.search.start` and `control.search.cancel` (Engineer), and `control.search.status` (Viewer).

Start request:

- `expected_project_revision` (required) and `execution` (required, `isolated-simulation`).
- `grid`: 1 to 4 axes. Each axis is `{symbol, unit, minimum, maximum, values}`, with 1 to 16 distinct finite values within the bounds. An axis names a writable, scalar REAL or LREAL member.
- `trial_count`: exactly the product of the value counts. The product is at most 64.
- `step_ms` (1 to 1,000), `duration_ms`, `seed`, `max_total_cycles`, `initial_values`, `events` and `observations`, as E8.3 and E8.4 define them. A search has no `record_every`; it records every grid point.
- `wall_budget_ms`: at most 3,600,000 (60 min) for the job. `per_trial_wall_ms`: at most 600,000 (10 min), and at most `wall_budget_ms`.
- `requirements`: at most 64 entries of `{id, symbol, minimum, maximum, start_ms, end_ms}`. A window has a positive width, lies within the duration, and aligns to samples.
- `objective`: `{reference_symbol, feedback_symbol, error_unit}`. The objective channels are observed, they are not swept parameters, and the unit of each channel MUST match `error_unit`.

A search has no client identifier. The runtime generates its `job_id`, of the form `search-<pid>-<nanos>-<n>`. Success response: `job_id`, `status` set to `queued`, `project_revision` and `trial_count`.

Rules:

- A swept symbol MUST NOT appear in `initial_values` or `events`.
- The initial values, the plant's initial state, the events, the time points and the seed are identical for every trial.
- Each trial runs in a fresh isolated process, from the same source snapshot, with a fresh simulation controller.
- A failed trial is recorded with its reason. The other trials continue.
- The error of a trial is `reference - feedback`. Its score is the trapezoidal integral of the squared error over the sampled grid. Its unit is `error_unit` squared times seconds.
- Requirements are checked before ranking. A missing or bad value never qualifies a trial. A requirement window is inclusive.
- Ranking covers qualified trials only. Trials are ordered by score, and ties are broken by the trial's index in the Cartesian order of the grid, with the first axis varying slowest.
- A trial's `status` is `qualified`, `rejected` or `failed`.

Status request: `job_id`, `expected_project_revision`, and optionally `include_results`.

Status response: `job_id`, `project_revision`, `status`, `cancel_requested`, `trial_count`, `trials`, `unattempted_trials` (the indexes not yet attempted), `ranking` (the indexes in rank order, when the job is completed), `reason`, `source_basis` and `io_provenance`.

- With `include_results` set to false, each trial gives `index`, `parameters`, `status`, `reason`, `score`, `score_unit` and `violations`, and no raw runs.
- With `include_results` set to true, each trial gives its full result, including its run. This is allowed only after a terminal state.
- Omitting the option gives the full trial objects.

Each trial has `index`, `parameters`, `status`, `violations`, `score`, `score_unit` (`<error_unit>^2 s`), `integration` (`trapezoidal`), `io_provenance`, `run`, `worker_memory_limit_bytes` and `runtime_version`.

Cancel: sets the cancel flag while the job is queued or running. The job becomes `cancelled` when its child processes have ended. Trials not yet attempted stay unattempted. A trial in progress at cancellation is recorded as `failed` with the reason `search cancelled`.

Limits of the job. The retained output of a job, of all its trials together, has a limit of 60,331,616 bytes. When the output would exceed it, the trial whose evidence cannot be kept is recorded as failed with the reason `total retained output budget exhausted; trial evidence omitted`. The job then ends `failed` with the reason `total retained output budget exhausted; no ranking is available`. A job never truncates samples and never ranks an incomplete job.

Concurrency: the runtime runs one isolated worker at a time. A second start is refused with `a search is already running` or `another isolated search worker is running`. A new job MAY start only after the previous job is terminal, and it replaces that job's results. The runtime keeps the results of one job.

A search result is never applied by the runtime. Choosing a qualifying trial MAY fill a parameter review, but only when every parameter that the review needs is explicit in that trial's grid or initial values. Writing the value is a separate parameter transaction (E7).

A runtime restart ends its jobs. A later status for such a job is refused with `search job not found`.

## E9. Live-write interlocks, reloads and restarts

### E9.1 One live write at a time

A runtime runs at most one experiment, transfer or parameter transaction at a time (A6.6). An experiment runs from its acceptance until its terminal state. A transaction is pending from its acceptance until its terminal outcome.

- An experiment start is refused with `transaction_pending` while a transfer or parameter transaction is pending. An experiment start is refused with `experiment_active` while another experiment runs.
- A transfer apply, a parameter apply or rollback, and a `control.pid.*` write are refused with `experiment_active` while an experiment runs or restores.
- A transfer apply is refused with the text `a control transaction is pending` while a transfer or parameter transaction is pending. A parameter apply is refused with the text `a parameter transaction is pending`.
- A refusal of this kind MUST name the operation that is running, by its kind and its identifier (A6.6).
- A `control.pid.*` write is one live write. It MUST be refused while a transfer or parameter transaction is pending, and a transfer or parameter apply MUST be refused while a PID write is pending (A6.6).
- An experiment start MUST NOT replace an experiment that is still restoring (A6.6).

Recordings, inspections, isolated runs and searches are never refused for these reasons. Stop, abort and cancel requests are never refused for these reasons either.

### E9.2 Faults and interlocks

The protocol defines no interlock model beyond the checks of E5.3, E6.5, E6.6 and E7.3.

### E9.3 Reload and restart

A program reload or a runtime restart (A6.7) does the following at the next scan; a status request, which only reads, shows the outcome from that scan on.

- Every experiment, transfer and parameter transaction of the old program stops. The runtime MUST NOT write a symbol that was resolved against the old program, and it MUST drop queued writes that were resolved against it (A6.7).
- An experiment whose target is a symbol ends `failed` with the stop reason `program_reloaded` or `runtime_restarted`. Its restoration is not written. An experiment whose target is an address ends with the same stop reason, and its restoration and abort assertion are written (E5.4).
- A transfer that was pending, committed or not, ends `failed` with the reason `runtime interrupted before transfer confirmation`. Its values are not restored, because they belong to the old program. (A fault of the owning execution is different: it reverts the written members in the same scan, A6.3.)
- A parameter transaction ends as rejected, with the reason. Its record is kept, so status reports why (A6.7). Retained previous values are dropped, so a rollback never spans a reload or restart.
- A recording ends with the stop reason `runtime_boundary`, and its acquisition is incomplete (E4.5).
- A `control.pid.*` write that is queued is dropped. The stopped write MUST report why (A6.7).
- A reload does not end an isolated run or a search. A restart ends them, and a later status of such a job is refused with `search job not found`.
- The session epoch changes, so the program revision changes. Identifiers of the old epoch are released (E0.2).

Studio resolves the new identity before any further live action (E11.5).

### E9.4 Runtime fault or stop

- A runtime fault or a stop of execution ends the same operations as a restart, with the same effects. The session epoch changes.
- A pause ends nothing.
- A committed transfer that has not been read ends `failed`, and its values are not restored.

### E9.5 Identity changes

A change of program revision or session epoch ends every operation bound to the old identity (A2.2). An operation MUST NOT continue on a new identity without a new review (E11.5).

### E9.6 Reporting

A stopped or refused operation reports its stop reason or refusal reason, its last state, and its last applied point or observed value (A11.1). A parameter transaction or a `control.pid.*` write that the runtime stopped MUST report its reason (A6.7).

## E10. Physical targets

### E10.1 Decision

The runtime decides whether a target is simulated or physical from the facts of E1.3 (A2.4). A request's `source` label never changes that decision (A2.4). The runtime applies the decision to every write it receives.

### E10.2 Approval

Every live write on a physical target needs the engineer's approval immediately before it starts (A2.5). The writes are: experiment start for every kind; transfer apply; parameter apply and rollback; `control.pid.apply`; and `control.pid.rollback`. The runtime decides from its own target fact (E1.3, A2.4) whether a write needs approval. A request whose `source` label contradicts that fact is refused with `invalid_target`.

The approval has these forms in the protocol:

- experiment start: the object `physical_approval` with the booleans `user_approved` and `topology_reviewed`, both of which MUST be true;
- transfer apply, parameter apply and rollback, and `control.pid.apply`: the boolean `physical_approved`, which MUST be true;
- `control.pid.rollback`: the boolean `physical_approved`, optional on a simulated target and true on a physical one
  (E7.7).

Rules:

- A physical write MUST be refused unless its approval is present. The refusal is `physical_approval_required` for experiments and `control.pid.apply`, and the text `physical review required` under the family code for transfers and parameters.
- An approval covers one reviewed campaign (A2.5). It MUST NOT authorize any other write.
- Approving an experiment never authorizes an apply (A2.5).
- An approval is not bound to a scan number. Scans may advance between the review and the write (A2.5).
- The runtime checks the presence and the content of an approval. It cannot observe the engineer's decision, which Studio takes (A2.5).
- Stop, abort, cancel, release, recordings and reads need no approval.

### E10.3 Isolated runs

Isolated runs MUST NOT run on the host of a physical target (A2.6). A physical host refuses an isolated run, with the family code and the text of E8.1.

### E10.4 Mixed I/O

A runtime with both simulated and physical I/O MUST be treated as physical (A2.4). A runtime with no simulation enabled MUST be treated as physical, whatever its I/O (A2.4).

### E10.5 Claims

No simulated, isolated or predicted result establishes physical qualification (Part D). Neither does any result of this protocol.

## E11. Client recovery and reattachment

### E11.1 Endpoint

A study's operations use the endpoint they started on. Studio MUST NOT switch the target of a study implicitly (A2.7). A journaled operation keeps the runtime identity that it started on. A reopened study with no connected runtime offers the action that connects one, and it performs no live action (A2.7).

Isolated runs and searches run on the target's host when it is simulated (E8.1). They run on a local analysis runtime only when the target is physical (A2.6, E10.3).

### E11.2 Journal before dispatch

Before Studio dispatches an experiment, recording, transaction or isolated job, it journals the operation's identity (A8.5). The journal MUST NOT hold a credential or a token. For each operation, the journal holds at least:

| Operation | The journal holds |
|---|---|
| Recording start | Project; runtime identity; `loop_id`; `loop_revision`; `recording_id`; the binding list; the source; the program revision, when one was acquired. |
| Experiment | Project; runtime identity; `loop_id`; `loop_revision`; `experiment_id`; `recording_id`; the reviewed plan. |
| Transfer or parameter transaction | Project; runtime identity; `loop_id`; `transaction_id`; the program and binding revisions; the kind of operation. |
| Isolated job | Project; runtime identity; `job_id`; the project revision; the purpose. |

Studio MUST journal the identity of an isolated search before it dispatches the search (A8.5).

### E11.3 Order of dispatch

Studio journals the identity, then dispatches the request. An experiment is dispatched only after the start of its recording is acknowledged. A stop for a recording whose start acknowledgement is pending waits for that acknowledgement, and then stops the same recording.

### E11.4 Unknown outcomes

- After a lost acknowledgement, Studio MUST NOT repeat a start, an apply, a rollback or a job start. It reads the operation by its identifier, through the status operation of that operation (E4.6, E5.6, E6.3, E7.2, E8.4).
- Stop, abort, cancel and release MAY be retried with the same identifier and scope (E0.2).
- A symbolic recording start MAY be attempted at most five times in total. A new attempt is allowed only after the runtime refused the start with `recording_unavailable` and the text `a current coherent inspection of these symbolic bindings is required`. Each attempt follows a fresh inspection pinned to the same program revision and the same bindings. Unknown errors, transport timeouts and lost acknowledgements never authorize an attempt.
- If a recording start is unconfirmed, Studio keeps the identifier and attempts to stop that recording. If the stop also fails, the owner stays unresolved, and the stop MAY be retried for that same owner. No new start replaces this recovery. A commissioning sequence stops before any gain is applied and before Auto is entered.

### E11.5 Reattachment

- After a panel close or an Extension Host restart, Studio MUST reattach to every journaled operation by its identifier, show its state through its status operation, and continue the study (A8.5).
- Closing a panel MUST NOT cancel a running experiment. A long experiment keeps running in the runtime while VS Code is closed (A8.5, A10.4). Closing during Apply's observation follows E11.11.
- If the program revision, the binding revision or the session epoch differs from the journaled one, or if no terminal status is available, the operation is unresolved. Dependent work stays blocked until a fresh review (A2.2).
- Studio MUST discard any reply that carries another session epoch. A new epoch starts a new time origin and a new sample window. Studio never draws a continuous response across it.
- Reopening a loop MUST NOT start a test, a tuning, a validation or a write (A8.6). After a runtime restart, Studio reads the actual controller values before it reports anything as active (A8.6).
- Studio resolves the new identity before any further live action (A6.7).

### E11.6 Owners and terminal outcomes

- A recording owner is cleared by a successful stop that returns the terminal record, or by a start refusal that happened before any buffer was allocated.
- An experiment owner is cleared only after its terminal recording has been retained. Studio labels the result with the stored plan. It reports only what the runtime reported.
- A transaction owner is cleared when its terminal outcome has been retained.
- A job owner is cleared when its terminal result has been retained.
- After an Extension Host restart, a recording owner loads as **Recording recovery unconfirmed**. It disables a new start and offers a stop for the retained identifier only. A terminal record is accepted only when its identity, its loop revision, its binding order and its program revision match the owner. Studio MUST NOT rebuild a recording under a new identifier, and it MUST NOT substitute editor samples.
- After a panel or Extension Host reload, Studio MAY reconstruct the owner of an experiment. It polls the stored `experiment_id`. If the runtime identity cannot be established, the owner stays unconfirmed and blocks any replacement test start.
- Studio MUST retain a recording's samples before it releases the recording (E4.7). It stops the recording after the terminal state, and it releases the recording only after the samples are retained.

### E11.7 Unconfirmed transactions

- A lost acknowledgement is resolved by observing the same transaction. Studio MUST NOT repeat an apply.
- An unconfirmed Manual operation keeps its owner and offers recovery in the current panel. A failed recovery review keeps that route available.
- Dependent commands and editable task inputs stay locked until the operation is reconciled. The lock holds in the extension host as well as in the view.
- An explicitly confirmed replacement-runtime review unlocks the next study. It does not claim that the old operation completed, and it does not replay the old operation's writes.
- A rollback after an unconfirmed operation is always a fresh review (E7.5). Studio never reuses the review of a closed panel, and it never issues a second apply or rollback from that review.
- New work cannot bypass an unresolved application owner. A saved outcome never restores write authority (A8.6).

### E11.8 Reviews and validation

- Every write follows a review of its exact request. A review is consumed once, by the start that it authorizes. An edit, a loop switch, a panel close or a change of identity invalidates it.
- Studio validates a request before it dispatches it, with the rules of this annex: declarations, unknown fields, duplicate symbols, bounds and unsupported types. A request that fails validation is not dispatched.
- Malformed evidence returned by the runtime never authorizes a write and never counts as confirmation. Absent or null optional evidence is unavailable.
- Studio's checks never replace the runtime's checks.

### E11.9 Mode return

When an operation changes the loop's mode, Studio returns the loop to the mode that it found, with a further transfer, when the operation ends. The exception is a mode change that is itself the result of the operation (A6.2). The runtime does not return the mode by itself.

### E11.10 Commissioning sequence

For a proportional controller, Apply runs these runtime operations in order.

1. Verify the matched target-response cases and the Manual-to-Auto initialization case with isolated runs (E8.4, E8.5). The initialization case uses explicit reference and mode events. Each run records the fixed parameters throughout.
2. Read the parameter values, the mode and the reference again. Stop if any of them changed. If the loop is not in Manual, transfer it to Manual first.
3. Stage the candidate gain in Manual, and confirm it (E7.3, E7.4).
4. Read the transfer status (E6.3). Require the typed bindings, the verified gain and the selected steady operating point.
5. Compute the reference `r = y + u/Kp`, where `y` is the measured output and `u` is the held output. Round `r` to the reference's type, and check its bounds. Add no bias.
6. Transfer the tracking state and the mode with `manual` false (E6.4, E6.5). Wait for the confirmation `output-image`.
7. Journal the recording, start it (E4.2) and wait for its acknowledgement; journal the transfers confirmed so far (specification 35, B6.3). Then apply the reviewed task reference through the same guarded boundary (E6.5). This is a separate reference change. It is not the Auto entry.
8. Observe with the recording for the declared duration, plus two task periods of process time. Assess the samples against the requirements.
9. On a passing observation, the loop stays in Auto. A rollback is a fresh review. It covers a Manual tracking transfer that holds the freshly observed output, restoration of the previous reference, and restoration of the previous gain set.

The continuity tolerance works as follows. Studio's review proposes `1e-6 × max(1, output maximum − output minimum)` as `maximum_output_change` (E6.9). The engineer MAY choose another finite value from zero to the output span. The reviewed value is kept in the record. It is used for the first-output check of the Auto entry, for the continuity check, and for recovery. Editing it invalidates the review.

Cancel resolves the owned transfer and keeps its real outcome. If Auto was already entered, Studio uses the reviewed Manual tracking recovery. Studio never repeats the Auto entry or the target step.

An interruption or failure keeps its evidence. Studio distinguishes unconfirmed recovery from success.

### E11.11 Closing an unresolved recovery

Closing the editor attempts only a recovery that is already in progress and owned (E11.7), such as a restore the engineer started. The outcome is saved with the loop's study as evidence. It never serves as write approval. Closing MUST NOT abort, cancel or restore runtime-owned work: a running identification experiment, an Apply observation or an isolated job keeps running, and reopening reattaches it (A8.5, A8.6, A10.4, B6.3). A pending Manual application is reconciled when the loop is reopened (E11.5).

## E12. Limits

### E12.1 Part A limits

Every quantity in Part A7 applies in full, in every component. Each component accepts at least the quantity that Part A7 states. A larger request is refused with the refusal code of its family, and the text names the limit.

### E12.2 Protocol limits

| Quantity | Limit |
|---|---|
| Identifier length: `recording_id`, `transaction_id` of a transfer (apply), `transaction_id` of a parameter set, `request_id`, and signal binding `id` | 1 to 128 bytes. A transfer apply counts characters. |
| Loop identifier: `loop_id` of a transfer | At most 128 characters. |
| `experiment_id` | Not blank. No maximum is enforced. |
| Transfer loop records held by the runtime | 32. A new record beyond that is refused. |
| Accepted transfer identifiers per session epoch | 1,024. Capacity exhaustion is refused. No evidence is evicted and no replay is accepted. |
| Accepted parameter identifiers per session epoch | 1,024. Capacity exhaustion is refused. |
| Last terminal outcome per transfer loop | One record per loop, readable until replaced. |
| Last terminal outcome of an experiment | One record in total. A new start replaces it. |
| Transfer states; transfer guards | 1 to 64 each. |
| Transfer additional modes | 0 to 8. |
| Transfer additional outputs | 0 to 15. |
| Tracking references, commands and measurements of `declared-state-v1` | At most 16 each. |
| Parameters per parameter set | 1 to 64. |
| Trajectory points per experiment | 10,000. |
| Trajectory spacing | At least one period of the experiment's task. |
| Relay hysteresis | Finite and greater than 0. |
| Bindings per recording | 1 to 32. Channels times samples is at most 1,600,000. |
| Samples per recording status response | 2,000. |
| Recording samples per channel | 100,000 (record period per A4.2). |
| Recording duration | 24 h of process time. |
| Recordings held at once | 2. |
| `record_every` | 1 to 100,000. |
| Parameter overrides per test run | 64. |
| Initial values per isolated run | 64. |
| Events per isolated run | 1,024. |
| Observed channels per isolated run | 64. |
| Scan deadline in an isolated run | 100 ms, or the run's wall budget when that is smaller. |
| Address space per isolated worker | 1 GiB. The worker reads the limit back. |
| Isolated workers per runtime | 1. |
| Source snapshot | 64 source files, 1,048,576 bytes of source text, and 262,144 bytes of simulation configuration. |
| Search grid axes; values per axis | 1 to 4 axes; 1 to 16 values per axis. |
| Trials per search | 64. |
| Requirements per search | 64. |
| Search and run step | 1 to 1,000 ms. |
| Isolated run wall time | 10 min per run. |
| Search wall time | 60 min per job. |
| Scans per isolated job | 10,000,000. |
| Channel samples per isolated job | 1,600,000. |
| Retained output of one search or test job, all trials together | 60,331,616 bytes. A job that exceeds it follows E8.5. |

The refusal codes of each limit are those of E0.4. A byte limit is a backstop, and a valid run or search MUST NOT reach it (A7).
