# VS Code Control Studio

Status: the binding product specification for Control Studio, revised 8 October
2026. It replaces the earlier Specification 35 and the eleven companion
documents listed in §0.4. The words MUST, MUST NOT, SHOULD and MAY are
normative.

## 0. Scope and authority

### 0.1 What Control Studio is

Control Studio is truST's general-purpose identification and tuning tool
(purpose contract, part 3). For a control loop in a truST project it identifies
the plant, tunes the controller to the engineer's requirements, validates the
tuned controller by running the project's actual Structured Text, applies the
result to the running controller and can restore the previous settings. It
works on simulated and physical plants, on loops that settle in a fraction of a
second and on processes that take hours, and on single loops, cascades,
interacting loop groups and the other structures of Part C. It never
hard-codes a domain, plant or lesson: names, units and ranges come from the
project.

### 0.1a Where Control Studio's logic runs (decided 9 October 2026)

All of Control Studio's logic runs in Rust (AGENTS.md, "Hard rule: all logic is in Rust"). That
covers the calculations (identification, tuning, analysis, evaluation), the workflow rules (when
an action is allowed, what Apply and Restore do), the runtime clients and the study storage.
- The control-system mathematics is in the domain-neutral library `trust-control-math`: linear
  models, discretization, simulation, sampling and step-response identification. It knows no
  Control Studio concept and no user-facing text; product limits are its parameters.
- Control Studio's calculations and rules are in the pure crate `trust-control`, which uses the
  library and does no I/O.
- The orchestration is in the crate `trust-control-studio`. It runs each stage against the runtime,
  stores the study and keeps the operation journal. Its binary, also named `trust-control-studio`,
  is Control Studio's engine. It has two front ends over the same functions, and neither holds a
  rule of its own:
  - `trust-control-studio serve`: JSON-RPC 2.0 on standard input and output, each message framed
    with a `Content-Length` header as the language server protocol frames its messages. The VS Code
    extension starts one engine per window and talks to it there.
  - the command line of 0.1b, for people, scripts and CI.
- The runtime's control endpoint (E0.1) is reached through the shared client crate
  `trust-runtime-client`, which the language server's inline values also use.
- The language server has no part in Control Studio: it serves the Structured Text language, and
  sharing its process would tie Control Studio's operations to its restarts and its load.
- The engine's methods, each arriving with its stage: `controlStudio/open`, `controlStudio/status`,
  `controlStudio/cancel`, `controlStudio/page`; `controlStudio/identify`, `controlStudio/tune`,
  `controlStudio/preview`, `controlStudio/validate`, `controlStudio/apply`, `controlStudio/restore`,
  `controlStudio/keep`; `controlStudio/setupDiscover`, `controlStudio/setupSave`. `controlStudio/page`
  records the page last viewed (A8.1), which `controlStudio/open` returns so reopening shows it
  (A8.6); it changes nothing else and starts nothing. `controlStudio/preview` answers at once with
  the predicted response, margins and outcome of entered controller settings on the study's model,
  as Tune with the manual method would finish, and stores nothing (B4.5).
- A long method answers at once with an operation identifier. Its progress and result follow as
  the notification `controlStudio/update`. `controlStudio/status` answers for an operation, and
  `controlStudio/cancel` cancels it as Cancel does.
- A method's JSON-RPC result is `{"result": …}` or `{"failure": …}`; a failure carries its cause
  and next action (A11). A malformed message or an unknown method is a JSON-RPC error.
- The engine ends when its client closes standard input. Ending it never stops a runtime-owned
  experiment or recording: the runtime keeps them, and the next engine reattaches through the
  journal (A8.5, A8.6).
- The VS Code extension only presents: it starts the engine, shows the pages, forwards the
  engineer's actions as these methods, and renders their answers. Its message types are generated
  from the Rust types. It passes the project folder and the selected runtime with its endpoint and
  control token. The engine keeps the operation journals in the folder of A8.3, which the command
  line uses too, so either front end reattaches an operation the other started. The engine never
  journals the token.
- The runtime keeps the real-time parts it owns today: recordings, experiments, transfers,
  parameter transactions and isolated runs (the runtime protocol annex).

The move from the TypeScript implementation follows
`docs/internal/testing/checklists/rust-logic-migration.md`. Until the switch (its stage V6), the
TypeScript engine still serves the extension.

### 0.1b Command line (decided 9 October 2026)

`trust-control-studio` runs Control Studio without VS Code: from a terminal, a script or CI. Each
verb calls the same function as its method in 0.1a, so every rule, limit, refusal and confirmation
is the same: a verb adds none and drops none.

- Verbs, each arriving with its stage:
  - `open`: lists the project's loops, or opens one with `--loop`: its stages and journaled
    operations;
  - `identify` (B3): a live campaign with the loop's method, or `--method <method>`;
    `--recording <id>` fits a kept recording without excitation (B3.6); `--resume` follows a
    journaled campaign to its end (A8.5); `--reset` starts a new working identification study;
  - `cancel`: cancels a journaled operation: it reattaches by the journaled identifier (E11.5),
    ends a running experiment through its restoration path (E5.5) and clears the owner once the
    outcome is kept (E11.6);
  - `tune` (B4): designs a candidate for the loop's controller from the study's model, with
    Automatic or `--method simc|amigo|relay|manual` (`--choice faster|balanced|more-robust` for a
    formula with a response choice, `--structure p|pi|pd|pid` to fix the structure,
    `--setting <symbol>=<value>` for each entered setting of the manual method), against the
    requirements the study holds and the criteria Automatic fills (B4.3); it shows the candidate,
    its predicted response and its margins, keeps the candidate in the study, and never writes to
    the controller (B4.1);
  - `validate` (B5): runs the independent model check when the model is provisional, then the
    actual-ST validation of the current and the candidate settings in isolated simulations, and
    prints each requirement's result and whether Apply is available; it never writes to the
    controller; later stages add `apply`, `restore`, `keep` and `setup`.
- Common arguments: `--project <folder>` (default: the current folder), `--loop <id>`,
  `--journal-dir <folder>` (default: truST's folder in the user's local state folder, by the
  platform's convention, where the extension's engine keeps them too, A8.3), `--json`.
- The runtime is the project's configured runtime (its `runtime.toml`), or `--endpoint
  tcp://host:port | unix:///path`. Its control token comes from the environment variable
  `TRUST_CONTROL_TOKEN` or from `--token-file <file>`, never from an argument: arguments are
  visible to other users of the computer.
- Output: each progress update is one line on standard error. The result goes to standard output,
  as readable text, or with `--json` as the same object the method answers (`{"result": …}` or
  `{"failure": …}`).
- Exit status: 0 when the verb finished with its result; 1 when it was refused or failed, with the
  failure's message and next action printed; 2 for a usage error (an unknown verb, a missing or
  invalid argument); 130 when an interruption ended it.
- Confirmation: a verb that changes a plant's controller (`apply`, `restore`, `keep`) needs, given
  explicitly, the reviewed identity that the panel's single confirmation needs (B6); the verb that
  produced the review prints it. No option skips a check or an approval. A physical target needs
  the same reviewed approval as in the panel (A2.5); until that approval exists, live physical
  identification is refused here as it is in the panel.
- Interruption: the first Ctrl-C cancels the operation as Cancel does. A live experiment is ended
  through its restoration path, and the verb waits for the runtime to confirm the restoration. A
  second Ctrl-C leaves the experiment, its recording and the journal to the runtime, as closing VS
  Code does (A8.6); `identify --resume` then follows it to its end.

### 0.2 Documents and what each owns

| Document | Owns |
|---|---|
| [Purpose contract](control-platform-purpose.md) | Purpose and acceptance claims of the control platform. |
| [Complete workflow specification](control-studio-complete-workflow-specification.md) | The user-dictated stage flow: automatic default, explicit stage actions, compact editors, save and reopen behaviour, and the engineering-method flowcharts. |
| [Identify](control-studio-identify-page.md), [Tune](control-studio-tune-page.md), [Validate](control-studio-validate-page.md) page specifications | Page layout and visible controls. They are the only layout authorities. |
| This specification | Foundations (Part A), stage behaviour (Part B), controller structures (Part C) and acceptance (Part D). |
| [Runtime protocol annex](control-studio-runtime-protocol.md) | The runtime operations Control Studio uses: requests, responses, checks and limits. |
| [Numerical annex](control-studio-numerics.md) | Identification, tuning, analysis and evaluation mathematics. |
| Specifications [32](32-control-pid.md), [33](33-simulation-plant-models.md), [34](34-vscode-plant-simulation-ui.md) | `Control.PID` execution, plant simulation, plant configuration UI. |

When two documents seem to disagree, the purpose contract comes first, then the
user-dictated workflow and page specifications, then this specification, then
the annexes. An annex never relaxes a rule of Part A. A disagreement is a
defect in the documents and MUST be fixed there, not resolved silently in code.

### 0.3 How this specification is organised

- **Part A, Foundations**: the rules everything else depends on. Each is stated
  once, here, and referenced elsewhere.
- **Part B, Workflow**: what each stage does, its inputs, its invariants and its
  failure states.
- **Part C, Controller structures**: one template per supported structure.
- **Part D, Acceptance**: what must be shown before a capability counts as done.

### 0.4 Documents merged into this revision

The runtime protocol annex replaces `control-studio-controller-transfer.md`,
`control-studio-parameter-transactions.md`, `control-studio-symbolic-acquisition.md`,
`control-studio-finite-search.md`, `control-studio-isolated-run.md`,
`control-studio-controller-bindings.md`, `control-studio-experiment-abort.md` and
the protocol parts of `control-studio-auto-commissioning.md`. The numerical
annex replaces `control-studio-general-models.md`, `control-studio-routh-hurwitz.md`
and `control-studio-run-validation.md`. Appendix 1 maps the old section numbers
to the new ones.

## Part A. Foundations

### A1. Terms

- **Loop**: one control structure Control Studio works on, declared by a loop
  definition in the project (§B1).
- **Plant input**: a signal that moves the process, for example heater power or
  a valve position. In Auto the controller writes it.
- **Measured output**: a signal the process produces and the controller
  measures, for example a temperature.
- **Setpoint**: the value the controller drives a measured output to.
- **Mode signal**: the program variable that selects Auto or Manual, together
  with the value that means Manual.
- **Manual output**: the program variable whose value the controller outputs in
  Manual.
- **Task**: the PLC task that executes the loop's controller. Its **task
  period** is its configured interval.
- **Execution**: one run of the task. The runtime starts a task at the first
  resource cycle at or after the moment it is due, so executions are not
  exactly evenly spaced (A4.3).
- **Process time**: the runtime's clock (A3).
- **Target**: the runtime Control Studio is connected to (A2).
- **Study**: everything Control Studio keeps for one loop (A8).
- **Experiment**: a bounded change the runtime applies to the plant to identify
  it or to check a model, ending with automatic restoration.
- **Recording**: the samples the runtime captures for a set of signals.
- **Sample**: the values of the recorded signals at one execution of the loop's
  task, or their means over one record period (A4.2).
- **Provisional model**: a model fitted from Identify's recording. Tune may use
  it. It cannot authorize Apply.
- **Qualified model**: a provisional model whose frozen predictions passed the
  independent model check on Validate.
- **Candidate**: controller settings produced by Tune and not yet applied.
- **Requirements**: the engineer's declared criteria together with the
  automatic test criteria Tune fills in (B4.3).
- **Isolated simulation**: a run of the project's actual Structured Text in a
  separate supervised process with only simulated I/O (A2.6).
- **Apply**, **Restore previous settings** and **Keep in project**: §B6.

### A2. Target

**A2.1 Connection.** Control Studio uses the shared truST runtime connection,
the same one Devices & Connections and the debugger use, and never opens a
second transport. The connection may be to a runtime VS Code launched (Run or
Debug) or to a runtime started elsewhere and attached through its control
endpoint. Both are equally valid targets; nothing in the workflow depends on
how the runtime was started.

**A2.2 Identity.** A target is identified by its control endpoint, the
runtime's session epoch and the loaded program revision. A VS Code debug
session is not part of the identity. A new epoch (the runtime restarted) or a
new program revision (the program was reloaded) ends every live operation bound
to the old identity (A6.7). Evidence observed before it remains in the study as
history.

**A2.3 What the runtime reports.** Control Studio takes these facts from the
runtime, never from editor settings:

- whether simulation is enabled, its configured speed and its achieved speed
  (specification 33, §6.1);
- whether all attached I/O is simulated: every attached driver identifies itself
  as simulated and no fieldbus client (ADS, OPC UA) is configured;
- the tasks with their intervals, and the resource cycle interval;
- the session epoch and the program revision.

**A2.4 Simulated or physical.** The target is **simulated** when simulation is
enabled and all attached I/O is simulated. Otherwise it is **physical**. A mix
of simulated and physical I/O is physical. An editor setting, the debug
configuration, the presence of `simulation.toml` or a label in a request never
makes a target simulated. The runtime applies the same rule to every write it
receives (experiment, transfer, parameter change): a request on a physical
target without a physical approval is refused, whatever the request says.

**A2.5 Physical approval.** On a physical target every live write needs the
engineer's explicit approval immediately before it starts. The approval shows
the target, the loop, every signal written with its values and limits, the
duration, the stop conditions and the restoration. One approval covers one
reviewed operation shown in full (for example Identify's experiment with its
mode change and return, or Apply's transfer with its observation); anything
outside it needs a new approval. Approving an experiment never authorizes
Apply. On a simulated target, pressing the action is the confirmation. The
approval is shown in the owning page's action area (§B9), not on a new page.

The runtime decides from its own target fact (A2.4) whether a write needs
approval, never from a label in the request. It refuses every live write on a
physical target (experiment, transfer, parameter change, controller write
and restoration) whose request does not
carry the engineer's approval, with the code `physical_approval_required`.

**A2.6 Isolated simulations.** Validation runs, parameter searches and
predictions of program-owned laws run the project's actual Structured Text in
an isolated simulation against the project's configured simulated plant or the
selected model (§B5). An isolated simulation never writes to the target.

- When the target is simulated, isolated simulations run on the target's host.
- When the target is physical they MUST NOT run on the target's host, where they
  would take processor time from the running control. They run on the
  engineer's workstation in a local analysis runtime loaded from the
  workspace's project, whose program revision MUST equal the target's.

**A2.7 One target per study.** Control Studio never switches its target
implicitly. Opening a loop uses the currently connected runtime. Without one,
live actions explain that a runtime must be connected and offer that action;
analysis of saved evidence remains available.

### A3. Time

**A3.1 Process time** is the runtime's clock: the time the runtime reports with
every scan. In simulation it advances in exact cycle steps at the configured
speed (specification 33, §6.1). A runtime that cannot keep up runs behind real
time, and process time then advances more slowly; it never skips. Without
simulation it follows the wall clock.

**A3.2 Every duration of the workflow is process time**: experiment durations,
baselines, holds, settle detection, recording durations, requirement times such
as a settling time, Apply observation, and the simulated duration of isolated
simulations. Control Studio measures elapsed process time from successive
runtime timestamps, never from its own clock.

**A3.3 Wall time is used only for named supervision limits** (A7): the
connection-loss timeout, the confirmation window of a transfer or parameter
change, the wall budgets and per-scan deadline of isolated simulations, and
request acknowledgement timeouts. A message reporting one of them names it.

**A3.4 Staleness.** A value is stale when no newer scan has been received for
the connection-loss timeout, measured on the receiver's own clock between
receipts. Control Studio never compares its own clock with the runtime's: they
may belong to different machines.

**A3.5 Experiments run on process time.** The runtime evaluates an experiment
after each execution of the loop's task (protocol annex). A trajectory point
whose time has been reached is written for the next execution, so it takes
effect at most one execution after its time; a later write to the same variable
before that execution replaces the earlier one. The recording shows the
execution at which each value took effect, and every fit uses the recorded
values, not the planned times. A relay decides its next level after every
execution from that execution's measurement. An experiment therefore behaves
the same at any time scale and under load.

**A3.6 Progress.** A long operation shows its elapsed and remaining process
time and, when the observed time scale differs from 1 by more than 5 %, that
time scale and the expected finish in wall time.

### A4. Sampling

**A4.1** A loop's sample is taken at the end of each execution of the loop's
task, after the Structured Text has run, and stamped with that execution's
process time. A recording for a loop samples the loop's task. A recording of a
program that runs in every resource cycle samples every cycle.

**A4.2 Record period.** A recording may average several executions into one
sample. The record period is a whole number k ≥ 1 of executions. It follows the
process, not the task: a recording keeps about 200 samples over the expected
settling time of the response it records, and at least 200 samples in all, so
the amount of data does not grow with a slow process or a long test.

- Validate (its model check and its isolated runs) and Apply's observation use
  the requirements' settling time.
- Identify does not know the settling time yet. It uses the expected settling
  time when the study declares one (B3.4 step 2); otherwise it takes a tenth of
  the maximum duration, so a response ten times faster than the maximum still
  gets about 200 samples.
- k is never smaller than the smallest value for which the planned duration
  fits the sample limit of A7.
- A fast process keeps k = 1, one sample per execution, and so does a check of
  every execution, such as the levels and dwell times of an ON/OFF output.

The controller always runs at its task period; only what is recorded is
averaged. A recorded sample holds, for each numeric signal, the mean of the k
values, and for a BOOL signal the last of them. The mean lies between the
smallest and the largest of the k values, so a value that does not change
during the period, such as a held parameter, is recorded exactly. The sample's
time is the mean of the k execution times. Its quality is the worst of the k
qualities: bad (including a missing value), then forced, then good. The last
sample of a stopped recording may average fewer than k executions; it carries
its own time, and the recording stays complete. Saturation and clipping are
found by Control Studio from the recorded values and the loop's declared
limits, not reported by the runtime.

**A4.3 Uneven spacing is normal.** A 100 ms task in a 10 ms resource cycle runs
100 or 110 ms apart. A fit that simulates a model under the recorded plant input
runs on the recorded sample times, holding the input between samples. An
analysis that needs an even grid (frequency estimation, discrete-time models,
the model check) resamples once onto a grid whose step is the median spacing, or
the record period: plant inputs hold their value between samples (zero-order
hold) and measured outputs are interpolated linearly. An interval longer than 1.5
grid steps is a gap: an analysis that needs continuous data refuses the
recording and names the gap's time and length. Times that do not increase are
refused.

**A4.4 Completeness.** A recording is complete when it holds every execution
(or record period) of its interval. A missed execution leaves a gap and makes
the recording Incomplete. An incomplete recording remains evidence; it is not
used where continuous data is required.

**A4.5 Fits use at most 5,000 points per channel.** A longer recording is
block-averaged to that size before fitting (numerical annex).

**A4.6** Isolated simulations sample on their exact simulated grid and need no
resampling.

### A5. Signals

**A5.1** A signal is a typed program symbol: a path through program instances,
function block instances (including members a block inherits) and structure
fields, for example `Main.TIC101.StateValue.ProcessValue`. An I/O address is
optional. A signal with an address MAY be read and written through it, but no
signal needs an address for Control Studio.

**A5.2** Control Studio reads signals at the end of an execution of the loop's
task and writes them at the task boundary before an execution. The runtime
resolves each symbol against the loaded program; after a reload a symbol that
moved reads as missing, never as another variable's value.

**A5.3 The plant input as the plant receives it.** For an addressed controller
output this is the committed output image. For an output without an address it
is the variable's value at the task boundary, as the last execution left it
for the block that owns the I/O.

**A5.4** Control Studio never infers a signal's role, unit or polarity from its
name. Every role is declared in the loop definition (§B1).

### A6. Mode, transfer and live checks

**A6.1 One mode binding per loop**: the mode signal and its Manual value. A
cascade or loop group also lists the mode signals of its further controllers.

**A6.2 Return to the starting mode.** An operation that changes the loop's
mode records the mode it found. When it ends, whether completed, failed or
aborted, it returns the loop to that mode, unless the mode change is the
operation's result: Apply leaves the loop in Auto, and Restore returns the mode
it had before Apply.

**A6.3 Transfers are atomic.** A mode change together with its controller-state
initialization is one transaction at one task boundary. If the check after the
first execution fails, or that execution faults, the runtime returns every
member it wrote to its value from before the transaction, in the same scan, and
does not publish the rejected outputs. The loop keeps the mode it had. Recovery never depends on a
second request from Control Studio.

**A6.4 Live values are checked with bands.** Only values that cannot
legitimately change are compared exactly:

- a value the transaction itself wrote reads back exactly in its IEC type (a
  REAL is compared after rounding to single precision);
- controller parameters and the mode are exact: a program that rewrites them
  owns them, and Control Studio reports that;
- a measurement is never required to stay unchanged; it is checked for
  freshness, quality and range;
- the first controller output after an Auto entry stays within the law's
  response in one execution to measurement noise and to the error it enters
  with. For a PID with gains Kp, Ki and Kd and task period Δt this is
  `Ki·Δt·(|e| + n) + 2·|Kp|·n + 4·|Kd|·n/Δt`, where e is the error at entry and
  n is the measured output's noise as single executions see it: three standard
  deviations of the measured output about its mean over the steady baseline
  (B3.4 step 2) of the recording the model was fitted from, closed-loop or
  open-loop, times √k when that recording averaged k executions per sample
  (A4.2). n is zero without a baseline. Other laws use the allowance of their
  Part C template. A reviewed tolerance replaces the allowance. Validate's
  initialization case, Apply's Auto entry and the return to Auto after a failed
  Apply use the same allowance;
- a state declared as bounded, for example a manual output that the program
  copies from the output, stays within its declared bounds.

This rule replaces case-by-case exemptions.

**A6.5 Experiments check the mode themselves.** An experiment that writes the
plant input or the manual output runs only while the loop is in Manual; a
setpoint experiment runs only in Auto. The runtime reads the loop's mode signal
when the experiment starts and after every execution. If the mode leaves the
required one, the runtime stops the experiment and restores.

**A6.6 One live write at a time.** A runtime runs at most one experiment,
transfer or parameter transaction at a time. A request for another is refused
and names the operation that is running.

**A6.7 Reload and restart.** A program reload or runtime restart stops every
experiment and transaction bound to the old program at once. The runtime never
writes a symbol resolved against the old program, and drops queued writes
resolved against it. The stopped operation reports why. Control Studio
resolves the new identity (A2.2) before any further live action.

**A6.8 Parameter changes** are written together at the task boundary and
confirmed after the task has executed. If the program changed a written
parameter, every parameter that still holds Control Studio's value returns to
its previous value in the same step, and the change is reported as rejected,
naming the parameters the program changed.

### A7. Limits

Every limit is enforced in one place in each component and quoted from there
in messages. No component enforces a lower limit than this table. Sizes in bytes
are not limits of their own: data travels in a compact form (channel names once,
values as arrays, each sample once), so its size follows from the sample and
channel limits below, and any byte bound in code is a safety backstop that a
valid recording or run never reaches.

| Quantity | Limit |
|---|---|
| Experiment duration, with baseline, hold, restoration and settling | 1 s to 24 h process time |
| Recording duration | 24 h process time |
| Samples per recorded channel | 100,000 (record period per A4.2) |
| Channels per recording | 32 |
| Channels × samples per recording | 1,600,000 |
| Recordings one runtime holds at a time | 2 |
| Points per fit and channel | 5,000 (block-averaged per A4.5) |
| Isolated simulation, simulated duration | 24 h |
| Isolated simulation, samples per run | 100,000 (record period per A4.2) |
| Isolated simulation, observed channels | 64 |
| Isolated simulation, channels × samples per job (a run, or a search's trials together) | 1,600,000 |
| Isolated simulation, scans per job | 10,000,000 |
| Isolated simulation, wall time per run | 10 min |
| Isolated simulation, wall time per scan | 100 ms |
| Parameter search, trials per job | 64 |
| Parameter search, wall time per job | 60 min |
| Apply observation | 24 h process time |
| Connection-loss timeout (no newer scan received) | 5 s wall time, or 3 intervals between the samples followed when longer: the record period of a followed recording (A4.2), otherwise the resource cycle |
| Confirmation of a transfer or parameter change | 10 s wall time, or 3 executions of the loop's task plus the connection-loss timeout when longer |
| Restoration confirmed by | the 3rd execution of the loop's task after the restoring write |
| Study storage per loop, recordings included | 256 MiB |

### A8. Studies and persistence

**A8.1 Contents.** A study holds, for one loop: the selected signals and
channel mapping, limits, requirements, methods and pins, every recording
(immutable once ended), the provisional and qualified models, candidates,
validation results, application and restoration receipts, and the page last
viewed.

**A8.2 Durable.** A study survives closing the panel, restarting VS Code and
restarting the runtime. It is keyed by the project, the loop id and the loop
definition's binding revision; never by a debug session, runtime epoch or
panel. Editing the loop definition starts a new study for the new binding
revision and keeps the old one readable as history; requirements and limits
whose signals still exist are carried over.

**A8.3 Location.** Studies are stored in the project, in
`control-studio/<loop id>/`: one versioned study document and one file per
recording. The operation journals (A8.5) stay outside the project, one file per
loop, in truST's folder in the user's local state folder (§0.1b); the engine
writes them for both of its front ends (§0.1a). Studies travel with the project,
and large recordings stay out of VS Code's state. A study file of an earlier format is not read: it moves to
`history/`, and a new study of the loop starts.

**A8.4 Invalidation.** A change invalidates exactly the results that depend on
it:

- changed signals or limits: the provisional model is no longer current when its
  signals changed; recordings are never deleted;
- a new or replaced model: candidates and validations based on the old model;
- changed requirements: validations;
- a changed candidate: its validation.

Invalidated results remain in the study as history and cannot authorize Apply.
**Reset** on Identify starts a new working study and keeps the previous one as
history.

**A8.5 Runtime operations are owned by the runtime.** Before dispatching an
experiment, recording, transaction or isolated job, Control Studio journals its
identity. After a restart of VS Code it reattaches to that identity, shows its
state and continues the study. It never replays a request whose outcome is
unknown. A long experiment keeps running in the runtime while VS Code is
closed; reopening shows its progress and, once it has ended, its result.

**A8.6 Reopening** restores the study and its page without starting any test,
tuning, validation or write. It reattaches the journaled operations (A8.5) and
completes them as they would have completed, including returning a mode that an
interrupted identification changed (A6.2); that completion is part of the
journaled operation, not a new one. Closing the panel releases only its client,
as closing VS Code does (A10.4): the runtime keeps the operation, and reopening
reattaches to it at once instead of waiting for it to end. After a runtime
restart Control Studio reads the actual controller values before reporting
anything as active; an old application receipt is not evidence of the settings
now active.

### A9. Evidence sources

Every recording, sample and result carries one of these sources:

- **Simulated plant**: observed on a simulated target;
- **Physical plant**: observed on a physical target;
- **Isolated simulation**: the project's actual Structured Text in an isolated
  simulation, naming the plant it ran against (the configured simulated plant or
  the selected model);
- **Model prediction**: calculated from a model, with no Structured Text executed;
- **Opened recording**: read from a file; it keeps its original source and is
  marked as opened.

Simulated evidence is never presented as physical, and a prediction never as an
observation. These five replace the earlier vocabularies ("Simulation",
"Physical plant", "Recorded data", "Offline model"; "model-predicted",
"ST-executed", "physical-recorded"); where the runtime protocol still carries
an older code, the protocol annex states its mapping. Control Studio labels a
live recording from the target fact (A2.4) read when the recording starts; a
target that changes kind during a recording ends it.

### A10. Long processes

**A10.1** A process is long when its response takes minutes to hours to settle,
for example a large tank, a building or a furnace. Control Studio MUST work for
settling times from under a second to many hours within the limits of A7.

**A10.2 Experiments follow the process.** A step is held until the response has
settled or the maximum duration is reached, not for a fixed fraction of the
duration (B3.4).

**A10.3 A response that has not fully settled is still usable.** The fit
estimates gain and dynamics together from the response so far (numerical annex)
and reports what fraction of its fitted final change the response reached.
Identify accepts the fit when it meets its error limit and the response reached
at least 63 % of the fitted final change. Otherwise it reports the fraction
reached and the hold the fitted time constant suggests.

**A10.4 Long operations survive VS Code.** Experiments, recordings and Apply
observations run in the runtime and are reattached on reopening (A8.5).

**A10.5 Isolated simulations run as fast as the computer allows.** Their
simulated duration is not bound to real time.

**A10.6 Closed-loop identification.** For a loop found in Auto with a declared
setpoint, Automatic identifies with a setpoint step, so the process stays under
control (B3.3). For a slow process this is usually faster than waiting for an
open-loop response.

**A10.7 VS Code stays responsive.** However long the process or the test,
Control Studio never holds VS Code's extension host for more than one second at
a time. Fits and searches run outside it. A recorded result is checked once,
when it is first decoded, and is then shared unchanged, so snapshots and saves
handle only what changed. The amount of recorded data follows A4.2.

### A11. Errors and next actions

**A11.1** Every failure shown to the engineer says in plain words what failed,
why, with the values involved (signal, measured value, limit, time), and the
next action that can fix it. A lower layer's message is kept, never replaced by
a generic text; a fit that tries several model forms reports each form's
reason.

**A11.2 Next actions are typed**, never derived from message text: connect a
runtime, open loop setup, open **Limits…**, open **Requirements…**, open
**Channels…**, choose another method, identify, tune, validate, approve, restore
previous settings, wait (saying for what and roughly how long), or none. An
action is offered as the fix for its own failure only when something about the
next attempt differs, and the message says what.

**A11.3** Each stage of Part B lists its failure states. A defect in Control
Studio itself is reported as such, with its technical detail available for a
report.

## Part B. Workflow

The user-dictated [three-page flow](control-studio-complete-workflow-specification.md#three-page-automatic-flow)
governs which action runs when. This part says what each stage does.

### B1. Opening Control Studio and setting up a loop

**B1.1 Entry points.** **Open Control Studio** is available from the truST
project sidebar directly below Devices & Connections, from a configured
simulated plant, from a configured loop and from the command palette. Without
an open project the sidebar entry stays visible, disabled, with the reason. A
handoff from the plant configuration (Specification 34) keeps its project,
plant and loop. Opening a loop that already has a panel reveals that panel.

**B1.2 Loop definitions.** A project declares loops in `control-loop.json` at its
root and in `control-loops/*.json`. Discovery reads both in file-name order,
validates every definition, rejects duplicate loop ids and never writes files.
Several loops, or several projects in a multi-root workspace, need a labelled
chooser; cancelling it changes nothing. A missing or invalid definition is
explained, with **Configure control loop** and **Choose an example** offered.

**B1.3 Setup.** **Configure control loop** sets up an existing program from the
compiler's metadata: symbols with their IEC types and optional addresses,
controller instances with their compiled types and owning tasks, task intervals,
the code links from each `<output> := <instance>.<method>(…)` call, and every
symbol each program body writes. It prefills what the code states (a
`Control.PID`'s parameter record, setpoint, measurement, manual output and
output; a cascade where one PID's output is the only source of another's
setpoint; a startup flag guarding `Initialize`; the mode condition; manual
outputs that copy the output) and the engineer confirms every prefilled binding.
A link is evidence, never a silent choice. Setup saves only after a complete
review, never overwrites an existing definition, and cancelling writes nothing.
Setup shows what it found in the code as a short summary (controller, parameter
record, measurement, output, setpoint, the mode signal and what its TRUE value
means, manual output, startup flag and task), each with **Change**, and asks only
for what the code does not state: the loop name (prefilled from the controller),
ranges that no instrument declares, and the controller action (prefilled as
increasing the output while the measurement is below the setpoint). It never asks
for an identification step or duration: Identify chooses them within the limits
(B3.3, B3.4). The identify-only structure is the one exception: the project runs
that experiment, so setup asks how long to record it (C12). **Step-by-step setup** remains for programs the form does not
recognise. Part C lists what each structure's setup asks for.

**B1.4 What a loop definition declares.** One binding per fact:

- project, loop id, name and binding revision (a hash of the declaration);
- the controller structure (Part C), its instance or instances and their
  compiled types;
- the setpoint (closed loop), the measured outputs, the plant inputs, the manual
  output, and the mode signal with its Manual value; the unit and expected range
  of each; optional auxiliary channels, each with a stable id and one role (SP,
  PV, MV, actuator, disturbance, state, quality or schedule);
- the controller action: whether the output increases while the measurement is
  below the setpoint (`direct`, as for heating) or above it (`reverse`, as for
  cooling). Setup and messages show the direction in words. Identify checks it
  against the sign of the identified gain (B3.9), and Tune gives the gains the
  sign it requires (numerical annex N6);
- the loop's task (its period comes from the compiled program, never typed);
- the parameter record and initialization adapter of the structure (Part C);
- the startup-done flag when the program initializes the controller itself;
- limits: the plant input range, the permitted measured-output range and the
  maximum experiment duration; optionally the expected settling time;
- requirements (B4.3), optional;
- the identification policy, optional: whether open-loop tests and sustained
  holds are permitted, the maximum model order, the fit error limit, and whether
  the process is known to integrate (`process: "integrating"`, for example a
  level).

Loop files keep schema version 2. Where version 2 holds one fact in two fields
(the mode signal in `modeSignal` and in `parameterBinding.mode`, the period in
`taskPeriodMs` and `nominalPeriodMs`), the loader requires them to agree and
rejects a definition where they do not; setup writes both consistently.
`trackingSignal`, `maximumJitterMs` and `operatingPoint` are read and ignored,
and are no longer written. Version 1 files are read as before.

### B2. Observing

When a study opens on a connected target, the page shows the selected signals
live, read by symbol (A5), before and during every operation. The chart grows
as samples arrive. A stale or unavailable value is shown as such, never as a
current value (A3.4). Live observation reads the runtime's coherent scans; an
editor poll is never evidence.

### B3. Identify

**B3.1 Purpose.** Produce a provisional model of the plant from one live
campaign, from an opened recording, or from a supplied model.

**B3.2 Inputs.** The selected plant inputs and measured outputs, the method
(Automatic or a named method), the limits (plant input range, permitted
measured-output range, maximum duration, optionally the expected settling time)
and the identification policy.

**B3.3 What Automatic chooses.**

- A loop in Auto with a declared setpoint: a **setpoint step** in Auto
  (closed-loop identification). Its size is the larger of 20 times the measured
  noise and 2 % of the measured output's range, capped at 10 % of the range and
  by the headroom to the permitted measured-output range.
- A loop in Manual, an open-loop experiment loop, or a loop in Auto without a
  setpoint binding where open-loop tests are permitted (switched to Manual,
  B3.4 step 1): a **plant input step** in Manual (open-loop). Its size starts
  at 10 % of the plant input's range, capped by the headroom to the plant
  input limits. A **pulse** replaces the step when sustained holds are not
  permitted. An on/off actuator steps between its two levels. Several plant
  inputs are stepped one after another in one recording; **PRBS** is used when
  the policy asks for frequency information.
- A loop in Auto without a setpoint binding, where open-loop tests are not
  permitted: no safe automatic method; Identify says so and names the missing
  binding or policy.

The engineer may choose a method instead: Step, Pulse, Setpoint step, Feedback
relay, PRBS, Multisine, Recorded-data fit (through **Open recording…**) or
Supplied plant model. Every live method follows B3.4.

**B3.4 The live campaign.**

1. **Preconditions.** A connected target; the selected signals fresh, good and
   not forced; the limits present (otherwise **Limits…** opens at the missing
   field); physical approval on a physical target (A2.5). A Manual method
   switches the loop to Manual through a transfer that holds the present output
   (A6.3) and returns it to its starting mode afterwards (A6.2).
2. **Steady baseline.** The baseline window is the longest of 20 samples, 5 % of
   the maximum duration and, when declared, 10 % of the expected settling time,
   capped at 30 minutes. The measured output is steady when its linear trend
   across the window moves it by no more than four times the measured noise,
   or 0.001 % of its range when that is larger. A tolerance tied to the range
   would let a short window call an output that still moves fast steady, and
   the step would then fit the earlier response's tail. Identify waits for a steady baseline for up to the longer
   of 5 minutes and 20 % of the maximum duration; it then stops with the rate
   still observed, offering to wait or, for a loop in Auto, a setpoint step.
3. **Step.** The runtime applies the step at the next execution of the loop's
   task (A3.5).
4. **Hold until settled.** The response has settled when, after it has left the
   noise band, it changes by less than the larger of twice the noise and 2 % of
   its change so far across the settle window: the longer of 20 samples and
   10 % of the hold so far. The hold ends when the response has settled or when
   the maximum duration, minus the restoration allowance of one baseline
   window, is reached. The maximum duration counts from the end of the
   baseline, so the wait for a steady baseline never shortens the step.
5. **Restoration.** The runtime restores the plant input or setpoint to its
   value from before the experiment and confirms it (A7). Identify does not wait
   for the process to return to its starting point.
6. **Limits check.** The runtime stops an experiment whose written target
   leaves its limits (annex E5.4); it does not watch the measured output.
   Before the fit, every recorded sample of the selected plant inputs and
   measured outputs is checked against the loop's limits: the plant input
   range and the permitted measured-output range. A sample outside them, or a
   recording that is not complete (A4.4), is an experiment stopped at a limit
   or with missing samples (B3.9): the recording is kept, nothing is fitted,
   and the refusal names the signal, the time and the value or the gap. The
   recording is not qualified for a fit: opened again (B3.6), it meets the
   same check with the same answer.
7. **Fit** (B3.5).

The recording covers the baseline wait and then the maximum duration, within
A7's limit, and uses the record period of A4.2 for that span. Identification
fits the recording from the start of the steady baseline window, so the wait
before it never sets the operating point; the wait stays in the retained
recording. The chart grows as samples arrive. Abort stops the experiment through the runtime's
restoration path at once.

**B3.5 Fitting.** Identify fits the forms of the numerical annex in order of
complexity: static gain, first order, first order with delay, integrating (with
delay), second order, second order with delay, then state models up to the
policy's maximum order. It keeps every attempt and its reason (A11.1) and
selects the simplest form whose free-running prediction meets the error limit
over the whole hold and over its final 10 %. A resolved delay that improves the
fit keeps first order with delay ahead of first order (numerical annex N3.3). A
partial response follows A10.3.
Closed-loop data is fitted from the recorded plant input to the measured output
as for any other excitation. The result is a **provisional** model with its
equation and fit evidence.

**B3.6 Opened recording.** **Open recording…** fits a recording from a file with
the same signal selection, method and presentation, without live excitation.
Ambiguous channels are selected first. Cancelling the file picker does nothing.

**B3.7 Supplied model.** A transfer function or state-space model with its
channels, units, operating point, delay and sample time enters as a provisional
model unless it carries evidence of independent validation.

**B3.8 Result.** Identify shows the model type and equation with the identified
coefficients, and the measured response with the model's prediction (Identify
page specification).

**B3.9 Failure states.**

| Cause | Next action |
|---|---|
| No target, or connection lost | Connect a runtime |
| A limit missing or invalid | Open **Limits…** at the field |
| A signal stale, bad or forced (named) | Wait, or remove the force |
| Physical approval declined | None |
| Not steady: the measured output still changing (rate given) | Wait; or Setpoint step for a loop in Auto |
| Mode change refused (the runtime's reason) | As the reason states |
| Experiment stopped: limit reached, forced signal, mode left, Abort, reload or restart | None; the recording is kept, not qualified for a fit (B3.4 step 6) |
| Restoration not confirmed | Restore by hand; the study is blocked until it is confirmed |
| Response too small against the noise (values given) | Open **Limits…** to permit a larger step, or choose another method |
| Response not settled enough (fraction reached, suggested hold) | Open **Limits…** for a longer maximum duration |
| Response still a ramp: a slow process cannot yet be told from an integrating one (numerical annex N2.3a) | Open **Limits…** for a longer hold; or a setpoint step in Auto |
| No adequate model (each form's reason) | Choose another method, a larger step or a longer duration |
| No input change: the recording holds no nonzero step (numerical annex N2.6) | Choose another method; for a project-scripted loop, open loop setup to check the project's scenario (C12) |
| Another operation of this loop is running, or journaled and unresolved (named) | Wait; or resume or cancel it |
| Identified gain sign contradicts the controller action | Open loop setup and check the signals and the action |

### B4. Tune

**B4.1 Purpose.** Produce a candidate for the selected controller from the
selected model and the requirements. Tune never writes to the running
controller.

**B4.2 Inputs.** The model (provisional or qualified), the controller (an
instance or a group, Part C), the method (Automatic or a named method of the
numerical annex and Part C), the requirements and the pinned settings. The
manual method takes the engineer's entered settings as the candidate; the relay
rule (numerical annex N5.R) applies only to a model fitted from a qualified
Feedback relay record of the same study.

**B4.3 Requirements.** Declared rows: settling time, overshoot, steady-state
error and error band, input disturbance, measurement noise (when a filter is
tuned), and actuator, rate and state constraints. Overshoot is stated in
percent of the test setpoint change and is checked only when it is declared:
a blank overshoot row declares no overshoot limit, and Tune never generates
one. When a test criterion is missing, Tune fills it from the model as the Tune
page specification describes. Automatic criteria are marked as such, frozen before validation and
never relaxed by its results. Settling times and observation windows may be as
long as the limits of A7 allow.

**B4.4 Design.** The numerical annex defines the formula methods and the
Automatic design to requirements; Part C defines each structure's design. The
simplest structure that meets every mandatory requirement is recommended; a
more complex one only when no simpler one can. An explicitly chosen structure or
method is never replaced silently.

**B4.5 Prediction.** Tune shows the predicted response of the candidate and of
the current settings on the selected model. For laws Control Studio calculates
itself (Control.PID and the program-owned P, PI and PID laws) it calculates
the closed loop; for any other program-owned law it runs the actual Structured
Text in an isolated simulation on the selected model (A2.6). When that is
unavailable, the candidate is shown with the reason there is no prediction.

**B4.6 Margins.** For every candidate whose law is linear on the selected model,
the result lists its gain, phase and delay margins and its peak sensitivity
(numerical annex) beside the settings.

**B4.7 Failure states.**

| Cause | Next action |
|---|---|
| No model | Identify |
| No controller selected, or its mapping missing (named) | Choose the controller; or open loop setup |
| Method not applicable (reason) | Choose another method |
| Requirements infeasible (the shortfall and the limit) | Open **Requirements…** |
| No candidate meets the requirements | Open **Requirements…**; or identify again with the reason |

### B5. Validate

**B5.1 Purpose.** Decide whether the candidate may be applied. One press of
**Validate controller** runs both parts below, in order.

**B5.2 Independent model check.** When the model is provisional, Validate
records a new live campaign with a different excitation: for a loop in Auto a
setpoint step, otherwise a plant input step of a different size or sign. It
follows B3.4, with the same limits, approval and mode rules. The frozen model's
free-running prediction from the recorded plant input is compared with the
recorded measured output on the grid of A4.3. The check passes when the error
is within the identification error limit over the whole response and over its
final 10 %. A passing check makes the model **qualified**; a failing check
blocks Apply.

**B5.3 Actual-ST controller validation.** The current and the candidate settings
run in isolated simulations (A2.6) under one immutable validation-conditions
record (numerical annex):

- against the project's configured simulated plant when the target is simulated
  and that plant drives the loop's signals: the loop names the plant, and the
  plant writes the loop's measured output address and reads its plant input
  address directly;
- otherwise against the qualified model. A plant that reaches the loop only
  through the program's logic, such as a scaled transmitter signal or a
  heating sequence, is validated on the model: an isolated run starts at time
  zero from the plant's configured state and the program's initial state
  (A2.6), not from the live operating point.

Part C names structures that always validate on the model. The scenarios cover
the applicable setpoint step, load step, noise and model variations, and, for
Auto commissioning, the Manual-to-Auto entry. Long processes are simulated for
their full observation (A10.5). The candidate reaches the program as Apply
delivers it: a program that sets its own parameter record during its first
execution has the candidate written again at the task boundary after that scan,
and must keep it from there (protocol annex E8.3). The current settings are the
record's values as the running controller holds them, read before the runs, and
reach the program the same way.

**B5.4 Results.** Every requirement reports Pass, Fail or Inconclusive with its
measured value, limit, unit and source. A scenario that did not run is
Inconclusive; missing evidence never passes. **Apply** becomes available only
when the model check passed and every mandatory requirement passes for the
current candidate.

**B5.5 Failure states.**

| Cause | Next action |
|---|---|
| Model check failed (error and limit) | Identify again; the message says what to change (method, step or duration) |
| Requirement failed (value and limit) | Tune, or open **Requirements…** |
| Inconclusive (the missing evidence) | As the reason states |
| Isolated simulation unavailable (reason) | As the reason states, for example reload the program or start the local analysis runtime |
| Isolated simulation failed or over its wall budget | None; the reason and any captured prefix are kept |

### B6. Apply, Restore previous settings and Keep in project

**B6.1 Apply** writes the validated candidate to the selected controller or
group, in one transaction at the task boundary: the parameter record, the
state initialization and, for Auto commissioning, the mode entry (Part C). A
physical target needs approval (A2.5).

**B6.2 Operating point.** Apply starts from the operating point the candidate
was validated at. For a loop in Manual, a plant input outside the Auto-entry
allowance of that point is refused at once and named, since it cannot settle by
itself. For a loop in Auto, the plant input is the running controller's output:
it moves with every measurement (A6.4), so it is not compared, and the transfer
starts from wherever it is (C1). Measured outputs
outside their settling bands are waited for: Apply shows each output with its
validated and current value and continues by itself once all are inside,
writing nothing meanwhile. The wait can be cancelled and lasts at most the
longer of 60 s and three times the test duration, in process time. An
integrating output takes its present value as its operating point.

**B6.3 Observation.** After the transfer, the validated target change runs and
the runtime records the response for the test duration in process time. The
observation is owned by the runtime and reattached on reopening (A8.5). The
recorded response is assessed against the requirements. A failed requirement is
reported, with **Restore previous settings** offered. When a measured output
leaves its permitted range, Control Studio restores the previous settings at
once and reports why. The measured outputs are the measurement and every
auxiliary channel of role PV or state, each checked against its own declared
range (B1.4). While the panel or VS Code is closed nobody supervises the
permitted range: the observation keeps recording in the runtime, and a departure
is acted on as soon as the loop is reopened. The PLC's own protections stay in
force meanwhile. A reattached observation's result carries the same confirmed
transfers as an uninterrupted one: those journaled before the target change, and
the target change as the runtime still reports it under its journaled identity.

**B6.4 Restore previous settings** returns the parameter record, and the mode
from before Apply, to the retained pre-Apply revision through a guarded
transaction. Live state is initialized from current observations, never
replayed from old history. **Restored** is shown only after the runtime
confirms it. If the active revision or target changed since Apply, Restore
explains that it is unavailable instead of overwriting another change.

**B6.5 Keep in project** is the separate, explicit action after Apply: the
values the loop's parameter record holds in the running PLC become the record's
initial values in the project, so a restart or a new download keeps them. It is
never a side effect of Apply or Restore and is not offered after a Restore.

- A record written by hand is edited in its own declaration: the kept fields of
  its initializer take the running values, written in the same form (`LREAL#…`
  stays typed); other fields stay as written; a missing field is added; a
  declaration without an initializer gets one. The edit applies only to the text
  the language server read; a file without unsaved changes of the user's own is
  saved.
- A record that Create PLC code declared belongs to the equipment design: it is
  kept through the HMI Builder's equipment edit (Specification 39 §21.4 item 6),
  which writes `plant.toml` and the generated declaration together. Values the
  equipment has no setting for are named as not kept.
- A record inside a function block instance is not edited, because one
  declaration serves every instance; Control Studio says to copy the values by
  hand.

The answer names the fields kept; a refusal keeps nothing and says why.

**B6.6** Apply never edits Structured Text and never changes the project's
startup or retain policy.

**B6.7 Failure states.**

| Cause | Next action |
|---|---|
| Candidate or validation no longer current | Validate |
| A plant input away from the validated operating point (values given) | None until the plant input is back; Cancel |
| Transfer failed (the runtime's reason); the loop kept its mode (A6.3) | As the reason states |
| Parameter change rejected: the program writes the named parameters | Open loop setup; change the program |
| Observation: a requirement failed | Restore previous settings |
| A measured output left its permitted range | None; the previous settings were restored |
| Restore not confirmed | Restore again; dependent actions stay blocked until confirmed |

### B7. After Apply

After Apply the Validate page keeps showing the loop live, with **Restore
previous settings** in the Apply position. This version does not detect drift
automatically; a new identification is the engineer's decision.

### B8. Automatic progression

The [three-page flow](control-studio-complete-workflow-specification.md#three-page-automatic-flow)
applies. In particular: **Identify plant** stops on Identify, **Tune
controller** stops on Tune and **Validate controller** stops at Apply;
navigation never starts work or discards a result; while an operation runs its
button becomes **Abort**; a failure stops the sequence and keeps every result
obtained. A compact editor's **Done** resumes a start only when that start
opened the editor; **Cancel** discards the edit and cancels any start that was
waiting for it.

### B9. Shared truST UI/UX contract — mandatory

#### B9.1 Page structure

The normal workspace has exactly Identify, Tune and Validate navigation.
Validate owns the Apply and Restore previous settings actions. The page
specifications are the only layout authorities (§0.2): they set each page's
element order and conditional controls.

Reuse the shared [theme](../../editors/vscode/src/webview/theme.css) and
[tokens](../../editors/vscode/src/webview/theme.ts). The UI MUST NOT add another
UI system, navigation row, toolbar, dashboard or general-purpose Details panel.

- Every visible item MUST support a decision or an action on its page.
- A user MUST be able to see the current page, the next action, the selected
  target and the meaning of the result without opening help or reading a
  paragraph. A short field explanation MAY appear on keyboard focus or hover. It
  MUST NOT add permanent text.
- The UI MUST NOT show a permanent status strip. There is no runtime-status
  strip and no SP/PV/MV strip.
- The response plot MUST stay dominant, with readable axes and a small legend.
  Identify shows the model type and equation. Tune shows the controller and its
  settings. Validate shows the measured requirements and the Apply or Restore
  previous settings action.
- Layouts MUST fit wide, split and narrow editors, including 200 % zoom, without
  clipping controls, labels, equations or parameter values.
- Layouts MUST NOT use nested scroll areas that hide the primary action or leave
  too little room to read the plot.
- Related controls MUST stay aligned. When a compact editor closes, focus MUST
  return to the control that opened it.

#### B9.2 Labels, names and inputs

- Labels MUST state what is selected, calculated or changed.
- Method names MUST be the full identification or tuning name. An abbreviation
  MAY follow the full name but MUST NOT replace it.
- The selected target MUST name its kind: **Simulation**, with its speed when
  that is not 1× (for example **Simulation ×10**), or **Physical**. When the
  configured speed is above 1× and the achieved speed is more than 5 % below
  it, the label MUST say so (for example **Simulation ×7 of ×10**). At 1× no
  achieved speed is shown.
- A signal selector MUST show the signal's actual name and unit.
- A controller choice MUST name the actual ST instance or declared group, and
  show its recommended configuration.
- Settings MUST use readable labels with their units, such as **Proportional
  gain (Kp)** and **Integral gain (Ki)**.
- An example value MUST NOT appear as a placeholder result or as an operating
  limit accepted silently.
- The compact Limits, Requirements and Channels editors MUST show clear field
  labels, units and current values, with **Done** and **Cancel** actions. They
  MUST show only the fields that apply.
- Invalid input MUST name the field and its permitted value, keep the other
  edits, and move focus to the problem.
- The UI MUST NOT show JSON, protocol names, hashes, scan counters, proof
  bookkeeping or unlabelled status symbols.

#### B9.3 Action area

The action area of a page holds that page's operation messages and controls:
failures and their next actions, the approval block, the progress display and
the reasons an action is unavailable.

- **Failures.** Every failure MUST show its typed next action (A11.2) beside its
  message. An action that opens or starts something, such as **Limits…**,
  **Requirements…**, **Channels…**, connect a runtime, identify, choose another
  method or restore previous settings, MUST be a control, not text. A wait MUST
  name what it waits for and roughly how long, and MUST show the control that
  stops it. A next action of none MUST show the reason alone. An action that
  would only repeat the failed attempt is offered only when something differs,
  and the message MUST say what differs (A11.2).
- **Unavailable actions.** Missing required values, errors and
  unavailable-action reasons MUST appear beside the control they concern, with a
  specific corrective action. A tooltip MUST NOT be the only explanation of why
  an action cannot run.
- **Errors.** Errors and required recovery MUST appear in the owning action
  area, never as permanent tab badges. A failure MUST keep the values entered
  and the earlier valid results. The plot MUST NOT show example data in place of
  missing evidence.
- **Approval block.** When an operation on a physical target needs approval
  (A2.5), the page that owns it MUST show the approval block inline in its
  action area, not on a new page. The block lists the target, the loop, every
  signal written with its values and limits, the duration, the stop conditions
  and the restoration. It offers **Approve** and **Cancel**. A2.5 defines what
  one approval covers. A simulated target shows no block; pressing the action
  confirms it (A2.5).
- **Progress.** A long operation MUST show its elapsed and remaining process
  time (A3.6). When the observed time scale differs from 1 by more than 5 %, it
  MUST also show that time scale and the expected finish in wall time. The
  progress display stays inside the action area.

#### B9.4 Navigation and operation state

- Navigation MUST NOT start work, overwrite a deliberate page choice or discard
  a result (B8).
- While work runs, its button becomes **Abort** (B8). The Abort control MUST
  stay reachable on every page, and every page MUST target the same operation
  identity.
- Changing a view MUST NOT duplicate an acquisition or a parameter write.

#### B9.5 Numbers

All displayed process values and analysis results MUST use the shared
engineering-number formatter. The formatter MUST:

- suppress meaningless floating-point digits, and keep meaningful small non-zero
  values;
- use engineering notation when fixed decimals would mislead;
- render invalid or unavailable values explicitly;
- render a mathematically unbounded positive or negative margin as infinity, not
  as missing data.

SP, PV, MV, limits, plot values and tooltips, and matching summaries MUST use
the same precision. Controller and model parameters MUST keep enough displayed
precision to reproduce the result. Display formatting MUST NOT change the
full-precision value that is used internally, edited, recorded or exported.

#### B9.6 Response projection

- One response projection and one chart implementation serve every page.
- Predictions from the selected model use the reference change, duration,
  controller period and physical operating point of the task. A zero-delay model
  MUST NOT gain an extra controller-period delay. A model delay is realized in
  whole controller periods, rounded to the nearest. If the current controller
  cannot be modelled, its baseline MUST NOT be a copy of the tuned response.
- Display data is bounded, but exact underlying samples are retained. Gaps,
  quality, source, channel units, and event and limit identities are preserved.
  Display reduction keeps extrema and gaps, including the quality and gap
  boundaries of auxiliary channels. It MUST NOT invent samples.
- A candidate edit changes only its unapplied result. Dependent responses,
  margins and requirement evaluations are recomputed against the same candidate
  and model revision. Results from different revisions MUST NOT be combined.
- An older scan MUST NOT replace a newer observation or refresh its freshness. A
  new runtime epoch or program revision (A2.2) starts its own scan history.
  Events from another target MUST NOT populate the active one.
- The projection keeps every declared auxiliary sample's identity, unit and
  quality, in live and reopened recordings. A missing, bad or forced auxiliary
  value is a gap in its own trace. It MUST NOT remove good primary measurements.

#### B9.7 Plot integrity

- Requested output and retained controller state are distinct from applied MV.
  When they exceed the actuator range, the shared unit axis expands to include
  them, and the actuator limit lines stay drawn.
- Incompatible units MUST NOT share one axis. Channels and units MUST stay
  explicit and time-aligned.
- Time-response plots frame SP, PV and MV around their actual displayed
  variation. A distant zero or rated actuator maximum MUST NOT be forced into
  the range when that hides the response being assessed.
- Declared bounds remain in the result and in the Limits and Requirements
  editors. Relevant active bounds are drawn within the visible range.
- Data MUST NOT be clipped to imply compliance. Out-of-range values, active
  limit crossings and data that reaches a limit stay visible. This includes
  requested and state traces against the actuator range.
- Constant-value numerical roundoff MUST NOT appear as meaningful oscillation.
- The plotted time range ends at the last recorded sample.
- Recorded events keep their exact time, source and meaning. An event MUST NOT
  be inferred from the shape of a curve.
- Measured and predicted traces MUST have distinct labels and line styles.

#### B9.8 Themes, accessibility and motion

- Every screen MUST render in the shared dark, light and high-contrast themes.
- Keyboard focus MUST be visible. Controls MUST have accessible names and
  descriptions. Every action MUST have an equivalent keyboard operation.
- Equations and response data MUST be accessible without adding a permanent
  toolbar or report.
- Plots MUST show readable axis units, trace identities and non-overlapping tick
  labels.
- Colour and line style MUST distinguish data without relying on colour alone.
- Reduced motion MUST be respected. Animation MUST NOT delay a value, a
  cancellation or a required recovery.
- Analysis MUST NOT block the page. It MUST be bounded and cancellable.

The rendered review of these rules is required by D6.

#### B9.9 Simple by default, complete one click away

Control Studio serves a beginner and an expert with the same pages. Everything a
beginner needs is visible; everything an expert adds is one click deeper, never a
page of settings. These rules follow the user's direction of 8 October 2026.

- **Signal names.** Plots, tables and messages name a signal by its role in the
  loop: Measurement, Output, Setpoint, or an auxiliary channel's id, with the unit
  in parentheses. The panel title names the loop. The full symbol appears on hover.
  Signal selectors show the symbol with its unit.
- **Durations.** A duration field accepts "90 s", "15 min", "2 h", "1 h 30 min" or a
  plain number of seconds. A duration is shown in the largest fitting unit: seconds
  below 2 min, minutes below 2 h, then hours and minutes.
- **Progress.** A running operation shows one line in its action area: what it is
  doing and its elapsed and remaining process time (A3.6), for example "Holding the
  step until the response settles — 12 min of at most 3 h 50 min".
- **Failures.** A failure shows one short sentence in plain words and at most one
  button for its fix (A11.2). Selecting the sentence reveals the full technical
  reason in place, for example each model form's reason; there is no separate
  details panel.
- **Empty states.** Without data a plot area shows one line, such as "Start the
  runtime to see live values" or "Waiting for the first samples", not empty axes.
- **Model.** The identified model shows its type, its equation and one plain line
  of its key numbers with units, for example "Gain 0.21 °C per %, time constant
  75 min, delay 12 s".
- **Manual settings.** In manual tuning the settings are edited in place and the
  predicted response follows each edit; there is nothing to pin. After automatic
  tuning, a calculated setting can be kept fixed for the next tuning run with a
  small Keep toggle beside it.
- **Analysis tools.** The tuning method list holds only tuning methods. Bode,
  Nyquist, root locus, pole-zero map, Routh–Hurwitz, lead/lag design and the
  frequency and multichannel tools are behind one **Analyze…** entry on Tune, shown
  once a model exists. A chosen tool shows its own few inputs in the result area
  and its plot in the plot area; closing it returns to the tuning result.
- **Requirements.** The Requirements editor shows maximum settling time, maximum
  overshoot and maximum steady-state error, each with the band it needs, and an
  optional input disturbance. A collapsed **More…** holds the test design: the test
  setpoint change, the validation duration, the Auto-entry tolerance, measurement
  noise and scenarios, filled automatically and marked Automatic. The signal column
  appears only for loops with several outputs.
- **Validate.** Apply, or Restore previous settings, sits directly under the results
  table. Pass, Fail and Inconclusive are marked with an icon and colour as well as
  the word.

### B10. Study storage, results and export

#### B10.1 Study files

- A study is one versioned study document and one file per recording, stored in
  `control-studio/<loop id>/` in the project. Workspace state holds only
  operation journals (A8.5) and the pointer to the current study.
- Studies are plain versioned files in the project, without a database or
  synchronization service.
- The study document holds the results and their revisions (B10.2 and B10.4). It
  has its own schema version. Each recording file holds one recording (B10.3).
- Autosave keeps the study's page, signals, channel order, methods, candidate,
  pins, requirements and result identities. Autosave and reopening follow the
  save and reopen behaviour of the [complete workflow
  specification](control-studio-complete-workflow-specification.md). Reopening
  restores only compatible saved data. Opened recordings open on Identify
  (B3.6).
- Saving and reopening never acquire recording data or write to the
  runtime (A8.6).
- The study has no separate save session, result-history panel or
  recording-management page.

#### B10.2 Results

- Each result carries a stable result ID, a positive revision, its creation
  time, the project ID, the loop ID and loop revision (the binding revision of
  A8.2), the controller instance and its revision, its evidence source (A9), the
  extension version, and the runtime and numerical adapter versions available
  when it was created.
- Each saved revision keeps its original source, controller, model, method and
  acquisition identities.
- Result documents are written at schema version 4. Reading a version 3 result
  stays supported. The read keeps its version and source content.
- When applicable, a result also keeps:
  - the experiment: kind, affected signal and address (if it has one), requested
    and applied value, the complete commanded trajectory, duration, derived
    interval, PRBS seed or multisine frequencies, approval source, bounds, stop
    conditions, stop reason, abort state, restoration value, restoration result
    and restoration-failure reason;
  - the plant: plant identity, simulation configuration, effective seed,
    uncertainty, scheduled events, actuator behaviour and declared
    model-validity range;
  - the identification: selected recording intervals, accepted model, fit and
    residual evidence, current controller, unapplied candidate, tuning method
    and objective;
  - the validation: requirements, measured metrics, margins predicted by the
    model, uncertainty results, evidence source and conclusion.
- Every observed sample and measured metric keeps the evidence source of its
  result. Simulated plant and Isolated simulation evidence MUST NOT be loaded,
  copied, compared or exported as Physical plant evidence. Model prediction MUST
  stay distinguishable from observed evidence after every save and reopen (A9).
- An opened recording keeps its original source and carries the opened marker
  (A9).
- A run with no acquired samples records `evidence.observation: "Not observed"`.
- A declared analysis is stored under `declaredAnalysis`. It keeps the exact
  definition, the calculated result, the selected parameter value, the fixed
  parameters and the model provenance. Reopening restores the result and the
  selection.
- Reopening a saved result creates no candidate, validation pass or runtime
  authority. An absent candidate MUST NOT inherit another result's predictions
  or application status.

#### B10.3 Recordings

- A raw recording keeps synchronized SP, PV, requested MV and applied MV, with
  elapsed time (process time, A3.2), observation time, actual cycle interval,
  scan number, engineering units, signal quality, freshness, controller mode,
  active output and rate limits, safety state and event markers.
- A recording made with a record period of k > 1 executions states k; each of its
  samples holds the means over those k executions (A4.2).
- Requested MV and applied MV are separate nullable fields. The usual controller
  output binding is the applied plant command. Requested MV is null when it is
  not observed: if no independently observed requested-output signal is bound,
  it stays null. It MUST NOT be copied from applied MV to fill the record.
- Missing values stay missing. Saving, reopening, comparing and exporting MUST
  NOT interpolate or invent them. Analysis resampling (A4.3) is never written
  back into stored samples.
- Each sample's additional channel entries keep their id, channel, role, unit,
  nullable value and quality. Channel identities and units stay the same
  throughout one recording revision.
- An open-loop result declares `loop.topology: "open-loop"`. It carries no SP
  value or SP unit, and MUST NOT derive either from PV.
- Ordering uses runtime elapsed time and scan sequence. Wall-clock timestamps
  MUST NOT replace that time base.
- Live observation, the identification baseline and recording start use the same
  ordered signal selection, including the actuator, auxiliary, controller-mode
  and acquisition-mode channels. A background observation MUST NOT replace the
  selection a recording needs. A recording start whose outcome is unknown is
  never replayed (A8.5).
- An ended recording is immutable (A8.1) and available to the study as soon as
  acquisition ends. Background observation MUST NOT append to or replace it.
  Saving a recording does not qualify it for model or controller validation.
- A recording from an aborted test, or from a test not qualified for
  identification, remains available for Save, Open and export. Identification
  stays disabled for it, and its stop reason, quality and restoration result are
  kept. Complete acquisition does not establish experiment qualification or a
  passed controller validation.
- A new runtime epoch or program revision (A2.2) gives the live response a new
  time origin and sample window. Recordings MUST NOT be joined across a runtime
  restart. The live response MUST NOT interpolate across one or reuse a
  saturation transition from before it. Saved results and retained recordings
  keep their original identity. Late replies from an earlier epoch are rejected.
  A backward scan within one unchanged epoch is invalid.

#### B10.4 Revisions and history

- A changed result creates the next revision. A revision MUST NOT rewrite or
  alias the raw recording of an earlier revision.
- Result history is bounded by the study storage limit of A7: 256 MiB per loop,
  recordings included. A save that would exceed the limit is a failed save
  (below).
- Replacing or deleting a result or a recording MUST be an explicit action.
- A result invalidated under A8.4 stays in the study as history and cannot
  authorize Apply.
- A failed or interrupted save MUST leave the last valid study readable and MUST
  report a specific reason.
- A record with a missing or unknown schema version, a malformed identity,
  nonfinite samples, inconsistent units or source, inconsistent sample order, or
  an invalid selected interval MUST be rejected as a whole. No part of it is
  accepted.

#### B10.5 Export

- JSON export writes the complete typed workflow result. CSV export writes the
  selected signal table. Both keep the additional channels.
- The CSV columns are: elapsed time, observation time, actual cycle interval,
  SP, PV, requested MV, applied MV, units, quality, mode, safety state, limits,
  source and event markers. Numeric precision MUST be enough to recalculate the
  saved metrics.
- Exports carry full-precision values (B9.5).
- A PNG export is presentation only. It is not a data record.
- Exports MUST NOT contain credentials or runtime secrets.

## Part C. Controller structures

Each structure has one template. Its subsections are Setup, Identify, Tune, Validate, and Apply and Restore, in that order. A subsection is omitted when the structure has nothing specific to say. Part B governs the common behaviour of every stage. Part A governs every rule that this Part refers to.

### C0. The template and what all structures share

#### Parameter-record adapter

A parameter-record adapter is a set of typed bindings. Each binding names an unaddressed scalar member of a program or function-block instance, by its symbol. Each binding has:

- IEC type `REAL` or `LREAL`;
- a non-empty unit and convention;
- finite minimum and maximum values.

Aliases and shared global ownership need a different adapter.

- A set has one to 64 bindings. No symbol may appear twice. The set MUST NOT contain the mode symbol.
- Every binding in a set MUST belong to a program owned exclusively by the declared task. `$background` selects unscheduled programs.
- The mode binding is a `BOOL` symbol with its Manual value (A1).
- The binding identity is a hash of the complete declaration, including units and limits. The adapter reports three revisions: the program revision (A2.2), the binding revision, and the active revision, which is the last confirmed set.
- Values MUST be finite, within their bounds, and representable in their IEC type. A `REAL` value MUST be rounded to single precision before it is submitted. Integer and enumeration gains are rejected.
- A write is queued with the expected revisions and the expected current values. At the task boundary the runtime rechecks them and commits the whole set or nothing (A6.3, A6.8). Control Studio reads the values back after the task has executed. A program that rewrites any value is handled by A6.8.
- A write that changes parameters only needs Manual when it is queued and again at the boundary. Custom adoption in Auto is not supported. An Auto entry is a transfer (A6.3), described for each structure below.
- Declarations are checked before any runtime call. The checks reject unknown fields, duplicate symbols, a parameter that shares the mode signal, non-finite or reversed bounds, and unsupported types. Only schema version 2 loops declare parameter bindings (B1.4).
- `analysisParameterSymbols` maps each analysis parameter to one unique parameter symbol. The mapped parameter has the same unit, and bounds that contain the analysis range. A missing or ambiguous mapping disables adoption.

#### Controller state initialization

- A state initialization is a `declared-state-v1` record. Each state has its compiled IEC type, its unit, its bounds, and either an explicit initial value or a coordinate of the selected model. The nominal actual-ST run loads these states and checks their declared bounds. Auto entry uses the same record through the task-boundary transfer (A6.3, B6.1).
- The record may take its values from fresh task-boundary sources: `model-input`, `model-output`, `model-input-is`, `integral`, `weighted-error`, `notch-history`, `scaled-measurement` (C8) and `decoupled-output` (C7).
- An actuator input uses the bound applied feedback where it exists.
- With several actuators, the coupled transfer binds each actuator and its typed Manual command separately from the controlled references. The reviewed output-change tolerance of the primary output applies to each additional actuator as the same fraction of that actuator's declared span. Later target changes use the full reviewed spans. Every actuator command and state requires a compiled binding.
- Recovery restores the original reference vector and the parameter record.

#### Families and zero terms

| Family | Mappings the adapter needs | Terms written as explicit zero |
|---|---|---|
| P | proportional gain | integral gain and derivative gain |
| PI | proportional and integral gains | derivative gain |
| PD | proportional and derivative gains | integral gain |
| PID | proportional, integral and derivative gains | none |

- The adapter declares a family only from its actual bindings. A P, PI, PD or PID choice is offered only when its mappings exist.
- A structure writes the complete record. Each gain that the family does not use is written as an explicit zero.
- The adapter MUST map every field that the selected structure or tuning method can change. These fields are the gains, the setpoint weight, the filter time, the bias, the output and rate limits, the tracking time and the anti-windup symbol. Each binding records its IEC type, its unit, its range and its parameter-record revision.
- Tune MUST keep the exact mapped record, not only the displayed gains. Validate runs that record, and Apply writes it.
- Tune recommends the simplest structure that meets every mandatory requirement (B4.4). A PI, PD or PID term is kept only when it gives a material benefit under the matched scenarios. An improvement beyond the requirements does not displace a simpler qualifying design.

#### Setpoint weight, measurement filter and anti-windup

**Setpoint weight β.** A PID law MAY declare `proportionalSetpointWeightSymbol`: one unique bound `REAL` or `LREAL` record field, dimensionless (unit `1`), within 0 to 1. The proportional term is `Kp·(β·SP − PV)`. The integral term uses the full error. Manual PI and PID settings require β beside the gains. Predictions use β, as the controller does: `Control.PID` without integral action ignores β, so a P or PD law is predicted with β = 1 (specification 32 §3.10). β is written through the same complete record. A calculated candidate writes β = 1 unless β is pinned.

**Measurement filter Tf.** A PID law MAY declare `measurementFilterTimeSymbol`: one unique bound `REAL` or `LREAL` program setting in seconds, not negative. It holds the time Tf of the first-order filter on the measurement that the controller uses.

```text
yf ← yf + α·(y − yf),   α = Δt / (Tf + Δt)
```

Tf = 0 bypasses the filter.

- Manual settings require Tf. Predicted responses and loop frequency responses include the filter lag. Tf is written with the gains.
- A calculated candidate writes Tf = 0 unless Tf is pinned.
- The program restarts the filter from the fresh measurement in Manual and in the scan of the Auto entry. The first output after the Auto entry is then checked under A6.4.

**Back-calculation anti-windup.** A PID law MAY declare `antiWindup: { method: "back-calculation", trackingGainSymbol: "..." }`. The symbol names one unique bound `REAL` or `LREAL` parameter in reciprocal seconds. Its value is the tracking gain `1/Tt`. Zero is the untracked baseline. A candidate uses a finite positive `Tt` and writes `1/Tt` through the same complete record. The law's declaration gives the value its tracking meaning, not the parameter's name.

```text
I* = trial integral
u_r = request computed with I*
u_a = u_r clamped to the output limits
I_next = I* + (Δt / Tt)·(u_a − u_r)
```

A loop without this declaration cannot offer a back-calculation candidate.

#### Margins of linear candidates

For every candidate whose law Tune writes as a linear transfer function on the selected model, Tune shows the gain margin, phase margin, delay margin and peak sensitivity Ms beside the settings. This applies to `Control.PID` and to a program's P, PI, PD or PID law that has a `parallel-pid` seed. The margins include the candidate's measurement filter and the model's delay. With the loop transfer function `L(jω)` of the candidate and the model:

```text
gain margin:   −20·log10|L| dB, where the phase of L crosses −180°
phase margin:  180° + phase of L, at the gain crossover where |L| = 1
delay margin:  phase margin in radians / ω_c, with ω_c the gain-crossover frequency
Ms:            the largest value of |1 / (1 + L(jω))|
```

- A margin the loop does not have over the whole frequency axis is shown as unbounded (B9.5: as infinity). This applies when the phase never crosses −180°, when the loop gain stays below one, or, for the delay margin, when there is no gain crossover. A crossing that a bounded frequency range does not show is reported as not observed in range, never as unbounded (numerical annex N7.8).
- Margins are only as trustworthy as the model. A switching or other nonlinear law shows no margins.
- Margins and frequency results need a valid model or a measured frequency response.

#### Model input of formula methods

- Formula methods use the model accepted by Identify. A comparison never refits or rescales the model for one method.
- A discrete single-state model with scalar transition `a`, sample period `T`, input coefficient `b` and no direct feedthrough gives an exact held-input continuous equivalent:

  ```text
  A = log(a) / T
  B = b·A / (a − 1),   with B = b / T when a = 1
  ```

  The output coefficient and the operating-point coordinates are unchanged. The transfer function is `c·B / (s − A)`. The discrete model stays the model of record: validation runs the original model and the actual ST. Negative or zero sampled poles, direct feedthrough and higher-order sampled models need their own supported route.
- Identification policy defaults apply unless a structure states its own. `identificationPolicy.maximumNormalizedError` defaults to 0.15. `identificationPolicy.maximumOrder` defaults to 4, with supported values 1 to 8. For a low-order step fit, the error limit applies to the whole independent response and to the RMS prediction error over its final 10 % of samples, with at least three samples. Both are normalized by the measured output span.

#### Validation conditions and requirements

- The current and the candidate settings run under one immutable validation-conditions record (B5.3). The record holds:
  - the complete plant and controller initial condition;
  - each setpoint or load event, with its signal, amplitude and time;
  - the exact sample-time sequence and the total duration;
  - the disturbance trajectory;
  - the noise generator, its amplitude and its deterministic seed;
  - the output and rate limits;
  - the model-variation cases.
- If a required condition is absent, or differs between the current and the candidate run, the comparison is invalid. Its requirements are Inconclusive.
- An isolated run starts its first scan with the bound process inputs of the running PLC, as they were when the run was requested. Outputs and memory start from the program's initial values, except for the seeded values of C1 and C2.
- An input-disturbance requirement is a constant load added to the plant input at the start of a separate trial. The setpoint is unchanged during that trial.
- Validate covers the applicable subset of the following: setpoint response; disturbance response; rise time, settling time, overshoot and steady-state error; manipulated-variable excursion, rate, saturation and recovery; noise response; gain and phase margins; bounded model or parameter variation. Each requirement reports Pass, Fail or Inconclusive (B5.4).
- For a model prediction, the default noise-response limit is the larger of 120 % of the current controller's noise-driven manipulated-variable root-mean-square movement, and 0.1 % of the configured output span. The result shows the numeric limit and its unit. A project-specific limit replaces the default only before the immutable validation plan runs.

#### Where a structure validates

B5.3 applies to every structure. A structure that always validates on the model says so in its Validate subsection. Every other structure follows B5.3: the configured simulated plant when that plant drives the loop's signals, otherwise the qualified model.

#### Missing adapters

- A missing or incompatible adapter blocks only the operations that depend on it. The owning page shows the reason in its action area (A11). Acquisition and analysis stay available.
- A controller with missing, ambiguous, stale, incompatible or non-atomic mappings shows the reason that the structure is unavailable. Control Studio MUST NOT enable a structure from the controller type or from a symbol's spelling. It MUST NOT invent a mapping, and it MUST NOT use a different UI to enable Apply.
- Auto commissioning of a PI or PID loop needs a typed initialization adapter in addition to the parameter-record adapter. The proportional-only adapter of C13 is never used for PI or PID.
- Without an initialization adapter, Auto is blocked. Manual staging stays available wherever the structure supports it.
- A mapping alone never grants Auto authority.

#### Common Apply rules

- Apply is one transaction at one task boundary. It writes the parameter record, the state initialization and, for an Auto entry, the mode entry (B6.1, A6.3). Each structure lists its own members.
- If the first-output check after the entry fails, the runtime reverts every member it wrote in the same scan, and the loop keeps the mode it had (A6.3). The candidate stays available for Manual staging where the structure supports it.
- The observation after the transfer runs in process time (B6.3). A runtime that stops publishing scans is reported through the connection-loss timeout (A3.4); one that runs behind real time is shown with its observed time scale (A3.6).
- Bumpless entry means that the first controller output after the Auto entry meets the Auto-entry allowance of A6.4. A structure states its own allowance only when it differs from A6.4.
- Control Studio never replaces the project's controller equations. It sets, observes, tests and records evidence for the project's own code.

### C1. Control.PID single loop

This structure is one `Control.PID` instance in a loop with `design: "pid"`. The primary guided PID journey executes this library block as Specification 32 defines it. An equation written separately in Control Studio is never a substitute.

#### Setup

- The structure is recognized from the compiled type `Control.PID`, never from the instance name (A5.4). Setup prefills the bindings that the code states (B1.3): the parameter record that `Configure` receives, the setpoint, the measurement, the manual output and the output.
- **The measurement.** The measurement is the PID's own `<instance>.StateValue.ProcessValue`: the value that its last `Update` received, whatever the program computed it from, such as a sensor input, a sensor block or a filter. Control Studio shows, records and identifies against it. It is never refused as computed. The transfer reads it at the task boundary, and never writes it as a state. The setpoint, output and manual output remain the program variables that the code feeds into and takes from the PID.
- **The parameter record.** When the code configures the PID from a record of its own, the PID's internal copy `<instance>.ParametersValue` is not offered as the parameter record. `Configure` would overwrite what Apply wrote on the next scan.
- **The mode.** Setup prefills the mode signal, and the value that means Manual, from the `IF` that sets the record's `Mode`. It also prefills what the TRUE value of that condition means (B1.4).
- **The startup flag.** When the code guards `Initialize` with `IF NOT <flag> THEN`, setup prefills that BOOL as the startup-done flag. When the program writes one of the loop's seeded values itself, or calls `Initialize` on the loop's PID, setup requires the BOOL that marks the startup as done. The flag is declared as an application state that Validate starts TRUE.
- **The manual output.** The manual output is bound by `controller.manualOutputSignal` and its `controller.manualOutputType`, with `controller.manualOutputAddress` when it has one. A missing signal or type blocks Identify with the configuration requirement. No `AT %M` mapping is needed for Control Studio.
- **A manual output that copies the output.** When the code copies the output into a manual output, the loop declares `controller.manualOutputFollowsOutput`. The manual output is then a bounded state (see Apply and Restore).
- **Identification.** Identify chooses the step size and how long to hold it, within the limits (B3.3, B3.4). The engineer sets the maximum duration in **Limits…**; it is never assumed silently (B3.2).
- **Seeded values.** Validate starts the program from seeded values: the setpoint, the manual output, the mode, the parameter record and, through the initialization adapter, the PID's state.
- **Initialization adapter.** A PI or PID loop declares one initialization adapter under `controller.pidInitialization`: `control-pid-v1` or `parallel-pid-v1`, described under Apply and Restore. A P-only loop uses the adapter of C13.
- **Application states.** When the application supervises its startup, `controller.pidInitialization.applicationStates` declares its named application states, for example a completed-startup flag. Each uses the scalar contract of C0. They accompany the library states in Validate and in commissioning. They stop startup code from overwriting the reviewed controller history before the first `Update`. A flag is never guessed from its name. A looser response requirement never replaces a missing initialization.

#### Identify

- Identify follows B3. The input is the manual output (or the plant input). The output is the PID's own measurement.
- In Manual, the excitation is a manual-output step. A manual output that the program copies from the output is stepped in Manual only, where the program does not write it. In Auto with a declared setpoint, Automatic uses a setpoint step (B3.3).
- The model is one of the forms of B3.5. A model run gives the model's measurement to the PID through the feedback target `pid:<instance>` (Specification 33, section 13.2). The measurement then reaches the controller however the program computed it.

#### Tune

- A candidate is P, PI, PD or PID. For `Control.PID`, Tune calculates the closed loop itself (B4.5). Tune needs no live PID.
- Every method states when it applies. A method outside its domain returns a deterministic reason and creates no candidate.
- Failed data qualification is an Identify failure. It never reaches Tune.
- The following domain failures return a reason and create no candidate: a zero plant gain, a gain whose sign contradicts the controller action (numerical annex N6), non-finite parameters, a non-positive required time constant, an invalid delay, a model error above 0.25, an unsupported model structure, and any other method-specific domain failure.
- The formulas below are written for a positive gain. A loop whose measurement falls when its output rises (`reverse`, as for cooling) has a negative plant gain: Tune evaluates the formulas on the gain's magnitude and gives Kp, Ki and Kd the sign of the gain (numerical annex N6).
- Results that cannot be distinguished are labelled equivalent. No fixed preference exists between SIMC and AMIGO.

**SIMC PI** (Skogestad, 2003) applies to a stable, monotone first-order or first-order-plus-dead-time model `K·exp(−L·s) / (T·s + 1)`, with K ≠ 0 (written below for K > 0), T > 0 and L ≥ 0. In the parallel convention it calculates:

```text
Kp = T / (K·(λ + L))
Ti = min(T, 4·(λ + L))
Ki = Kp / Ti
Kd = 0
```

**SIMC for an integrating model** also applies to an accepted positive-rate-gain model `K′·exp(−L·s) / s`, with K′ > 0 and a resolved effective delay L > 0. K′ has controlled-variable units per manipulated-variable unit per second. K′ MUST NOT be displayed or used as a self-regulating steady-state gain. In the same convention it calculates:

```text
Kp = 1 / (K′·(λ + L))
Ti = 4·(λ + L)
Ki = Kp / Ti
Kd = 0
```

**Response choice.** The declared response selects λ:

- Faster: `λ = max(L, T/3)`;
- Balanced: twice the Faster value;
- More robust: four times the Faster value.

For an integrating model the Faster value is its effective delay L. Balanced and More robust use the same factors. An integrating model without a positive effective delay has no response-time scale. It returns an unavailable reason. The three choices change only λ. They do not change the limits or the acceptance requirements. The numeric λ is shown and saved with the candidate.

**AMIGO PI** (Åström and Hägglund, 2004) applies to an accepted stable, monotone FOPDT model with K ≠ 0 (written below for K > 0), a positive dead time L and `0 < L/T ≤ 1`:

```text
Kp = 0.15/K + (0.35 − L·T / (L + T)²)·T / (K·L)
Ti = 0.35·L + 13·L·T² / (T² + 12·L·T + 7·L²)
Ki = Kp / Ti
Kd = 0
```

AMIGO has no response-speed parameter. The response choice changes the comparison and the ranking, not these coefficients. The interface MUST say so. AMIGO is unavailable for integrating models.

**Translation to the parallel record.** Formula results are ideal (ISA time-constant) PI values. They are translated to `ProportionalGain = Kp`, `IntegralGain = Kp / Ti` and `DerivativeGain = 0`. No series, incremental, positional, vendor or per-scan gain conversion is permitted. Tune MUST NOT bake a nominal scan interval into `Ki`. The runtime passes the actual cycle interval to `PID.Update`.

**Automatic.** Automatic designs the selected structure to the declared requirements.

- For a formula with a response choice, Automatic uses the choice nearest the declared one whose predicted response, from the settling-time deadline to the end of the test, stays within the allowed error of the target and within the settling band of its final value. Of two equally near choices, the more robust one is used. The Tune page fixes the declared choice for Automatic at More robust, whether or not criteria were entered.
- When the requirements declare an input disturbance, each prediction covers it. From the operating point, with the setpoint unchanged, the disturbance is added to the plant input at the start. The output must then stay within the allowed error of the target and within the settling band of its final value, from the deadline to the end of the test.
- When no choice meets the requirements, and the model is delay-free, first or second order and has no zeros, Automatic places the closed-loop poles with damping 0.8. The first decay rate is `max(4, ln(|Δr|/b)) / Tₛ`. The response envelope then reaches the settling band `b` from the setpoint change `Δr` by the settling time `Tₛ`, and never decays more slowly than the 2 % envelope. Each further placement is 25 % faster. At most eight placements are tried, all with `ωh < 0.2` (the sample-rate limit). A structure with fewer gains than the closed-loop poles places what it can: P on a second-order model matches the natural frequency and keeps the model's own damping; PI and PID on a second-order model place the third closed-loop pole at three times the natural frequency. The controller's zeros can make the response faster or less damped than the envelope, so each placement is checked by its prediction. The first placement with non-negative gains whose predicted response meets the requirements is used. Otherwise the structure is rejected. The rejection gives the deviation of its fastest predicted response, the deadline, and the limit it misses.
- Without a model prediction, the declared choice is used and Validate decides. SIMC or AMIGO selected explicitly calculates its formula unchanged.

**Constrained comparison.** Around the qualified SIMC candidate, Tune evaluates a fixed neighbourhood:

- the formula PI;
- a less aggressive PI with `0.8·Kp` and `1.25·Ti`;
- P, by removing the integral action;
- PD, by removing the integral action and adding `Kd = Kp · min(max(L, 0.05·T), 0.25·T)`;
- PID, by adding that same derivative term to the formula PI.

Each non-formula candidate is labelled Constrained comparison. All candidates use the same validation and ranking. The comparison produces no candidate when none passes the mandatory requirements.

**P and PD.** For an accepted integrating model, proportional control stays eligible, because the plant supplies integral action. With a positive delay L, P uses the SIMC proportional gain above, with the same response choices as PI. Automatic predicts it on the model, with the declared input disturbance, and refuses it with the shortfall of that prediction. PI, PD and PID stay eligible only where they apply, and each additional term must give a material benefit.

For the stable first-order PD alternative, the steady error is `|ΔR + K·u₀| / (1 + K·Kp)`, where `K` is the first-order model's gain and `u₀` the operating-point input. Tune rejects a non-positive closed-loop DC denominator. It also rejects an error above the declared error allowance minus the transient margin. A PD candidate is not rejected merely because it has no integrator. Dynamic requirements still need actual-ST validation. In other formula domains, an unsupported P, PD or PID candidate is reported as unavailable, not synthesized from an unqualified rule.

**Candidates and results.** Candidates use the exact PI, PD or PID form, timing, derivative filter, weights, limits, tracking and anti-windup behaviour of Specification 32. The result keeps the winning structure, the method, the limiting requirement, the rationale, the exact formula, λ, the scores, the rejected candidates and the conventions. Every generated candidate stays unapplied.

Formula references: S. Skogestad, "Simple analytic rules for model reduction and PID controller tuning", *Journal of Process Control* 13 (2003), 291–309; K. J. Åström and T. Hägglund, "Revisiting the Ziegler-Nichols step response method for PID control", *Journal of Process Control* 14 (2004), 635–650.

#### Validate

- Validate runs the actual ST of the `Control.PID` instance. Its location follows B5.3: the configured simulated plant when that plant drives the loop's signals, otherwise the qualified model.
- The primary PID journey executes the real `Control.PID`. A handwritten substitute, or a mathematical prediction shown as executed ST, is not allowed.
- Validation starts from the seeded values of Setup. It runs the B5.3 scenarios: setpoint step, load step, noise and model variation. For Auto commissioning it also runs the Manual-to-Auto entry. That run checks the first output under A6.4, the actual mode, the gains and the signal bounds.
- Actual-ST evidence MUST confirm the whole parameter, state and mode path. The fixed parameter values are recorded and checked through every target response. An ST overwrite therefore cannot qualify a different controller.
- A required scenario that does not complete is Inconclusive.

#### Apply and Restore

The transaction members are:

- the complete `Control.PID.ParametersValue` record, as the adapter maps it (C0), including β, Tf and the tracking gain where declared;
- the state initialization of the adapter: the `control-pid-v1` states or the `parallel-pid-v1` integral and derivative states;
- the mode entry to Auto (A6.2). In a cascade, the other controllers' modes too (C6).

**Atomic write.** The whole record is validated before one write at the scan boundary is queued. If any field, value, enumeration, instance, authority or expected revision is invalid, no field is written, no pending revision is created, and the active revision is unchanged. A queued revision stays pending until a later scan reads the complete structure back. Control Studio MUST NOT label it applied before that read-back. After the read-back, the adoption of parameters and states is observed for 30 s of process time (A3.2). The previous confirmed revision is retained for Restore.

**Bumpless entry.** The Manual-to-Auto entry sets `align_output` to true. Control Studio then calculates the retained states for first-output continuity, as described below. The later target change sets `align_output` to false. That transaction keeps the retained PID state and allows the reviewed target response. It still confirms the actual ST state evolution at the task boundary.

Startup initialization MUST NOT overwrite an isolated candidate or accepted live settings.

**Restore previous settings** returns the parameter record and the mode from before Apply, through a guarded transaction (B6.4). The live state is initialized from current observations. The old integral and filter history is not replayed. **Restored** is shown only after runtime confirmation.

##### The `control-pid-v1` state initialization

The loop declares `controller.pidInitialization` with `kind: "control-pid-v1"`, `states` and `maximumSnapshotAgeMs`. Seven states bind the controller instance's members:

| Member | Initialized at a stationary operating point to |
|---|---|
| `StateValue.Setpoint` | the measured process coordinate |
| `StateValue.SetpointRate` | zero |
| `StateValue.ProcessValue` | the measured process coordinate, unless it is the loop's measurement (below) |
| `StateValue.ProcessValueRate` | zero |
| `StateValue.FeedForward` | the application's declared initial feedforward value |
| `StateValue.NominalOutput` | the applied actuator coordinate |
| `StateValue.SaturatedOutput` | the applied actuator coordinate |

- Each state is an `LREAL` with a unit, finite bounds, an initialization source and a bounded post-scan observation. These are the declared-state scalar descriptor of C0. Missing, duplicated, incorrectly typed or incorrectly owned states are rejected.
- When the loop's measurement is `StateValue.ProcessValue`, that state is the measurement. A model run gives it to the PID. The transfer reads it at the task boundary and never initializes it. The transfer then initializes the other six states.
- All seven states are checked in the actual-ST Validate and in its mandatory state bounds. Initialization and live commissioning reuse the declared-state task-boundary transaction, its first-output observation, its cancellation and its recovery. Control Studio never copies the controller's equations or replaces the library block with a second transaction service.
- This adapter and `parallel-pid-v1` describe different retained-state semantics. A loop declares one of them.

##### The `parallel-pid-v1` initialization

```json
{
  "kind": "parallel-pid-v1",
  "integralState": { "symbol": "Main.Loop.Integral", "iecType": "LREAL", "unit": "<output unit>", "minimum": -100, "maximum": 100 },
  "derivativeState": { "symbol": "Main.Loop.PreviousWeightedError", "iecType": "LREAL", "unit": "<process value unit>", "minimum": -100, "maximum": 100 },
  "proportionalSetpointWeight": 1,
  "derivativeSetpointWeight": 1,
  "maximumSnapshotAgeMs": 100
}
```

The example values are illustrative. The adapter declares the retained integral state and the derivative (filter-history) state, with their IEC types and ranges. It declares both setpoint weights: `b` for the proportional term and `c` for the derivative term. It also declares how fresh the single task-boundary snapshot must be.

From one snapshot of the applied output `u₀`, the process value `y₀`, the reference `r₀`, both retained states and the timestamp, with the candidate gains `Kp`, `Ki` and `Kd`:

```text
weighted proportional contribution:  Kp·(b·r₀ − y₀)
derivative history:                  c·r₀ − y₀        (so the first derivative contribution is zero)
integral state:                      u₀ − Kp·(b·r₀ − y₀) − Ki·(r₀ − y₀)·Δt
```

The integral is updated once before the first output is calculated, so the integral state carries the `Ki·(r₀ − y₀)·Δt` term. The owning runtime applies the complete state and the mode at one task boundary. It confirms that the first actual output is within the A6.4 allowance or within a reviewed tolerance. It retains the snapshot and the applied state as evidence. The adapter is rejected if any declared state, unit, range, weight, freshness bound or law binding is incompatible. It never guesses ST variable names, and it never treats a parameter write as a state initialization.

##### Manual output that follows the output

When `controller.manualOutputFollowsOutput` is declared, the program copies the output into the manual output in every Auto scan. The manual output is therefore a bounded state. It changes at the scan of the Auto entry, and between the reviewed snapshot and the task boundary of a transfer to Manual. The runtime accepts the declared bounded state within its bounds, and the manual output is written from the held output in either case.

- The held output is the last committed I/O image for an output with an address.
- For an output without an address, the held output is its value at the task boundary: the value that the last scan drove the plant with. An uncommitted write is never the held output.
- When identification returns the loop to Auto, the references return to their values from before identification (B3.4). A manual output that follows the output is not written back. Auto overwrites it, and writing it back would move the output.

### C2. Program-owned P, PI or PID law (the parallel-pid seed)

The program computes the law itself, in parallel form, with its gains in program variables. Control Studio sets those variables through the parameter binding (C0).

#### Setup

- The setup asks for these items:
  - the program instance;
  - its measurement, output and setpoint;
  - its Auto/Manual signal and manual output;
  - the law: P, PI or PID;
  - for each gain, the variable that holds it, with the range Tune may choose from;
  - for PI and PID, the variable that holds the integral term;
  - for PID, the variable that holds the previous error or the previous measurement that the derivative uses, and which of the two it is;
  - the startup flag (C1); identification sizes and durations are chosen by Identify (B3.3, B3.4).
- The law is `u = Kp·e + I + D`, with `I = u₀ + Ki·∫e dt`, and `D = Kd·de/dt`. When the derivative acts on the measurement `y`, `D = −Kd·dy/dt`. `e` is the control error in the measurement's unit. `I` is in the output's unit. Time is in seconds. Each scan updates the integral as `I_new = I_old + Ki·e·Δt`.
- The loop is a project-owned controller (`design: project`). Its `parallel-pid` seed names the gain variables of the law.
- A law with reset or derivative times, a derivative filter or further retained states uses the step-by-step setup.

#### Identify

As C1. The startup flag is set as for a PID; Identify chooses the step and its hold (B3.3, B3.4).

#### Tune

- Tune designs the gains from the model as for `Control.PID` (C1).
- It offers only the families whose gains the program has.
- The margins of C0 apply to the law's linear form. The result is checked on the actual ST (Validate).

#### Validate

- Validate runs the program's actual ST. Its location follows B5.3.
- Validation starts the integral term at the output that the loop has when it switches to Auto, with the setpoint at the measurement. The switch is therefore bumpless.
- The previous value starts at the error of that moment (zero) or at the measurement. The derivative therefore starts at zero.

#### Apply and Restore

- Apply writes the gain variables through the parameter binding. It starts the integral term and the previous-value variable as Validate does, and switches the loop to Auto, in one transaction (B6.1).
- The Auto-entry allowance is that of A6.4, with the law's own gains.
- Restore previous settings returns the gains and the mode from before Apply (B6.4).

### C3. Switching (ON/OFF) controller

The output is binary and switches between two levels, with hysteresis. The controller is project-owned (`design: project`). It has no PID adapter, so the generic PID Apply is not offered.

#### Setup

- The setup asks for these items:
  - the program instance that runs the controller;
  - its measurement and setpoint;
  - its numeric output, with the low and high levels it switches between;
  - its Auto/Manual signal and its manual output;
  - the three settings that Tune designs (below).
- **Hysteresis.** The hysteresis is in the measurement's unit. The convention is declared: half-band, where the value is the half-width of the band around the setpoint, or full-band, where it is the full width.
- **Minimum times.** The minimum ON time and the minimum OFF time are in seconds.
- **Equipment limits.** The switched equipment's own minimum ON and OFF times are asked for when the equipment has them, for example a restart protection. A blank value means the equipment has none. The values are declared as the output channel's actuator dwell limits. Tune never designs shorter minimum times.
- **Retained states.** The output latch starts ON when the output starts at its high level. A dwell timer, in seconds, is optional. It starts at zero.
- A switching controller whose output is `BOOL`, or that has other retained states, uses the step-by-step setup.

#### Identify

- Identification uses the actuator's full binary levels. Its timing changes between independent records, so that validation has data that identification did not use (B5.2).
- The design inputs of Tune are the measurement's noise and the process band. Tune does not use a fitted model for them.

#### Tune

- Tune selects the threshold and the hysteresis from the measurement's noise and the process band. It selects the minimum ON and OFF times from the process requirements. It never designs them shorter than the equipment's dwell limits.
- Tune proposes these values only from the declared actuator ratings, the process bounds and the measurement uncertainty. Unknown equipment limits are requested from the engineer. They are never guessed.
- The requested state changes when the measurement crosses a threshold on the side that the declared action requires. Inside the hysteresis band the requested state keeps its previous value. The threshold-equality policy is declared.
- The applied state changes only when the dwell condition holds: the time since the last change is at least the dwell.
- The candidate is generated in the controller's own conventions, and it is then run on the actual ST (Validate).
- No margins are shown for this law (C0).

#### Validate

- Validate runs the actual ST on the selected model. Each run starts the plant at a declared level, even when the equilibrium input lies between the two output levels.
- The scenarios are the events of B5.3 and a state-recovery scenario that perturbs the initial plant state once.
- The criteria are these:
  - every recorded transition obeys the switching rule;
  - every completed normal interval is at least the dwell;
  - the applied command is binary;
  - the retained request stays inside its band;
  - an abort is recorded separately and never counts as a normal dwell completion.
- Output levels and completed dwell intervals are assessed by the shared requirement evaluator. A missing complete interval is Inconclusive (B5.4).

#### Apply and Restore

- Apply writes the hysteresis and the minimum times, and switches to Auto, in one task-boundary transaction (B6.1). The latch and the dwell timer are initialized as declared (C0).
- The Auto-entry allowance differs from A6.4. A binary output has no band. At the entry, the latch starts at the output's applied level and the dwell timer starts at zero. The output therefore keeps its level until the dwell has elapsed.
- Restore previous settings returns the settings and the mode from before Apply. It initializes the latch and the dwell timer from current observations (B6.4).

### C4. State feedback (pole placement, LQR) and LQG

The law is `u = u₀ + N·(r − y₀) − K·(x − x₀)` in identified measured coordinates. LQG replaces the measured states `x` with estimates `x̂`.

#### Setup

- The setup asks for these items:
  - the program instance that runs the law;
  - the plant input, with its unit and range;
  - the controlled output, with its unit, range and setpoint;
  - one to three further measured outputs, each with its unit and range.
- Each measured output is a state of the model.
- For each measured output, the setup asks for the variable that holds its gain, in the plant input's unit per that output's unit. For the controlled output it also asks for the variable that holds the reference gain `N`.
- The program applies `u = u₀ + N·(r − y₀) − Σᵢ Kᵢ·(yᵢ − yᵢ₀)` around the operating point at which the plant is identified.
- The Auto/Manual signal, the manual output and the startup flag are set as for a PID (C1); Identify chooses the step and its hold (B3.3, B3.4).
- The loop declares `controller.stateFeedback` with `family` set to `state-feedback`, `LQR` or `LQG`. Its convention is `sampled-deviation-negative-feedback`, meaning `du = −K·dx + N·dr`.
- The ordered `gainSymbols` and `referenceGainSymbols` matrices name the compiled `REAL` or `LREAL` parameter fields. These fields belong to one parameter binding and one task.
- Coefficient units follow the declared input/state and input/controlled-output coordinates. Auxiliary model outputs remain measurements, not extra controlled objectives.

**LQG adds the following.**

- The program estimates the states from the controlled output with a fixed model, in deviations from the model's operating point:

  ```text
  x̂[k+1] = A·x̂[k] + B·u[k] + L·(y[k] − C·x̂[k])
  ```

  The estimates feed back through `K` and `N`.
- The setup asks for the model file in the project that the estimator implements. It is a state-space model with one input, the controlled output as its one output, sampled at the task period.
- For each model state, the setup asks for the estimate variable, its gain and its observer gain (`observerGainSymbols`). It also asks for the process-noise variance of each state and the measurement-noise variance. These variances set the observer's trade-off between trusting the model and trusting the measurement.
- The estimates start at zero, the operating point.
- The binding declares the estimator's fixed `estimatorModel` matrices A, B and C, and its sample period. These MUST equal the selected sampled model. A newly selected plant MUST NOT silently tune an estimator that still runs different model coefficients.
- In this adapter, LQG requires zero direct measurement feedthrough.

#### Identify

- The plant is identified in measured coordinates, first order in each measured output (`identificationPolicy.maximumOrder` 1). Each gain then belongs to one measured signal.
- For LQG, the selected model MUST be the estimator's model. Supply the model file on Identify, or identify a model and write its matrices into the program and the model file before Tune.

#### Tune

- **Pole placement.** Explicit pole targets are discrete poles. A continuous model is converted by the exact zero-order-hold conversion at the actual task period. The gains place the sampled poles.
- **LQR.** LQR weights may be supplied. Automatic weights normalize each coordinate by its declared operating span. The gain is the steady-state gain of the discrete linear-quadratic regulator for the sampled model, from the algebraic Riccati equation.
- **LQG.** Tune designs `K`, `L` and `N` by LQG on the selected model. The design needs supplied process and measurement covariances. The estimator follows the prediction and correction convention above.
- If a required mode is not controllable, Tune reports the infeasibility instead of a candidate.
- A pin keeps explicit coefficients. The mapped sampled poles are recomputed, and any optimality claim is discarded.
- **Margins at the plant input.** For one plant input, the loop is broken at that input:

  ```text
  L = K·(zI − A)⁻¹·B
  ```

  This is evaluated on the model sampled at the task period, with the gains `K` on the fed-back outputs, up to half the sampling frequency. The delay margin is then the extra dead time at the plant input that the loop tolerates. The margin definitions are in C0.

#### Validate

- Validate runs the actual ST (B5.3). The operating range is the intersection of the selected model's operating ranges and the bound signal and actuator limits.
- The state-recovery scenario perturbs the initial plant state once.
- The record enters the same actual-ST validation and Manual staging operations as any other record.

#### Apply and Restore

- Apply writes the gains `K` and `N`, and `L` for LQG, through the parameter bindings. It switches to Auto in one transaction (B6.1).
- Apply starts from the validated operating point. Every measured output lies within its settling band `bⱼ` of that point (B6.2). With several actuators, the coupled transfer of C0 applies.
- **Auto-entry allowance.** The allowance differs from A6.4. A state-feedback law has no retained state that makes the Auto entry bumpless. At entry each reference follows its measurement, so the first output moves by the law's response to the process's deviation from the operating point. Unless a reviewed tolerance is set, the first-output check allows, for plant input `i`:

  ```text
  Σⱼ |Kᵢⱼ|·bⱼ + Σₖ |Nᵢₖ|·bₖ
  ```

  Here `j` runs over the model states and `k` over the controlled outputs. A state is a measured output when its column of `C` selects exactly that output, and `bⱼ` is that output's settling band. A reviewed tolerance replaces this allowance. A model with any other state keeps the exact check.
- Without a typed Auto state-transfer adapter, the mapping alone never grants Auto authority. In that case Manual staging is offered explicitly.
- Restore previous settings returns the gains, the reference gains and the mode from before Apply (B6.4). It restores the original reference vector and the parameter record (C0).

### C5. Laws tuned by a finite search over coefficient variables

The law is a program law whose coefficients Control Studio has no design formula for. Examples are MPC weights, a ratio, split-range break points and override limits.

A notch component of such a law is the discrete second-order section with its zeros at the centre frequency `f`: with `ω = 2π·f·h` (task period `h`, `0 < ω < π`) and `α = sin(ω)/(2Q)` for the quality factor `Q`, `y[n] = b0·u[n] + b1·u[n−1] + b2·u[n−2] − a1·y[n−1] − a2·y[n−2]` with `b0 = b2 = 1/(1 + α)`, `b1 = a1 = −2·cos(ω)/(1 + α)` and `a2 = (1 − α)/(1 + α)`. Its gain is zero at `f` and one at zero frequency and at the Nyquist frequency; its denominator must be strictly stable.

#### Setup

- The setup asks for these items:
  - the program instance that runs the law;
  - its measurement, output and setpoint;
  - its Auto/Manual signal and manual output;
  - the kind of law;
  - one to four coefficient variables. For each one, the setup asks for its unit and the explicit list of values that Tune may try.
  - up to three values that the program keeps from scan to scan, for example an integral term or a filter state. Each has its unit and the start that validation uses: zero, the output or the measurement;
  - the objective, which names the reference symbol, the feedback symbol and the error unit;
  - the startup flag, as for a PID (C1); Identify chooses the step and its hold (B3.3, B3.4).
- A pinned setting keeps its value. It is not part of the grid.
- The coefficient and gain variables MUST be `REAL` or `LREAL` and unaddressed (C0). Initial values and events MUST NOT target a swept variable. A duplicate or ambiguous write is rejected. A `BOOL` value of 0 or 1 may start a project-owned sequence.

#### Identify

As C1. The search runs on the selected model.

#### Tune

- The grid is the Cartesian product of the value lists. The trial count MUST equal that product. It MUST NOT exceed 64 (A7).
- Each trial is one combination. It runs the project's actual ST against the selected model, in an isolated simulation with complete resets. Every trial uses the same initial values, the same plant initial state, the same events, time points and seed. A search never runs against the selected physical runtime.
- Bounds are checked before ranking. A trial that fails a constraint does not qualify. Missing or bad evidence never qualifies. Only qualifying trials are ranked.
- **Selection.** Keep the combinations that meet every mandatory requirement. Choose the one with the smallest error after the settling time. The error is the trapezoidal sampled integral of squared error, in error-unit squared times seconds, with the actual step duration as the sample interval:

  ```text
  J = Σₖ ½·(eₖ² + eₖ₊₁²)·Δt
  ```

- Of equal errors, the first combination in the listed order is used.
- A `REAL` variable's values are rounded to the nearest `REAL` before the trial runs.
- The search stops on any change of the program revision. Every attempted trial is retained. A trial that is not attempted, after a cancellation or an exhausted budget, stays marked as not attempted. Nothing is truncated silently.
- The trial and job budgets follow A7.

#### Validate

Validate follows B5.3 for the chosen candidate. The trials of the search are Tune evidence on the selected model.

#### Apply and Restore

- A search result is never applied implicitly. Selecting a trial populates the parameter review only when every declared tunable and fixed parameter is explicitly present in that trial's grid or initial conditions.
- Apply writes the chosen coefficient values through the parameter record (C0). It switches to Auto in one transaction. The retained values start as the setup names them, zero, the output or the measurement, as the validated trial started them.
- No first-output formula exists for this law, so Auto entry requires a reviewed tolerance (A6.4).
- Restore previous settings returns the previous coefficient values and the mode from before Apply (B6.4).

### C6. Cascade

A cascade has two controllers in series: the outer controller's output is the inner controller's setpoint. There are two forms. The one-loop form is one loop with two `Control.PID` instances. The two-loop-file form is two loop files, each with a `cascade` declaration.

#### Setup

- **One-loop form.** The Cascade structure of the setup writes one loop for two `Control.PID` instances. The inner controller drives the plant input and measures the inner measurement. The outer controller measures the outer measurement against the outer setpoint, and its output is the inner setpoint. Setup recognizes a cascade when one PID's output is the only source of another PID's setpoint (B1.3).
- **Two-loop-file form.** Each loop declares `cascade: { role, partnerLoopId }`, with role `outer` or `inner`.
  - The outer loop's manipulated variable is its controller output, in the inner measurement's unit and range.
  - The inner loop's setpoint is the outer controller's manual output.
  - The inner declaration's `referenceHold` names the outer mode signal, its Manual value and the outer manual output.
- **Modes.** The one-loop form binds the outer controller's mode as an `additional_modes` member of the transfer (see Apply and Restore).

#### Identify

- **One-loop form.** The inner controller is put in Manual, and its manual output is stepped. One model is fitted from the plant input to both measurements. It is first order in each measurement (`identificationPolicy.maximumOrder` 1), so the outer measurement follows the inner one as in the process.
- **Two-loop-file form.** Each loop is identified, tuned, validated and applied through the ordinary workflow, the inner loop first. The outer loop is identified with the inner loop applied and in Auto. Its model therefore includes the closed inner loop.

#### Tune

- **Inner loop first.** Automatic (C1) designs the inner controller on the path from the plant input to the inner measurement, for the inner requirements.
- **Outer loop second.** Tune predicts, on the model, the closed inner loop's response of the outer measurement to a step in the inner setpoint. It reduces that response to first order with delay by the two-point method (C7). It then designs the outer controller with Automatic, for the outer requirements.
- Without declared inner requirements, the inner loop is designed to settle in one fifth of the outer settling time. The outer settling band is scaled to the inner measurement's range. The outer design's assumption that the inner measurement follows its setpoint then holds.

#### Validate

- **One-loop form.** Validate runs the actual ST of both controllers in Auto, from the model's operating point, with the outer requirements. The run includes a load at the plant input that both loops reject. This validation always runs on the model.
- **Two-loop-file form.** The inner loop is validated with the outer controller held in Manual at the target, through `referenceHold`. Its location follows B5.3. The outer loop is validated on its model only, never on the project's configured plant. That run would start the program from its declared values, with the inner controller in Manual and away from the operating point that the outer model was identified around.

#### Apply and Restore

- **One-loop form.** One task-boundary transaction writes both parameter records and switches both controllers to Auto. The outer controller's mode is an `additional_modes` member of the transfer. `additional_modes` takes up to eight further controllers' modes. The modes are observed like `mode`, written with it at the same boundary (Auto when the transfer leaves Manual, Manual for a Manual transfer), and confirmed after the first scan.
- The inner controller starts at the present plant input, with its setpoint at the inner measurement. The outer controller starts with its output at the inner measurement. Neither switch bumps.
- Restore previous settings returns both parameter records and both controllers to Manual, in one transaction.
- **Two-loop-file form.** The inner loop is applied only while the outer controller is in Manual. Apply refuses before it waits or writes, with the reason "Put the outer controller in Manual before applying". The inner transfer guards the outer mode. The runtime therefore also refuses a switch of the outer controller to Auto before the task boundary.
- The outer loop is applied through its own ordinary workflow.

### C7. PID group (decentralized PI) and static decoupler

A PID group is two or more `Control.PID` instances whose processes interact. Each member's output moves several measured outputs. The group contract `controller.group` pairs each member's input channel with one output channel, and shares one Auto/Manual signal. The members therefore change mode together.

#### Setup

- The setup offers **PID group** when the program has two or more library PIDs. The engineer selects the members. For each member the engineer confirms the compiled measurement, output, setpoint, manual output and parameter record, as for a single loop.
- The group has one shared Auto/Manual signal. Each member's input channel may declare its own identification step (`experimentStep`); otherwise Identify chooses it (B3.3). The maximum duration comes from the limits.
- Setup writes one group loop. Its model channels follow the member order.
- **Decoupler (when the program has one).** The setup asks for the decoupler's variables `D_ij`, for plant input `i` and member `j`. The unit of `D_ij` is plant input `i`'s unit per member `j`'s unit. In Auto the program writes `u = D·v`, where `v` are the members' `Control.PID` outputs. In Manual it writes `u = v`, so that identification and Manual operation act on the plant itself.

#### Identify

- The group is identified in one recording. Identify runs one experiment per member output within that recording. Each input is stepped by its own identification step. The result is one multivariable model.
- With a decoupler, the identification is the same, in Manual.

#### Tune

1. **Steady-state gains and the relative gain array.** Calculate the steady-state gain matrix `K` (outputs × inputs) and the relative gain array `Λ = K ∘ (K⁻¹)ᵀ` for the members' pairing. `Λ` needs a square, non-singular `K`. A pairing whose diagonal relative gain is not positive, or whose `K` is singular, is refused. Tune names the pairing with positive diagonal relative gains closest to 1.
2. **Two-point reduction.** For each member's own path `yᵢ/uᵢ`, let `t₂₈` and `t₆₃` be the times at which the step response reaches 28.3 % and 63.2 % of its final change. Then:

   ```text
   τ = 1.5·(t₆₃ − t₂₈)
   θ = t₆₃ − τ          (if θ < 0: θ = 0 and τ = t₆₃)
   ```

   The two-point reduction is also used by C6, C8 and C9.
3. **Effective gain and SIMC.** With the other loops closed, member `i` sees the effective steady-state gain `Kᵢᵢ / λᵢᵢ`. SIMC (C1) designs the member's PI for that gain, for the path's time constant and delay, and for the member's own requirements.

The result names, for each member, its path model, its `λᵢᵢ` and the resulting gains.

**Static decoupler.**

- Tune calculates `D = K⁻¹·diag(K)`, from the steady-state gains `K` of the group model. Through the decoupler, each member's output moves only its own measurement in steady state, with the gain of its own path. A singular `K` is refused.
- Each member's PI is designed as in step 3, on the model seen through the decoupler. Its relative gains are then one.
- The decoupler corrects steady-state interaction only. It does not cancel dynamic interaction.
- Tune shows `D` and the per-member design.
- Centralized designs (C4) are separate structures.

#### Validate

- Validate runs the actual ST of all members against the full multivariable model. It uses every member's requirements, and each setpoint alone. This validation always runs on the model.
- With a decoupler, the decoupler is part of the run.

#### Apply and Restore

- **Group.** Apply writes all members in one transaction with one rollback. Only the group changes. Other PIDs' parameters and retained states do not change. A group update is never presented as independent PID writes.
- **Decoupler.** Apply writes the PI gains and `D` in one transaction, and switches the group to Auto. Each member's PID starts at its share of the present plant inputs, `v = D⁻¹·u`, using the decoupled-output initialization of C0. The product `u = D·v` therefore does not change at the switch.
- Restore previous settings returns all of them, and the group to Manual (B6.4).

### C8. Measured-disturbance feedforward

The program adds a feedforward from a measured disturbance `d` to the `FeedForward` input of a `Control.PID`.

#### Setup

- The setup asks for the single-loop fields, and also for these items:
  - the measured disturbance: its compiled variable, unit and range;
  - the variable that Control Studio steps to move the disturbance during identification, and the step. In a simulation this is usually the simulated load itself;
  - the program's gain variable `K_ff`;
  - optionally, the lead time `T_lead`, the lag time `T_lag` and the filtered disturbance `z` that the program keeps. These three are all present or all absent.
- The program computes:

  ```text
  FeedForward = K_ff · (z + (T_lead / T_lag)·(d − z))
  ```

  where `z` is `d` filtered by a first-order lag with time constant `T_lag`. This is the lead/lag `K_ff·(T_lead·s + 1)/(T_lag·s + 1)` applied to `d`, with unit steady-state gain. Without lead and lag, `FeedForward = K_ff·d`.
- The disturbance is measured. The controller never writes it.

#### Identify

- The PID is put in Manual. The manual output and then the disturbance variable are stepped. Each has its own interval in one recording, as the inputs of a PID group do (C7).
- The disturbance is stepped through its declared write binding. Such a binding exists only for an explicitly actuated simulation or test input.
- One model is fitted from both inputs to the measurement. It is of at most second order (`identificationPolicy.maximumOrder` 2), so that each path keeps its own time constant.
- The fit MUST agree with an independent recording within 5 % (`identificationPolicy.maximumNormalizedError` 0.05). A first-order fit that shares one time constant between the two paths is not accepted.

#### Tune

- Automatic designs the PI (C1) on the path from the controller output to the measurement, for the requirements.
- The feedforward is designed from both paths. Each path is reduced to first order with delay by the two-point method (C7): `(K_u, τ_u, θ_u)` from the controller output, and `(K_d, τ_d, θ_d)` from the disturbance.

  ```text
  K_ff   = −K_d / K_u
  T_lead = τ_u
  T_lag  = τ_d + max(0, θ_d − θ_u)
  ```

- The lead cancels the lag of the output's path. The lag reproduces the disturbance's path. A disturbance delay longer than the output's delay is approximated as further lag. When `θ_d < θ_u`, the disturbance reaches the measurement before any action can. That part of the deviation remains for the feedback.
- Tune shows both path models and the three values. On the model it shows the largest deviation of the measurement after a disturbance step equal to the identification step. It shows this with the PI alone and with the feedforward.
- The feedforward acts outside the loop. The margins of C0 are therefore those of the PI on the output path.

#### Validate

This validation always runs on the model. It covers the single-loop cases of C1, and a measured-disturbance case. In that case, from the operating point with the setpoint unchanged, the disturbance steps by its identification step at the start. The measurement MUST stay within the error allowance by the settling time.

#### Apply and Restore

- Apply writes the PID gains and the feedforward values in one transaction, and switches to Auto. The `FeedForward` state starts at `K_ff` times the measured disturbance. `z` starts at the measured disturbance. The switch therefore does not bump.
- A measured disturbance that is a model input is read as a measurement, never as an actuator (C0).
- Restore previous settings returns all of them (B6.4).

### C9. Smith predictor

The program feeds the controller `y + c` instead of the measurement `y`. The correction `c` is a value that the program keeps.

#### Setup

- The setup asks for the single-loop fields, with the measurement being the process measurement `y`.
- It also asks for the program's variables for the model gain `K`, the time constant `T`, the delay `θ` in seconds, and the correction `c`.
- The correction obeys:

  ```text
  T·dc/dt = K·(u − u_θ) − c
  ```

  where `u_θ` is the controller output `θ` seconds earlier. With a correct model, `y + c` is the measurement that the process would give without its delay. Equivalently, `y + c = T₀ + (y − T_L)`, where `T₀` is the delay-free model's output and `T_L` the delayed model's output.
- **History.** The program keeps the controller output over the delay. Before the controller's update, while the controller is not in Auto and on its first scan, the history is filled with the output that the loop is applying. So `c` is zero whenever the output has been steady. The first Auto update's change of the output then enters the history.

#### Identify

As one loop (C1). The step response from the controller output to the measurement is reduced in Tune.

#### Tune

- Reduce the model's step response from the controller output to the measurement to first order with delay, by the two-point method (C7). Write its gain, its time constant and its delay into the program's model variables `K`, `T` and `θ`.
- Design the PI with Automatic (C1) on the model without its delay. The settling time used is the requirement's settling time less the delay. A settling time that is no longer than the delay is refused, with that reason.
- Show the model values and the gains.

#### Validate

This validation always runs on the selected model. It covers the single-loop cases of C1, run as actual ST.

#### Apply and Restore

- Apply writes the PID gains and the model values in one transaction, and switches to Auto with `c` at zero.
- The Auto-entry allowance is that of A6.4, applied to the PI on the measurement.
- Restore previous settings returns all of them (B6.4).

### C10. Gain scheduling over declared operating points

The program sets the proportional and integral gains of a `Control.PID` each scan, from a schedule table. The gains are interpolated between declared operating points by a scheduling variable, such as the measurement.

#### Setup

- The setup asks for the single-loop fields, and for two to four operating points. Each point has a name, and the program's proportional and integral gain variables for that point.
- Setup writes one loop per operating point, named after the loop and the point. Its parameters are that point's two gain variables. This is the `parallel-pid` seed of C2.

#### Identify

At each point, bring the process to that operating point in Manual first, then identify. The model then describes the process at that point.

#### Tune

Automatic designs each point's PI on that point's model (C1). Tune recommends scheduling only when one fixed design fails, or is inadequate across the declared range.

#### Validate

This validation always runs on that point's model. It runs the actual ST with the program's schedule active, from the operating point. The gains that the program interpolates as the process moves are therefore the ones validated. The active gains MUST change continuously across the schedule. A hidden or discontinuous schedule fails.

#### Apply and Restore

- Apply writes the point's gain variables and switches to Auto. The start of the integral term follows C2.
- Restore previous settings returns them (B6.4).

### C11. Trajectory feedforward

The program moves its reference along a planned trajectory `r(t)` when the setpoint changes, for example a motion profile. It adds a feedforward through the `FeedForward` input of `Control.PID`.

#### Setup

- The program computes the planned rate `ṙ` and the planned acceleration `r̈`. The feedforward is:

  ```text
  FeedForward = K_v · ṙ + K_a · r̈
  ```

- The project controller declares the `trajectory-feedforward-pid` seed. It names the PID's three gains, the program's rate gain `K_v` and its acceleration gain `K_a`. That is five distinct bound parameters.

#### Identify

As C1. The feedforward is designed from the same model.

#### Tune

- The PID for the chosen family is designed as Automatic designs `Control.PID` (C1).
- For a model `n / (a·s² + b·s)` from controller output to measurement, an integrator with a lag when `a ≠ 0`:

  ```text
  K_v = b / n
  K_a = a / n
  ```

  With these values the model follows the trajectory exactly. For example, for a torque-to-angle model `1 / (s·(J·s + B))`, `K_v = B` and `K_a = J`.
- A model of another form has no trajectory-feedforward rule, and Tune says so.
- A delay is not anticipated. Its effect remains for the feedback.
- The feedforward acts outside the loop. The margins of C0 and the Auto-entry allowance of A6.4 are therefore those of the PID.
- Tune's predicted response runs the program with the current values and with the tuned values (B4.5).

#### Validate

Validate runs the actual ST in the single-loop cases of C1. Its location follows B5.3. The setpoint change starts the program's trajectory.

#### Apply and Restore

- Apply writes the PID gains and both feedforward gains in one transaction, and switches to Auto. The planned rate and acceleration are zero at rest, so the switch does not bump.
- Restore previous settings returns all five values.

### C12. Open-loop experiment loops and project-scripted experiments

This structure has no controller. No controller is designed or written, so the Tune, Validate and Apply and Restore stages do not apply. The restoration of the experiment's own outputs is part of the experiment, as described below.

#### Setup

- **Topology.** The loop declares `topology: "open-loop"`. It has a process variable and a manipulated variable, and no setpoint. The loop MUST NOT carry or invent a setpoint, and it exposes no error or setpoint metric. It requires a project-owned experiment or controller.
- **Execution owner.** The loop's execution is one of two kinds:
  - `control-studio`: Control Studio writes the reviewed excitation through the runtime experiment service;
  - `project-scripted`: the project owns the scenario. Control Studio observes it with zero external amplitude, records the declared duration, and writes no duplicate excitation. The project owns the experiment kind and the activation.
- **Activation.** A project-scripted scenario MAY declare one writeable `BOOL` activation binding, named by symbol or by address (A5.1). Without it, the scenario is observation only. When it exists, the authorized start writes it TRUE through the runtime experiment service. Abort, timeout, failure and normal completion restore it FALSE. The restoration is confirmed by the third execution of the loop's task after the restoring write (A7). The plan keeps the activation as the project's authority. It is not SP, not MV, and not a second excitation.
- **Parameter purpose.** A project MAY declare `parameterPurpose: "experiment"`, with a `parameterBinding` and its Boolean activation symbol (`manual_value: false`). This is valid only for project-scripted execution with an activation binding. Parameter updates preserve typed bounds, explicit authority, observed values and rollback through the existing owner. The experiment settings are frozen through review, baseline and execution. That ownership is released only after terminal restoration.
- **Baseline.** A project-scripted experiment MAY declare `baselineSeconds`. The reviewed maximum duration includes the baseline, the held experiment and the restoration.
  - During the baseline the runtime recorder runs with activation disabled. The timed-trajectory service enables the project only after the baseline.
  - The project stays responsible for qualifying its actual measurements before actuation.
  - The baseline and the response samples are saved together. A baseline MUST NOT be rebuilt from nominal values or from a qualification flag.
  - The plan retains the baseline duration. A maximum duration that leaves no sample interval after the baseline is rejected.
- **Hold parameter.** When the engineer chooses how long the project holds its step, the loop names that parameter in `holdParameter`. It is a declared `parameterBinding` parameter in seconds, under `parameterPurpose: "experiment"`. Review reads the applied value from the runtime. It records the baseline, every scan of the hold, and one restoration sample. For example, 5 s + 30 s + 0.1 s = 35.1 s. `durationSeconds` is then the maximum, and it MUST record the longest permitted hold. Review refuses a hold that cannot be read, lies outside its declared bounds, or exceeds that maximum.
- **Maximum duration** follows A7: 1 s to 24 h of process time.
- **Abort.** The loop MAY declare `experiment.abort` alongside `experiment.activation`. The declaration names a writeable `BOOL` binding, by symbol or by address (A5.1), with `direction: "write"`, `iecType: "BOOL"` and `assertedValue` (a boolean). The abort and activation bindings MUST be distinct. Other experiment kinds MUST NOT accept an abort declaration.

#### Identify

- **Excitation.** `control-studio` execution follows B3 and A6.5. `project-scripted` execution is observation only. Its recording covers the declared duration and the project's scenario. Identification of that recording follows B3.5. Identify (Automatic) runs the project's scenario whatever the controller mode; an explicitly chosen method is refused, because the project owns the experiment kind and Control Studio writes no duplicate excitation.
- **Timing.** Experiments follow the timing of B3.4, not a fixed fraction of the duration (A10.2).
- **Review.** The review shows the abort assertion together with the activation and its restoration. Starting an experiment never clears an asserted abort.
- **Terminations.**
  - A normal completion of the duration restores the activation value, without asserting the abort.
  - A user abort, a runtime fault, invalid data (a forced, missing or out-of-range value) or any other failed termination asserts the declared abort input, and then restores the activation.
  - Without an abort binding, a failed termination restores the activation only.
  - The ST program owns the resulting actuator policy. For example, a normal completion may hold an output at 75 %, while an abort latches it at 0 %. The runtime invents no controller-specific policy.
- **Restoration status.** The status of the abort binding is `not_requested`, `pending`, `observed` or `failed`. Restoration stays pending until an unforced scan observes both the asserted abort, when one was asserted, and the restored activation. The restoration is confirmed by the third execution of the loop's task after the restoring write (A7). Missing, mistyped, forced or unobserved data never confirms success.

### C13. Connected P commissioning (proportional-only adapter)

The connected P adapter commissions a P law. The law's parameter record maps the proportional gain. The adapter tracks the output with a reference `r = y + u/Kp`. It applies only to the P family (C0).

#### Setup

- The adapter maps the proportional gain of the parameter record. The integral and derivative gains are written as explicit zero terms (C0).
- The bindings are:
  - the setpoint, which is the tracking reference `r`;
  - the process value `y`;
  - the controller output `u`, as the committed output image;
  - the typed manual command, with compiled numeric types;
  - the mode.
- A legacy loop without type metadata needs configuration. Unsupported scaling, state or plant conditions are explained before any write.
- Live writes follow A2.4 and A2.5. Approval on a simulated target never authorizes physical I/O.

#### Identify

Identify follows B3 for the loop. The model used for tuning is the one selected in Identify.

#### Tune

- **Integrating plant.** For a selected delay-free integrating plant `G(s) = k/s`, the connected P candidate uses `λ = k·Kp > 0`, with the closed-loop pole at `−λ`. In deviation coordinates, `y′ = k·(u − u₀ + d)`, with `u = Kp·(r − y)`. The constant error is `e_inf = (u₀ − d) / Kp`. No controller bias is invented.
- **Bounds.** Bound the separate target and input-disturbance scenarios with:

  ```text
  E_inf = max(|u₀|, |u₀ − d|) / |Kp|
  envelope(T) = (|ΔR| + E_inf)·exp(−λ·T),   T = the time after the change
  ```

  Choose a bounded gain that meets the declared error and settling allowances.
- Supplied and identified integrating models use this same calculation and the actual-ST validation. A candidate's prediction alone never authorizes Apply.

#### Validate

- Validate runs the matched target-response cases and an actual-ST Manual-to-Auto initialization case, on the selected model. The initialization case starts from the selected plant's operating point. It uses the typed manual command, explicit simultaneous reference and mode events, and the candidate gain.
- It checks the first-output continuity under A6.4, the actual mode, the gain and the signal bounds.
- The declared fixed parameter values are recorded and checked through every target response. An ST overwrite therefore cannot qualify a different controller.
- Auto requires this initialization validation. Manual staging remains a supported alternative where it applies. Changing the application mode invalidates the application eligibility, not the model or the tuning.
- Before the Apply action, the exact target, gain, reference, observation duration, transfer tolerance and recovery policy are retained.

#### Apply and Restore

The sequence is:

1. Start the native recording. Stage the candidate gain in Manual, and confirm it.
2. Read the fresh transfer status. Require compatible typed bindings, the verified program and gain, and the selected steady operating point.
3. Compute the tracking reference `r_track = y + u / Kp`. Round it to the reference's type, and check its bounds. Do not add a bias, and do not change the law. The alignment is explicit. A bumpless entry at non-zero `Kp` requires it, unless the reference already holds that value.
4. At one task boundary, transfer the reference (the tracking state) and the mode to Auto. Wait for the output-image confirmation. The mode, the reference, the manual-command state and the fixed gain are exact guards. PV and output are refreshed within their declared bounds.
   - The held output is the last committed mapped output image, read at the task boundary with fresh PV. An uncommitted debugger write to the output is not the held output.
   - The manual-command state is set to the held output, rounded to its IEC type.
   - When the alignment flag is set, the reference is set to `PV + heldOutput / Kp`. This needs the Auto entry and a finite non-zero fixed gain. Otherwise the explicitly reviewed reference is used.
   - For an output without an address the held output is the variable's value at the task boundary (A5.3). Invalid roles and out-of-range calculated states are rejected before any write.
   - The initial values at the boundary and the applied values are retained in the transaction.
5. **Continuity.** The first-output check follows A6.4. For this P law the allowance is `2·|Kp|·n`, because `Ki` and `Kd` are zero. `n` is half the measured output's range over the steady baseline, and zero without a baseline. A reviewed transfer tolerance replaces that allowance. It is a finite value from zero through the declared output span, in output units. The tolerance is retained in the reviewed record and used for initialization, live entry and recovery. Editing it invalidates the qualification.
6. Apply the reviewed task reference through the same guarded boundary. This is a deliberate reference step. It is separate from the continuous Auto entry.
7. Observe for the declared duration, with the runtime recorder, in process time. Assess the actual samples against the target deadline, the error, the process and output bounds, the quality, the mode and the transfer evidence. Missing scans, or an unconfirmed operation, cannot pass. Retain the complete recording, and the times and values of the reference changes. A trace polled from the page is not evidence (B2).
8. On a passing observation the loop stays in Auto. **Restore previous settings** returns the loop to Manual tracking of the freshly observed output, restores the previous reference and the retained previous gain set, and then returns the loop to the mode it had before Apply (A6.2), also when Restore is chosen after reopening, from Apply's report. It does not reset the process, and it does not assume that zero output is safe. **Restored** is shown only after runtime confirmation (B6.4).
9. A cancellation resolves the owned transfer and keeps its real outcome. If Auto was already entered, the reviewed Manual tracking recovery applies. An unconfirmed recovery is retried on the original endpoint and program identity. The Auto entry and the target step are never repeated.

## Part D. Acceptance

**D1. Evidence classes.** A test result states which evidence it establishes:
native runtime behaviour, numerical correctness, extension behaviour, rendered
interaction, executed integration on the reference project, or physical
qualification. One class never stands in for another.

**D2. Native runtime tests** run the runtime protocol in the deterministic
harness that advances process time by scans. They cover a long experiment (at
least six simulated hours) completing without any wall-clock wait, each
failure state of the protocol annex, atomic transfer revert (A6.3),
interlocks (A6.6), reload and restart (A6.7), parameter rejection (A6.8), and
recording with a record period (A4.2).

**D3. Numerical tests** cover every formula and rule of the numerical annex,
including jittered sample spacing (A4.3), gaps, block averaging (A4.5) and
partial responses (A10.3).

**D4. Extension tests** are registered in the extension suite and cover each
stage's behaviour and failure states with typed next actions (A11). Rendered
journeys are selected by tag, not by full title.

**D5. Reference integration.** The real-plant reference scenario is the
`examples/room_heating` project: the real `Control.PID` TIC101 holding the
tank at 45 °C, a long process with a slowest time constant of about
75 minutes. Against that project's runtime started outside VS Code in
simulation mode and attached, with the loop in Auto: Identify with Automatic
(a closed-loop setpoint step), Tune with Automatic, Validate (model check and
isolated simulation), Apply with observation, Restore previous settings, and
Keep in project, each through the actual Control Studio. The evidence names
the project, controller, runtime build, plant, timing and operations. The
runtime protocol is also exercised directly against `trust-runtime`, in a Rust
integration test, for each combination of: loop in Manual or in Auto; addressed
or unaddressed controller output; task period equal to the resource cycle or ten
times it; time scale 1 or 10. The two ways VS Code reaches a runtime, launched
by the extension or attached to one started outside it, are proved by the
rendered journeys, because they differ only in the extension's wiring.

**D6. Rendered review.** The purpose contract's whole-platform visual review
applies to every Control Studio screen, including the approval block, the
progress display and every failure state with its next action.

**D7. Physical qualification** needs a reviewed physical target. Simulated or
predicted results never establish it, and without that target it remains
explicitly unqualified.

## Appendix 1. Where the earlier sections went

| Earlier section | Now |
|---|---|
| §1 Supported control loop | B1.4; C0, C1, C6 (two-loop cascade), C12 |
| §2 Opening Control Studio | B1.1, B1.2 |
| §3 Source and state | A9 |
| §4 Page structure, including the shared UI contract | B9 (same heading and anchor) |
| §4.2 Navigation and operation state | B8, B9.4 |
| §5, §5.1 Response and analysis data, integrity | B9.6, B9.7 |
| §5.2, §5.3 Analysis eligibility, multichannel and stability evidence | Numerical annex N4 |
| §6 Recording acquisition | A4; runtime protocol annex E4 |
| §7 Experiment execution | B3.3, B3.4, A6.5; runtime protocol annex E5; C12 |
| §8 Identify | B3; numerical annex N1–N3 |
| §9 Tune | B4; C1; numerical annex N5 |
| §10 Validate | B5; numerical annex N8 |
| §11 Automatic workflow orchestration | B8 |
| §12 Apply and rollback | B6; C1, C4, C13; runtime protocol annex E6, E7 |
| §13 Saved results and export | B10 |
| §14 Failure and recovery | A11; the failure tables of B3.9, B4.7, B5.5 and B6.7 |
| §15 Visual, responsive and accessible behaviour | B9.8 |
| §16 Acceptance | Part D |
| §17.1 Setup and capability boundaries | B1.3; C1–C5 |
| §17.2 Models, controllers and analysis | Numerical annex N4 |
| §17.3 Experiments and candidate evaluation | B5; C5; runtime protocol annex E8 |
| §17.4 Application and persistence | B6, A6.8; C1; runtime protocol annex E7 |
| §17.5 Completion (course gallery) | The course teaching specification |
| §17.6 Declared root locus | Numerical annex N7.6; B10.2 |
| §17.7 Runtime scan recording | A4; runtime protocol annex E4; B10.3 |
| §17.8 Competence target | Part C: every confirmed capability has its template |
| §17.9 PID groups | C7 |
| §17.10 Robustness margins | C0; numerical annex N7.7 |
| §17.11 Cascade tuned together | C6 |
| §17.12 Feedforward | C8 |
| §17.13 Smith predictor | C9 |
| §17.14 Gain scheduling | C10 |
| §17.15 Static decoupler | C7 |
| §17.16 Trajectory feedforward | C11 |
| §17.17 Keep in project | B6.5 |
| Primary Control.PID state adapter | C1 |
| General identified-model response evidence | B3.8, B9.7 |
| control-studio-controller-transfer.md | Runtime protocol annex E6 |
| control-studio-parameter-transactions.md | Runtime protocol annex E7 |
| control-studio-symbolic-acquisition.md | Runtime protocol annex E2–E4, E11 |
| control-studio-finite-search.md | Runtime protocol annex E8; C5 |
| control-studio-isolated-run.md | Runtime protocol annex E8 |
| control-studio-controller-bindings.md | Runtime protocol annex E2 |
| control-studio-experiment-abort.md | Runtime protocol annex E5; C12 |
| control-studio-auto-commissioning.md | C13; runtime protocol annex E6, E11 |
| control-studio-general-models.md | Numerical annex N4; C0, C4 |
| control-studio-routh-hurwitz.md | Numerical annex N7 |
| control-studio-run-validation.md | Numerical annex N8 |
