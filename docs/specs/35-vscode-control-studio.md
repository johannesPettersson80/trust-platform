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
  sharing its process would tie Control Studio's operations to its restarts and its load. Two
  things Control Studio needs from the compiled project come from the language tools: loop
  setup's catalog of signals and controllers (B1), and Keep in project's edit (B6.5), the one
  project edit Control Studio makes. The engine obtains both through `trust-lsp command`
  (Specification 14 §7.8), a separate command-line run that loads the project from disk and ends
  with its answer, so each has one implementation and no language server session is involved.
- The engine's methods, each arriving with its stage: `controlStudio/open`, `controlStudio/status`,
  `controlStudio/cancel`, `controlStudio/page`, `controlStudio/inputs`, `controlStudio/run`;
  `controlStudio/identify`, `controlStudio/tune`, `controlStudio/preview`, `controlStudio/validate`,
  `controlStudio/apply`, `controlStudio/restore`, `controlStudio/keep`; `controlStudio/setupDiscover`,
  `controlStudio/setupSave`, `controlStudio/projectLoops`; `controlStudio/analyze`; `controlStudio/parameters` (the parameter writes of a C12 loop's tool: the parameters of a declared analysis, or the experiment's declared settings; its `invalidate` answer carries the sentence the page shows as `message`, and the page shows no sentence of its own; its `review` takes the values as the text the engineer typed (`json`), reads and refuses it, and answers the values it read; its `confirm` starts the write of the review it names, and the engine decides from that review whether the write applies or restores; the write's final answer says how it ended (`status`, `applied` or `failed`, with the engine's sentence in `message`); each answer of `inspect`, `review`, `reviewRestore`, `invalidate`, `released` and of the write names the tool's `offer`, the step its button runs next and the actions the engine takes, which the page renders without a phase of its own, and the open answer's project test form names the first offer, Inspect (`projectTestForm.parameterOffer`); the write reports its progress in the engine's words; its `released` answer says whether a project test left the settings released, and reads no runtime); `controlStudio/export` (the loop's results for the engineer to save, B10.5); `controlStudio/monitor` (the loop's live sample and the target's label, B2 and B9.2); `controlStudio/plantMonitor` (the plant summary of a plant handoff, B1.1 and B1.2); `controlStudio/simulatorStart` and `controlStudio/simulatorStartResult` (the Start simulator action, A2.8); `controlStudio/chartExport` (a chart image's file, B10.5).
- An answer of a method that answers more than one shape names its kind in `kind`, a word the engine writes and never reads, and the
  page shows each answer by that word, never by the fields it carries: `controlStudio/identify` and the final result of its operation
  answer `identification`, `projectTest` (a project test whose fit kept no model, C12), `manualIdentification` (the Recorded-data fit,
  B3.3), `reset`, `importedRecording` or `operationStarted`, and its `review` answers the review, whose own `kind` names the experiment
  the campaign runs; `controlStudio/apply`, `controlStudio/restore` and `controlStudio/keep` and the final results of their operations
  answer `applicationReview`, `keepReview`, `applyOutcome`, `restoreOutcome`, `keepResult` or `operationStarted`; and the final result
  of `controlStudio/tune` is `tuneResult` or `searchAnswer` (C5). The methods that start an operation (`run`, `identify`, `tune`,
  `validate`, `apply`, `restore`, `keep` and `parameters`) answer `operationStarted` with its `operationId`; `controlStudio/analyze`
  names its answers by their `status` (B9.9). The page tells the answers apart by `kind` alone, and shows an answer of another kind
  than its request asks for as none of the answers it asked for.
- `controlStudio/open` answers the page's own starting values (wave 5b, V7.17): `identifyMethods`, the Method list in its order, each with its
  value, label, availability, whether the study records the method as an acquisition setting (`records`) and, when the loop cannot run it,
  the sentence that says why (B3.2, B3.3; a setpoint step is offered by a loop
  without a setpoint when it has a setpoint-role signal in the process unit); `experimentLimits`, the Limits editor's limits: the loop's
  declared defaults with the study's own record over them (B3.2); and
  `projectTest` (whether the loop has a project test) and `projectTestForm` for it, the signals the test offers and which start selected,
  its duration and baseline, and the parameter tool the page shows: the parameters of a declared analysis, or the experiment's declared
  settings (C12), which are the two tools an engine method serves (`controlStudio/parameters`), so a loop that declares neither is named
  none and shows no parameter controls (B6.1). The form also gives the binding the tool starts from (`parameterBinding`: the loop's whole
  binding for the experiment's settings, and the binding with no parameter for an analysis's, because the engine's answers name the ones
  the analysis may write). `experimentLimits` also gives the review's defaults: the
  declared amplitude, the relay direction, and the loop's declared duration (`reviewDurationSeconds`, zero when the loop declares none).
  The project test's review sends no duration: the engine reviews the loop's declared experiment, and refuses a missing duration under
  Limits (C12). The page builds the Method list, the project test's controls and the Limits editor from them,
  shows each sentence as its notice, and computes none of them. The state-design section of the Tune page is shown when the Tune page
  offers State-space design (C4); its fields are the answer's `stateFields`. The page takes the engine's checked loop (`loop`).
- The live sample is the engine's `MonitorResult.sample` (B2): the panel posts it as the engine answers it, in a `loop.monitor.snapshot`
  message that names its project, session, loop and revision, and the page reads it as the generated declaration. Each signal of an
  available sample says whether the plots draw its value (`plotted`: a forced value is a gap in its own trace, B9.6), and the page
  applies no quality rule of its own. When the engine answers a failure instead of a sample, the panel posts the failure's sentence
  as an unavailable sample that names no source, because the failure names none and the panel does not know the target; the same
  holds for the plant summary of a plant handoff.
- `controlStudio/export` answers the loop's results for the engineer to save (B10.5). With `format` `json` it answers the complete typed workflow result: the `controlStudio/open` answer of the loop, with the study's sections as the page shows them and the additional channels of its identification, and without any key that names a credential (a token, a password, a secret or an authorization). It also carries, under `recordings`, the run context of each recording the study keeps whose run states one (B10.2, B10.5). With `format` `csv` it answers the signal table of one recording: the recording `recordingId` names, or else the recording the study's current Identify result was fitted from; a study with none refuses with the next action Identify. The table has one row per recorded sample with the columns of B10.5, and the additional channels as five columns each. The answer is `{"result": {"format", "suggestedFileName", "text"}}`. It writes nothing and starts nothing, and the request carries no runtime connection: the extension asks where to save the text.
- `controlStudio/plantMonitor` answers the plant summary of a plant handoff (B1.1, B1.2). Its arguments are the handoff's project, plant and active configuration digest, the runtime's state (`runtimeStopped`, and whether its endpoint answered the probe, `endpointReachable`), and the connection (the endpoint and its control token). The engine makes one `plant.status` read of the runtime (specification 34, §20.4), bounded to two seconds, and answers `{"result": {"availability", "source", "plantId", "reason" or "status"}}`: unavailable with the sentence that says why, or available with the plant's status. The reasons are: "Runtime is stopped" (a stopped runtime whose endpoint does not answer; it is not read), "Runtime is unreachable" (a blank endpoint, or a read that cannot connect), "Runtime authorization was rejected" (a refused token), "Plant status is unsupported", "Plant status is malformed" (an answer that is not the protocol's form for this project), "Running plant revision is stale" (a revision other than the handoff's, checked before the plant), and "Plant no longer exists". The extension forwards the lifecycle's facts and posts the answer; it decides none of them.
- `controlStudio/simulatorStart` decides a Start simulator request (A2.8): `{"result": {"decision": "start" | "join" | "reply", "reply"?}}`. The extension starts the local simulator through the runtime lifecycle service only on `start`; it joins the start in flight on `join`; it shows `reply` otherwise. `controlStudio/simulatorStartResult` takes the lifecycle service's outcome and answers the reply the page shows, `{"result": {"ok", "message" | "failure"}}`.
- `controlStudio/chartExport` answers the file of a chart image the page renders (B10.5): `{"result": {"fileName", "filterLabel", "extension", "bytesBase64"}}`, or the refusal of the page's message. The engine checks the message against the session the panel shows, decodes its data URL (base64, or percent-encoded text whose bytes are UTF-8), and checks its MIME type against the data. It writes nothing; the extension shows the save dialog and writes the bytes.
- `controlStudio/projectLoops` answers, for the workspace folders, each folder's loop files decided for the runtime: an accepted loop with its open request, a refused file with its reason, or no file (`missing`). It names the folder that declares a loop (the first whose loop has its revision, else the first whose loop has its id and project; A8.3), and the loops of an equipment: the accepted loops whose controller is the equipment's instance or a member of it, or the reason none is (B1.3). The extension opens, offers and locates from these answers and decides none of them.
- `controlStudio/projectLoops` also answers the loop Control Studio opens (B1.2, A8.3). A request that names a loop (`loopIdentity`) answers `resolution`: the decision on the file that declares it, accepted with its open request, or refused or missing with its reason, its failure and next action (the engine's sentence, shown as it is). A request that names neither a loop nor an equipment answers `open`: the one configured loop; the first folder's first decision when none is configured (its missing file, or the sentence that asks for a project when no folder is open); or, when several loops are configured, no `open` and the `choices` the engineer picks among. The panel shows the engine's refusal with its next action and never checks a loop file.
- `controlStudio/setupDiscover` answers the mapping of the three-page binding selectors when its request names one (`mapping`, B1.2, B3.6):
  the loop the panel shows, the plant inputs and measured outputs that the selectors name by symbol, and, while a recording is shown, the
  recording's selection. The answer's `mapping` gives the outcome (`current`, `other`, `ambiguous`, `unmatched` or `recorded`), whether the
  mapping blocks the work (`blocksRun`: the ambiguous and the unmatched outcomes), the sentence the page shows for an ambiguous or unmatched
  mapping, the selection of the one other loop (`other`), the controller choices of the mapping (each family that a loop's adapter realizes,
  with its label and its selection), and whether a recorded selection `changed`. The page matches nothing itself, gates its actions on
  `blocksRun` and shows the answer. A request that names the loop the panel shows (`loopId`) and no `mapping` is answered with that
  loop's own mapping: the outcome `current`, which blocks nothing, and as the controller choices the other loops of the project that bind
  the same plant inputs and measured outputs; a loop that the discovery does not list has no `mapping`. The panel's first list at open
  names the loop this way, so that the Controller selector offers those loops before the engineer changes a selector.
- `controlStudio/open` also answers `liveSides`: the plant inputs and measured outputs that the live selectors offer (B3.6), each signal with its
  identifier, symbol and unit. A loop's channels give them in channel order; a loop without channels gives its manipulated variable and its
  process variable. The page offers each side from this answer, and the Requirements editor takes the variable and the unit of each output
  channel's row from it (B4.3): the page looks up none of the loop's signals. `liveSides` also lists `plotSignals`, the signals the live
  plot and the plot of a kept recording draw (B9.6), by the identifier of the project test's Signals selector (`sp`, `pv`, `mv`, the
  output the process received, `requestedMv` where the loop declares an actuator, then the identifier of each additional signal),
  each with its kind (`kind`: `setpoint`, `measurement`, `appliedOutput`, `requestedOutput` or `additional`), its variable, its unit and the width of its declared range, which frames a flat value. The applied output of a loop with an
  actuator also names the symbol its experiment's recorded trace carries (`recordedAs`: the manipulated variable's). The page draws
  from this list and holds no copy of the rule that builds it.
- `controlStudio/open` also answers `heldInputs`, what the study's recorded inputs mean for the page's editors (A8.6), so that the page decodes
  no record: the entry of the Method list that holds the study's method (`identifyMethod`, the word of its offer; the method the loop
  declares is shown as Automatic; absent when the study records none), the controller family the study holds (`family`: a state law's
  family when the engineer chose one, else the structure they fixed), and the pinned settings that apply to the tuning method the study
  holds (`pins`: the state design's for the state-space method, the structure's for any other).
- `controlStudio/open` also answers `manualIdentification`, the Recorded-data fit the study keeps as the manual answers state it (B3.3,
  A8.6): the saved stage replayed from the kept recordings, or the engine's default form and its action that opens a recording for a
  study that keeps none. A saved stage that cannot be replayed is answered as a failed stage with the sentence that says why, so the open
  does not fail for it. `controlStudio/identify` with `reset` answers the same field for the new working study. The page holds the answer
  and shows it when the Recorded-data fit is the method it shows; it decides no replay, and the extension's route asks for none.
- `controlStudio/open` also answers `requirementOutputs`: the controlled outputs of the Requirements editor, in channel order (B4.3, C7). They
  are the outputs that declare a setpoint, or every output when none does; an output whose signal the loop does not declare is left out
  (as `liveSides` leaves it out), and a loop without channels lists none. Each names its `channelId` (the channel its rows are named by and
  its record is kept under), its `signalId` (the signal `liveSides` names with its variable and unit) and the `requirements` its rows start
  from, in the order Validate holds an output to its requirements (C7): its own record in the requirements the study holds (the inputs'
  record, else the one Tune designed to), else the requirements the loop declares for the channel, else, for the loop's process variable, the
  record's own values. The field is absent when the output is held to none. The editor shows the signal column and a row set for each
  output when the answer lists several; it takes an output's rows from the record the page holds before it takes them from this offer, and
  decides neither which outputs have requirements nor what a row starts from.
- `controlStudio/open` also answers the Requirements editor's scenario offers (B4.3): `scenarioTemplate`, the scenario the editor adds to its
  list (it has no name; the engine names it when the list changes), and `scenarioRowSignal`, the signal a new initial value or timed change
  starts at (the loop's setpoint; absent when the loop declares none).
- `controlStudio/setupStep` answers the step-by-step loop setup (B1.3, C12): the session the page sent back, the engineer's answer or Back, and the question the engine asks next (its key, phase, kind, choices and prompt), the words of a refused answer, or, when every answer is in, the finished draft and the sentence the engineer confirms before the save. The session is the engine's (`setup/step_session.rs`): it holds the answers in the order their questions were asked, and decides Back, the replay of the answers Back passes without asking, and the discard of the answers after a changed one. The page keeps the session as the engine returned it and asks no question of its own.
- `controlStudio/tune` with the automatic method on a loop whose `controller.project` declares `axes` and no seed runs the finite search of C5 as an operation: it claims the loop, journals each trial's isolated run before its start, updates the page after each trial, and is cancelled by `controlStudio/cancel`. Its final update carries the search's answer: how it ended, the selected trial, and every trial with its J and bounds outcome, or marked not attempted; J's unit (`scoreUnit`) is the engine's, and once the candidate is kept the answer also carries the stages and the stage evidence the study has then (`stages`, `stageEvidence`). A named method (formula, relay, manual or state-space) on such a loop is refused with the sentence "A search law is tuned by its finite search: choose Automatic.", and the Tune stage of a run refuses such a loop too. A state-feedback law is tuned by `controlStudio/tune` with the method `state-space`, which designs it on the study's current model (C4); the answer carries the state answer of C4.
- `controlStudio/inputs` records the engineer's study inputs (A8.1), as the page's editors change
  them: the identification method and the experiment limits (B3.2, the Limits editor), the
  requirements with the verification scenario list declared beside them (B4.3, the Requirements
  editor), the tuning method, a fixed structure, and the pinned settings (B4.2). The acquisition record keeps its ranges by signal, one
  entry for each plant input and measured output (B3.2). The request names either the whole inputs or one
  `change` with the page's keys, as the editors send it: `acquisitionSettings`, `family`, `tuningPreferences` (`method`, `values`,
  `stateDesignDraft`), `pinnedValues` (an object, or `null` to clear), and the requirements record that `referenceChange` marks,
  or that the rows typed for each controlled output (`outputRequirements`) mark, with its `scenarioDefinitions`; the engine assembles
  that record from the typed rows (B4.3). The engine merges a change into the recorded inputs in that order (A8.1): a family sets the
  structure it names and clears it for any other word; a pinned change takes the family the study holds after it, and records no
  pins when there is none; a key the study does not take, and a tuning method the engine does not run, are refused with one
  sentence each. A channel mapping goes as the request's `channelSelection`, and a binding change names no input. For a state-feedback law the inputs also record `stateDesign`: the family the engineer chose, the text of each field of the design form (the desired poles, the state and input weights, the process and measurement covariances, the solver, the horizon steps and the terminal weights) and the pinned coefficients by symbol (C4). Tune reads them. The answers of `controlStudio/inputs` and `controlStudio/open` carry `stateFields`: the design-form fields that the family shows, with their labels, and the horizon fields only under the finite-horizon solver policy (C4); a loop without a state law carries none. The response of a
  formula method is a field of the requirements (B4.3), so it is recorded once, and the requirements
  keep no tuning method of their own: the method is the one the inputs name. Tune reads that response
  from the requirements when its request names none (`controlStudio/tune` and the automatic run alike,
  B4.3), so the page sends no response of its own. The request names the journal folder (`storageFolder`, A8.3; absent or empty,
  the engine reads its own): while the loop's journal holds an Apply's checkpoint that waits for its Restore, every change is refused
  with the block's reason and the next action Restore previous settings (B6.4). It answers at once with
  the inputs the study then holds (`inputs`), whether they changed (`changed`; the study is written only when they did),
  the stages, and the Apply offer and the action that holds the Validate page's position after the change (`applyAvailable`,
  `primaryAction`), by the one rule of B7, so a change that leaves the validation not current withdraws Apply; a change invalidates
  exactly the results that depend on it (A8.4), and a refused change names the rule it breaks (B3.2:
  a relay without its declared direction is refused with that rule's sentence). The answer's `invalidated` says whether the change left the
  validations not current (`validation`) and, when it did, the sentence the Validate page shows (`reason`, "Inputs changed. Tune and verify
  before applying."); the page writes none. The answer also lists, as `stale`, the tool whose recorded analysis (`controlStudio/analyze`,
  Records) was calculated for a model that is not the study's now (B9.9); the page drops that answer and asks the engine nothing. The channel mapping of a multichannel loop (A8.1, the Tune page's **Channels…**) is the request's `channelSelection`: the plant signal of every declared channel of both sides. The engine checks it against the loop's declared channels (each channel once; a signal the loop declares for its side, with the channel's unit; no signal twice; a PID group keeps its declared wiring; and the binding rules of the channels), keeps it in the study, and the stages then read the loop with the selection applied, so Identify, Tune, Validate and Apply use one mapping. A changed selection invalidates the validations (A8.4), and a fit stays current only while its signals are the loop's (`identify_signals`). The answers of `controlStudio/inputs` and `controlStudio/open` carry `channelChoices`: one row per model channel, with the plant signal it has now, the signals of its side it may take, and the PID member that owns it; a loop with one input and one output carries none, and the page offers **Channels…** exactly when the answer carries them. The echo of the recorded inputs that follows an open answer is no answer of `controlStudio/inputs`: it carries the inputs alone and leaves **Channels…** as the open answer set it. `controlStudio/open`
  answers the recorded inputs; the Apply offers of B6.4, B6.5, B7 and A8.6 (`applyAvailable`, `restoreAvailable` and
  `keepAvailable`, a journaled observation to resume, and `primaryAction`, the action that holds the Validate page's position); the
  last Validate result while it is current for the
  study's model, candidate, requirements and scenarios (A8.4), with its table and whether Apply is
  available (B5.4); the operations this engine runs for the loop (`running`), which the page follows
  (A8.6); and the journaled operations that reopening reattaches to (`reattached`): the recordings,
  experiments, campaigns and isolated jobs that the journal holds unfinished. A journaled checkpoint is
  no operation and is not listed: an Apply's checkpoint is answered as Restore availability, and a
  mode checkpoint is returned to its mode by the campaign's resume (A6.2, A8.6). So reopening shows
  them. The answer also holds `stageEvidence`, the evidence its stages are computed from, which the page
  gates its actions on (B9.4, A8.6); its `stages` carry the block of an Apply's checkpoint that waits for its Restore (B6.4); and `identification`, the study's current Identify result as Identify
  answers it, absent without a current model and after Reset (A8.6), and `tune`, the study's current candidate as
  Tune answers it (B4.5), rebuilt from its Tune section and absent without a current candidate (A8.4), which is `candidate` for a `Control.PID`, `projectCandidate` for a program's own law (C2) and `searchCandidate` for a finite search (C5), exactly one of them; its `response`
  is absent, because the study keeps no prediction and none is run on open. Tune and Validate use the requirements the
  study holds: the recorded ones over the loop's declared ones. Validate runs the scenario list the
  study holds in the same order of precedence: the recorded list over the loop's declared list.
  For a loop whose program's own law the engine serves (C2 Setup), `open` answers Tune, Validate and Apply as applicable. For any other
  program's own law it refuses the loop with the cause `controller-capability` before it reads or moves the study.
- `controlStudio/run` is the automatic progression of B8: it runs, from the earliest stage whose
  result is missing or no longer current (A8.4), each stage in order up to and including the one
  it names (Identify, Tune or Validate), reusing the current results. Identify plant is the one run
  that identifies again: its request sets `identifyAgain`, and its Identify stage then runs a new
  campaign even when the study holds a current model (the complete workflow specification, "Identify,
  review, then tune"). A new model replaces the candidates and validations based on the old one
  (A8.4), so such a run plans every later stage. It is one operation with one Cancel. It stops at the
  first failure and keeps every result obtained; when a stage needs an
  input the engineer must give (manual settings, a requirement Automatic cannot fill), it ends
  naming that input, so the page opens the editor that owns it and **Done** runs it again.
  A successful Validate stops at Apply: `run` never applies. Its final result carries the status
  of every stage, the stage evidence the page gates its actions on (a model, a candidate, a passed
  validation), the identification the run made when it identified, and the input it waits for, if
  any. Open recording… with a method other than Recorded-data fit (B3.3) identifies from a file,
  which is not a run: the engine keeps the file as a recording of the study (`identify` with
  `importRecording`, below) and fits a checked recording in the same answer, as it fits any recording the study keeps (`identify` with
  `openRecording`, B3.6). On a served loop, Open recording… with Recorded-data fit keeps the file the same way, and the
  manual identification of that file runs in the engine: `identify` with `manualEdit` stores the page's form; `manualQualify`
  checks a kept recording with the form (the recording `recordingId` names, or else the study's last opened recording, as `openRecording`
  takes it; a study that keeps none refuses the check); `manualFit` fits it (an operation that a Cancel stops); `manualChooseStructure` keeps
  the sensor-lag structure that the comparison recommends; `manualAccept` makes the accepted model the study's current model
  (B3.7); and `manualRestore` replays the saved stage as an operation (A8.6; the open answer carries the same state as
  `manualIdentification`). Each answer carries the form the study keeps, and a
  study that keeps none answers the engine's default form (B3.3). The study keeps the stage in its section
  `identifyManual`: the identifiers of the recordings and their settings, not their samples, which are read from the kept files. Another model that becomes the study's current model clears that section in the same save
  (B3.7). A supplied model enters through `identify` with `importModel` (B3.7). The TypeScript
  session and its `control-studio/<loop id>/typescript-session/` folder are removed (stage V7.17), so a loop the engine
  refuses has no other store: its refusal is its answer. The analysis the Tune page shows
  (its tool and inputs) is kept with that page, and opening the loop delivers it as `pageAnalysis`, so the page
  calculates it again (B9.9, A8.6).
- `controlStudio/identify` with `action: "review"` answers the campaign Identify would run (the
  method, its excitation and limits, the duration, and for a project-scripted experiment its
  activation, abort and restoration, C12) and starts nothing; the page shows it before the
  engineer starts the campaign. The project test of a project-scripted loop on the Identify page
  (C12) is this review and the campaign it starts: its review asks `identify` with `action:
  "review"` and no `settings`, and the engine reviews the loop's declared experiment with the
  Automatic method, because the project owns the excitation (any other loop's request without
  `settings` is refused with Limits as the next action); its start asks `identify` with no
  `settings` and the identity it names; its Abort is `controlStudio/cancel` of that operation.
  A project test's review and start need the running runtime, which holds its experiment settings
  and states the target's kind (A2.4): a request that carries no connection names the mode of the
  window's runtime lifecycle (`runtimeMode`), and the engine refuses it with the next action that
  applies, Start simulator for the local simulator (`simulate`) and Connect runtime otherwise
  (A2.8, E11.1); the page shows that action's button and asks the extension for nothing before the
  review. A physical target reviews and runs its project test as any campaign: with the approval
  block and the engineer's approval. The open answer's `projectTest` says whether the loop declares a project-scripted experiment, which is the engine's rule for the project test (C12), so the page offers and adopts the test from that answer. A project test finishes when its
  recording is complete and its outputs are restored, even when its fit refuses the recording: its answer then carries the
  refusal as its note and keeps no model (C12). The review answers an `identity`, the digest of what the campaign runs: the loop and its revision, the settings, the
  experiment and limits the review chose, the duration, the project's scenario of a project-scripted experiment, and
  whether the target is physical. A start names that identity in `review`. The engine answers the identity of the same
  settings on the live target and refuses a start that names an identity it no longer matches, with no next action:
  the review is asked for again (A2.5, E11.8). A review of a physical target carries an `approval` block (B3.4 step 1,
  B9.3). On a physical target the start is refused unless `review` names the current identity and `physicalApproved`
  is true, with the next action Approve. A simulated target needs no review: pressing the action is the confirmation
  (A2.5). A duration outside 1 s to 24 h (A7) is refused with Limits as the next action, by the review and by the
  start. A project test whose loop declares a hold parameter (C12) is reviewed with the hold that the runtime applies when the
  review is asked: the review reads it, and the duration that it shows, and that its identity names, is the recording of that
  hold. The start reads the hold again before anything is recorded; a hold whose recording differs from the reviewed one gives
  another identity, and the start is refused as above. A project test whose loop has a parameter tool binds the values of the
  parameter record that the runtime holds when the review is made (C12, Parameter purpose): the identity digests them, the start reads
  them again, and a setting changed between the review and the start gives another identity, so the start is refused as above. A start whose method the loop cannot run is refused before anything starts, whether it is `identify` or the Identify stage of a
  run (`controlStudio/run`): the refusal is the sentence that the open answer's `identifyMethods` gives that method, with the next
  action Choose another method (B3.2, B3.3), and the page checks no method itself. The Identify stage of a run makes the checks that
  need no runtime before it requires the runtime (E11.1): the method, then the campaign's limits (B3.4 step 1), which a run answers as
  the input it needs (`needs: limits`). A run on a stopped runtime therefore names an inapplicable method or asks for a missing limit
  first, and only then refuses for the missing connection. A run that waits for an input answers the sentence that asks for it
  (`needsMessage`) and, when the sentence concerns one form field, that field (`needsField`): the sentence is the first sentence of the refusal that
  stopped the run (a missing maximum duration is "Set the maximum experiment duration in Limits before identification." under the field
  `durationSeconds`), and for the manual settings, which the run asks for before any stage runs, "Enter the manual settings to tune the
  controller." The page shows the sentence as the run's reason, and the Limits editor opens with it under the field; the page writes no
  sentence for any input. The manual parameter tool has no method for a served
  loop, because Apply writes only the validated candidate (B6.1), with two exceptions: a C12 observation or open-loop loop whose declared
  analysis maps its parameters has the parameter tool of C12 (`Declared analysis parameters`), and a C12 loop that declares
  `parameterPurpose: "experiment"` has the tool for its experiment settings (`Experiment settings`); `controlStudio/parameters` serves
  both. Any other served loop refuses its parameter requests, and the page shows no parameter controls for it.
  `controlStudio/page`
  records the page last viewed (A8.1) and, on Tune, the analysis tool it shows with that tool's
  inputs, which `controlStudio/open` returns so reopening shows them (A8.6, B9.9); it changes
  nothing else and starts nothing. It also keeps the page's own view choices (`view`: the Method the
  Identify page shows, including Open recording… and Open model…, which start no campaign) as view
  state, which `open` answers as `pageView`; the engine never reads them, and a request without
  `view` keeps the recorded one. `controlStudio/preview` answers at once with
  the predicted response, margins and outcome of entered controller settings on the study's model,
  as Tune with the manual method would finish, and stores nothing (B4.5). Its request names the settings only: the engine reads
  the study's structure and pins (B4.2, B4.5), and refuses settings that are missing or are not numbers by their symbols, with the
  sentence `The preview needs the controller settings, each by its symbol and as a number.`
- `controlStudio/analyze` answers the analysis tools of the Tune page (B9.9). Its request has
  `projectRoot`, `runtimeId` and `loopId`, as `controlStudio/preview` has them, and `tool` and
  `inputs`. `tool` is one of `rootLocus`, `routh`, `poleZero`, `frequency`, `measuredFrequency`,
  `measuredPaths`, `feedback`, `leadLag`, `initialState`, `stateStructure` and `relativeGain`. `inputs` holds the
  tool's own fields; each is optional, and an absent field takes the engine's default. The model
  analysed is the study's selected model, or the general model that the request carries as `model`
  (N4.1; at most 256,000 bytes of JSON). `feedback`, and `initialState` on its feedback connection,
  close the loop with the controller the request carries as `controller`, or, when there is none,
  with the loop's declared law. The analysis uses the model as Identify accepted it; it never
  refits or rescales it (C0). A request without a `tool` answers the tools that apply to the model
  and the loop, each with its label, its action text, whether it is an operation, and its default
  inputs. Which tools apply, and every input check, are the engine's. A request without a `tool` may also name the answers the page
  holds (`held`: each answer's tool and the revision of the model it was calculated for), and the engine lists the tools whose held
  answers are no longer current as `stale` (B9.9): an answer of a tool that reads the study's model is current only while it was
  calculated for the revision of the model the engine analyses now, and an answer of the declared analyses (`rootLocus`, `routh`) and
  of the measured ones (`measuredFrequency`, `measuredPaths`) does not depend on the model. A tool that does not apply,
  or an input outside its limit (A7), is a method failure that names the input and gives its next
  action (A11).
  - A calculation is answered as `{"result": {"tool", "inputs", "defaults", "result"}}`, or with
    `"failure"` in place of `"result"` when the calculation refuses, carrying its cause and next
    action. `inputs` lists the inputs as the engine read them, with the defaults filled in; the
    model and the controller are named by their revisions. The `result` holds the tool's answer with
    two sentence lists: `summary` for the result area, warnings first, and `details` for the details
    disclosure. A result the mathematics reports as unavailable (a state structure that does not
    exist, a margin not observed in the range) is a `result` with its status, not a failure.
  - `poleZero`, `leadLag`, `stateStructure` and `relativeGain` answer at once. `rootLocus`,
    `routh`, `frequency`, `feedback`, `measuredFrequency`, `measuredPaths` and `initialState` are operations: their
    answer is `{"result": {"operationId": …}}`, and the final `controlStudio/update` of that
    operation carries the calculation's answer as its result. Each operation runs under the budget
    of A7; `controlStudio/cancel` stops it at its next step, and a cancelled operation keeps
    nothing. An operation claims its loop as Tune does (A8.5), so one operation runs per loop at a
    time; a quick answer claims nothing.
  - The method starts nothing on a runtime and writes nothing to a runtime. It changes no ST, no
    controller parameter and no candidate, and no result qualifies a candidate for Apply (N7.6). It
    stores no model: a supplied model is kept by `identify` with `importModel` (B3.7).
  - Records: for a loop with an engine study, a qualifying `rootLocus`, `routh`, `feedback` or
    `measuredFrequency` or `measuredPaths` result is kept in its study section (B10.2). `controlStudio/open` answers
    the kept records, as `analysis`, while their loop and model revisions are current (A8.4). The
    other results are not kept; reopening shows the tool and its inputs (`controlStudio/page`) and
    calculates the result again. The engine records the analysis each calculation of the study's own model answers (a request that
    carries no model and no controller of its own) as the analysis the Tune page shows (`study.analysis`): the tool, the inputs it read
    and the revision of the model it used (`modelRevision`, B9.9). The page names the analysis it shows in `controlStudio/page`, and the
    engine keeps the revision it recorded for the same tool; the page's route stores nothing of its own.
  - `measuredFrequency` measures the recording that its `recordingId` and `recordingRevision` name; each one the request names must
    be the recording's. `recordingId` is the recording's identifier in the study (the `rec-` identifier that `importRecording` answers,
    which the study lists), not the identifier the runtime gave its run, and the answer names that identifier. A request that names
    neither measures the study's last kept recording that is a periodic record (B9.9), and the answer names the recording it used.
    Beside the identifier the answer names the recording in words (`recordingLabel`), which the page's Recording field shows: the
    experiment the run wrote first and the UTC time of its first sample, for example "PRBS recorded 2026-10-22 14:05:31 UTC", or
    "Recording of …" ("Opened recording of …" for a file opened from disk) when the run wrote none.
    A study that keeps no such recording refuses with the next action Identify. A file opened with Recorded-data fit on a served loop is one of the
    study's recordings (B3.6, and the paragraph above); a loop the engine refuses keeps no such file, so it is not one of the
    study's recordings.
  - `measuredPaths` (C7, "Measured paths") takes its recording as `measuredFrequency` does (`recordingId` and `recordingRevision`, or the
    study's last kept recording, which the answer names). Its `inputs` hold `settledTrials` and `maximumUnexcitedInputDrift`.
    `settledTrials` has one object for each input of the loop's channels, `{"inputId", "baseline": {"startSeconds", "endSeconds"},
    "settled": {"startSeconds", "endSeconds"}}`, with the windows in seconds from the recording's first sample. The drift tolerance,
    when given, is finite and nonnegative; each window is finite and ordered. A window end is sent as the text the engineer typed: a blank end is absent, and a text that no number reads is refused under the
window's field (`<inputId>:<window key>`), as a missing end is; the drift tolerance is read the same way. The answer is the `measuredPaths` result of C7: the
    steady-state gain matrix, the relative gain array, the static decoupler, the pairing, the rank and conditioning of the fit, and its
    two sentence lists. A refused calculation is the envelope's failure with the sentence of its rule. The tool applies only to a loop
    whose channels give as many inputs as outputs, two or more.
  - A loop whose structure the engine does not run yet (stage V7) has no engine study. For such a
    loop the analysis takes the model from the request, keeps nothing, and answers no
    `measuredFrequency`, because the recording it needs is held by the study. Its declared analysis
    is read from the loop file: the cause `controller-capability` refuses stages, not a calculation.
    When the loop file uses a structure the engine does not run yet, so that the engine cannot open the file at all, a request that
    carries `model` answers every tool that applies to that model and to the `controller` it carries, operations included, as it
    answers a served loop. The engine reads no declared analysis from that file and keeps no recording for it, so `rootLocus`, `routh`
    and `measuredFrequency` do not apply, and nothing is kept. An operation claims the loop by the `projectId` and `id` that the file
    names. A request for the tools without a `tool` and without a model answers an empty list when the loop's own cause is
    `controller-capability`: the refusal is the open answer's, and the page shows no tool for that loop (B9.9). A request for a tool
    without a model keeps the loop's own cause, and so does a loop identifier that the project does not declare; any other failure of
    opening the loop is answered as it is.
- `controlStudio/identify` with `action: "importRecording"` checks a saved Control Studio result or
  recording file, given as its text in `json` (at most 32 MiB of text), and keeps it as a recording
  of the study, with its original source and the opened marker (B10.2, A9). It answers at once; a
  checked recording's fit is named in that answer (B3.6). The engine reads the file as B10.4 requires: a result of a schema
  version other than 3 or 4 (B10.2), a malformed identity, a nonfinite or out-of-order sample,
  inconsistent units or source, an invalid selected interval, or a credential field is refused as
  a whole, with the sentence that says why. It then checks the recording as B3.6 and B3.9 require:
  a complete runtime scan buffer at a steady record period (A7, annex E4.5), an experiment that
  completed, and the loop's units, with no conversion (A9). A file whose identification keeps a
  step-response reference is refused, because the engine does not read that reference yet.
  This check does not read the sections of later stages (B10.2). The answer is `{"status": "checked", "resultId",
  "resultRevision", "projectId", "source", "samples"}`: `source` is `Simulation` or `Physical` as the
  file writes it, and `samples` is the number of raw samples. The answer also names `recordingId`,
  the kept recording and `recordingCatalog`, the channels the recording offers with their ids and units, and the selection the
  study keeps: the loop's plant input and measured output unless the engineer chose others (B3.6, B10.2). Every channel of the
  archived run is kept with its unit: a channel the loop binds keeps the loop's signal, and any other channel, an extra output,
  keeps its name and its unit, so `recordingCatalog` offers it and it can be selected (B3.6). The engine then fits the
  checked recording with that selection, in the same answer as `openRecording` below gives: the answer names the fit's operation
  as `operationId`, which the page follows, or carries the answer that `openRecording` gives the recording (a pair fitted from the
  study, or a recording that needs another); a file that is refused keeps nothing.
  A saved result that keeps completed runs of its engineer's study (`engineerStudy`, schema 4, B10.2) is read run by run. The
  runs that hold the selected signals with their units, on the latest run's project revision and model revision, are the
  candidates; the last two are the estimation and the independent validation run (B5.2). The engine keeps every archived run
  as a recording, and with two candidates it fits the model on the first and checks it against the second: the answer then
  names `identification`, and says that the model was checked against an independent recording. With one candidate the runs are
  kept and the answer is `status: "recording-needed"` with the reason "Open an independent recording to check the plant model."
  A malformed run refuses the file as a whole (B10.4). An isolated simulation's run is read as its frozen request and its verdict
  say: it must answer the request, its stored verdict must be the verdict of its requirements evaluated again, and it is archived
  unless it took a selected model's plant. A run of another execution is not read yet, so the file is imported as one recording as
  before. The candidates of the pair also include the study's kept opened recordings of the same source. A recording kept from an
  isolated simulation names the model revision of its run, so it reads back as that isolated run, with its channels, its samples
  and its model revision, and pairs as the run it was (B3.6, B5.2). A pair of isolated simulation runs is fitted with independent
  validation as a pair of connected records is (B5.2), and keeps its recordings like any other pair. `importRecording` pairs under
  the study's kept recording selection when the file holds those channels, and under the loop's own plant input and measured
  output otherwise; it keeps the selection it pairs under (B3.6).
- `controlStudio/identify` with `action: "openRecording"` fits a recording the study keeps again, without live excitation (B3.6).
  `recordingId` names the recording; without it the study's last opened recording is fitted, and a study without one refuses.
  Its optional `inputs` and `outputs` name the plant input and the measured output of the fit by the recording's channel names;
  without them the loop's own signals are used. A name the recording does not hold is refused at once with the sentence "Choose
  input and output signals present in this recording." and no next action. The fit reads the selected signals from the
  recording as the loop's plant input and measured output; the study keeps the signals the fit used and the selection
  (B3.6, B10.2). The fit keeps the loop's plant input and measured output as its signals, with the recording channels it read
  them from beside them; it is current while its signals are the loop's (A8.4). One plant input and one measured output are
  fitted here; several signals wait for the state model fit (V7).
  A selected output that the loop does not bind is a channel the recording holds as an extra output: it is fitted as the measured
  output, read only, with the range its samples took, which describes the recorded data and grants no excitation limit (B3.6). The
  fitted model names that channel with the loop's model identifier for its role (the `modelChannelId` the loop gives its measured
  output or plant input, or the symbol of that signal when the loop gives none) and with the recording's unit, so an analysis or a
  structure design that names the loop's channel accepts the model, while the measured trace and the identify section's recorded channels keep the recorded name.
  The selection is kept in the study whatever follows. When the study keeps an independent pair of opened recordings that
  includes this one under that selection (the rule of `importRecording`), the pair is fitted with independent validation as
  `importRecording` fits it, and the answer comes at once in the `importRecording` shape: `status: "checked"`, `identification`,
  and `recordingId` the estimation recording. When there is no such pair and the recording is an isolated simulation's run, the
  answer comes at once with `status: "recording-needed"` and the reason "Open an independent recording to check the plant model."
  Otherwise an opened connected recording alone is fitted by the operation, which answers with its `operationId`. An answer about
  a kept recording names it in its check: `resultId` is the recording's identifier and `resultRevision` is `0`, because the study
  keeps the recording and not the revision of the saved result it came from (B10.2).
- `controlStudio/identify` with `action: "importModel"` takes a supplied model as its text in `json`
  (at most 256,000 bytes), checks it as N4.1 requires before any numerical work, and keeps it as the
  study's model (B3.7): provisional unless it carries evidence of independent validation. It answers
  at once with the identification result, starts nothing and writes nothing to a runtime. The answer is
  the identification result of the current model, as `controlStudio/open` gives it (B3.7). A model
  that is refused leaves the study's model as it was. A model that reuses the current model's revision (A8.4) is the same
  model, which changes nothing, or it is refused with the sentence "A changed model needs a new revision." and no next action.
- The result of a completed identification (the final update of `controlStudio/identify`, and the
  `identification` of a run) names `modelRevision`, the revision Validate's model check names for the model
  (B5.2), and `requirements`, the requirements Tune designs to for the model now (B4.3): the values the study
  records as given, with Automatic's values for the rest, listed in `automaticCriteria`. `requirements` is
  absent where Automatic cannot form them, and Tune then names the missing prerequisite (B4.7). The identify, run,
  reset and open answers also name `stageEvidence`, the evidence their stages are computed from: an
  identification leaves no candidate and no validation current (A8.4), and a reset answers the evidence of the new
  study, which holds no model, candidate or passed validation. A Tune answer names the evidence of the model and the candidate it
  designed, and a Validate answer the evidence of its validation's result, passed or not (B9.4); a preview keeps nothing, so it names
  the study's own evidence (B4.5). Each answer of the manual identification (B3.3) names `stageEvidence` and `stages` as the study
  stands once the action is saved, so an accepted model, which is the study's current model (B3.7), makes Tune available.
- A long method answers at once with an operation identifier. Its progress and result follow as
  the notification `controlStudio/update`. A run's updates name the stage it is in (`stage`:
  Identify, Tune or Validate, B8), so the page shows where the flow is; the updates of the other
  methods name none. `controlStudio/status` answers for an operation, and
  `controlStudio/cancel` cancels it as Cancel does. The cancelled update of every operation carries the sentence the page shows
  (`message`): `Identification cancelled.` for an identify operation, `Analysis cancelled.` for an analysis, and `The operation was
  cancelled.` for every other; the page writes none (B9.4). The failed update of an Apply or a Restore carries the stages as the
  study and the journal hold them then (`stages`): one that stopped after its first write leaves its checkpoint waiting for its
  Restore, and the page shows that block at once (B6.4, B6.7). The extension's listener of an operation the page started, or a route
  adopted while it ran, also reads the operation's status once it stands, so an operation that ended before the listener stood still
  reaches the page with its final update. A reopened panel's listeners of the operations `controlStudio/open` listed do not read it,
  since an operation that has ended shows nothing there (A8.6).
- A method's JSON-RPC result is `{"result": …}` or `{"failure": …}`; a failure carries its cause
  and next action (A11). A malformed message or an unknown method is a JSON-RPC error.
- The engine ends when its client closes standard input. Ending it never stops a runtime-owned
  experiment or recording: the runtime keeps them, and the next engine reattaches through the
  journal (A8.5, A8.6).
- The VS Code extension only presents: it starts the engine, shows the pages, forwards the
  engineer's actions as these methods, and renders their answers. Its message types are generated
  from the Rust types. It passes the project folder, the selected runtime's identifier with its endpoint and
  control token. The engine decides the target: a live request whose loop is bound to another runtime is refused
  (A2.7), before anything starts. The engine keeps the operation journals in the folder of A8.3, which the command
  line uses too, so either front end reattaches an operation the other started: a request whose
  `storageFolder` is empty gets that folder, so the extension needs no copy of its convention. The
  engine never journals the token.
- The runtime keeps the real-time parts it owns today: recordings, experiments, transfers,
  parameter transactions and isolated runs (the runtime protocol annex).

The move from the TypeScript implementation follows
`docs/internal/testing/checklists/rust-logic-migration.md`. Since its stage V6 the panel's loops run
on the engine. A loop the engine refuses (with the cause `controller-capability`) shows the refusal and its next action on
the page, which is ready to show that answer once the open answers; the page offers no action the engine did not name, and nothing else opens: the TypeScript route and session are removed (stage V7.17). A loop without a controller law (C12) runs on
the engine.

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
    controller;
  - `apply` (B6.1 to B6.3): without `--review`, prints the review: the loop's mode, each
    parameter's running and candidate value, the validated operating point and the review identity;
    with `--review <identity>`, applies the reviewed change while the runtime still matches the
    review, waits for the operating point, transfers, observes the response for the test duration
    and prints the assessed result; `--resume` follows the journaled Apply's observation to its end
    when the engine that started it ended (A8.5, A8.6);
  - `restore` (B6.4): the same review and confirmation; returns the parameter record and the mode
    from before Apply, and prints Restored once the runtime confirms it; on a runtime restarted or
    reloaded since Apply it prints at once, with or without `--review`, that nothing was restored
    and the checkpoint is kept as history, with a status other than 0;
  - `keep` (B6.5): the same review and confirmation; keeps the values the record holds in the
    running PLC as its initial values in the project, and prints the fields kept;
  - `setup` (B1.3): without `--draft`, prints what the compiled project offers a new loop: each
    controller with what its code states (parameter record, measurement, output, setpoint, mode
    signal and what its TRUE value means, manual output, startup flag, task) and the structure
    setup proposes; with `--draft <file>`, a JSON draft of the confirmed choices as the setup form
    writes it, checks it, writes the loop definitions to `control-loops/` (all or none, never over an
    existing definition) and prints their ids. It compiles the project through `trust-lsp command`
    (§0.1a) and never writes anything else.
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
- Confirmation: a verb that changes a plant's controller or its project (`apply`, `restore`, `keep`)
  needs, given explicitly, the reviewed identity that the panel's single confirmation needs (B6);
  the same verb without `--review` produces the review and prints it. The identity covers
  everything the confirmation approves: for `apply` and `restore` the runtime's program, binding and
  active revisions, the mode, the values to write and the study's validation; for `keep` the values
  to keep and the text of each file the edit changes. A change of any of them since the review
  refuses the confirmation and asks for a new review. No option skips a check or an approval. A physical target needs
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
  (specification 33, §6.1), and the speed words the runtime gives for them
  (B9.2, annex E1.3);
- whether all attached I/O is simulated: every attached driver identifies itself
  as simulated and no fieldbus client (ADS, OPC UA) is configured;
- the tasks with their intervals, and the resource cycle interval;
- the session epoch and the program revision.

**A2.4 Simulated or physical.** The target is **simulated** when simulation is
enabled and all attached I/O is simulated. Otherwise it is **physical**. A mix
of simulated and physical I/O is physical. An editor setting, the debug
configuration, the presence of `simulation.toml` or a label in a request never
makes a target simulated. The runtime states the target's kind in its `status`
answer (annex E1.3), and every surface shows that answer rather than deriving the
kind from other facts. The runtime applies the same rule to every write it
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
The engine's review of an Apply or a Restore says in `confirmation` which one
confirms it, `press` on a simulated target and `approval` on a physical one, and
its review of a Keep in project says `explicit` (B6.5). The page shows the
review as that word says and reads the target's kind for no decision.

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

**A2.8 Starting the local simulator.** When a live request finds no runtime
running, it carries the mode of the window's runtime lifecycle, and the engine
answers the next action that applies: Start simulator when the runtime is the
local simulator, Connect runtime when it is a runtime attached through its
endpoint (§0.1a). The page shows that action's button. Start simulator asks the
extension to start the local simulator, and its answer continues the project
test with its review (C12). The engine
decides each start request (`controlStudio/simulatorStart`, §0.1a), in this order: a
request that does not name the loop's runtime and the simulate mode is refused
with "The Control Studio scope or loop revision changed. Reopen the current loop
before starting the simulator."; a start in flight for the same panel and loop
revision is joined, and a start in flight for another refuses with "Another
simulator start is still finishing. Wait and retry."; a runtime that is not the
local simulator is refused with "Start is available only for the local
simulator. Connect to a physical runtime explicitly."; a running simulator
answers "Simulator is already running."; a start in progress refuses with
"Simulator start is already in progress."; otherwise the extension starts the
simulator through the runtime lifecycle service and reports its outcome
(`controlStudio/simulatorStartResult`), which answers "Simulator started." or
the lifecycle's own sentence, or the start's failure with its sentence. The
engine starts nothing itself.

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
- A project test (C12) records every execution of its hold: the project owns its
  scenario, and Identify steps no plant input for it, so its recording takes no
  expected settling time and k is the smallest value for which the planned
  duration fits the sample limit of A7, which is 1 for any test of minutes.

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
| Analysis operation (`rootLocus`, `routh`, `frequency`, `feedback`, `measuredFrequency`, `initialState`) | 10 s of wall time, checked between steps (one frequency point, one sweep value, one Routh row, one response sample); past it the operation fails with "Analysis exceeded the 10 second budget." The size limits below bound its memory (128 MB, the budget of the former worker) |
| Analysed model | 1 to 32 states, inputs and outputs each; 1 to 17 coefficients per transfer polynomial; a supplied model's JSON at most 256,000 bytes |
| Frequency grid (`frequency`, `feedback`) | 2 to 512 positive, increasing points (N4.3), and at most 16,384 channel-frequency points, a feedback counting its plant channels twice and its controller channels once; default 0.1 to 100 rad/s with 128 points, the points lowered when the channel limit needs it, and a discrete model's maximum at most 0.5·π/Ts |
| Initial-state response | at most 4,096 samples times outputs; the duration over the step an integer within 1e-8, at least 1; a discrete model's step equal to its sample time within 1e-10; at most 512 delay states |
| Routh–Hurwitz and root locus | degree 1 to 16 and at most 17 coefficients (N7.2); exact values at most 32,768 bits (N7.4); a root locus of at most 1,001 sweep values and 64 parameters (N7.6) |
| Measured frequency response | at most 100,000 samples per recorded channel; startup samples at least 1 and fewer than the samples; at least 2 segments; minimum coherence 0 to 1; tones positive and below π/Ts |
| Lead/lag design | crossover above 0 rad/s; lead phase above 0° and below 85°; lag ratio above 1 (N7.14) |
| Nyquist and margins | 10 to 100 contour points per decade and at most 511 margin points; a margin with no crossing in the range is "not observed in range", never infinite (N7.8) |

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
it. The engine's stage evidence names no current validation after a change that invalidates one (`validationHasResult` false, so the
stages and the page's table follow it), and the study keeps the result as history. The cases:

- changed signals or limits: the provisional model is no longer current when its
  signals changed. A changed limit alone keeps the model; a changed signal is a changed
  loop definition, which starts a new study (A8.2). A fit keeps the loop's plant input and measured output as its signals and
  the recording channels it read them from; it is current while its signals are the loop's. Recordings are never deleted;
- a new or replaced model: candidates and validations based on the old model;
- changed requirements, or a changed list of declared verification scenarios (B4.3):
  validations;
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
reattaches to it at once instead of waiting for it to end. The engine names the
operations it still runs for the loop (`running` in `controlStudio/open`), and the
page follows them from their latest update; the page's Cancel stops each one, as it
stops an operation the page started. The last Validate result the study
holds is shown again while it is current (A8.4), with its table and whether Apply
is available; its plot is not kept (B5.4). After a runtime
restart Control Studio reads the actual controller values before reporting
anything as active; an old application receipt is not evidence of the settings
now active. The page's editors take the study's recorded inputs (the method, the limits, the tuning method and its settings, the pins, the
state design and the recording selection) once, when the loop opens; the open answer says which Method entry, which controller family and
which pins apply to the method the study holds (`heldInputs`), and carries the Recorded-data fit the study keeps (`manualIdentification`),
which the page shows when it shows that method. The answer to a later change renews the engine's own records, the
requirements and the stages, and leaves the editors as the engineer set them, so that an answer that reaches the page after a newer edit
never puts an older value back; a refused change leaves the study as it was (B3.2).

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
runtime, start the local simulator (A2.8), open loop setup, open **Limits…**, open **Requirements…**, open
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
plant and loop. Its plant summary is the engine's answer (`controlStudio/plantMonitor`,
§0.1a): the runtime's `plant.status` decides whether the plant is available, and an
unavailable plant names its reason. The page shows that answer in one line under the target
label: the plant, whether its status is a simulation's or a physical plant's, and either its model and
operating state or its reason. Opening a loop that already has a panel reveals
that panel.

**B1.2 Loop definitions.** A project declares loops in `control-loop.json` at its
root and in `control-loops/*.json`. Discovery reads both in file-name order,
validates every definition, rejects duplicate loop ids and never writes files.
Several loops, or several projects in a multi-root workspace, need a labelled
chooser; cancelling it changes nothing. A missing or invalid definition is explained, with **Configure control loop** and **Choose an example** offered. The loop that opens is the engine's answer (`controlStudio/projectLoops`): the one configured loop, the choice among several that the engine lists, or the engine's refusal or missing file with its sentence and next action. The panel checks no loop file. The engine also refuses at open what the page's loop validator refuses, so that a loop the page refuses is not opened: a null in any field the page reads, at any depth (the page tests each present field, so a null fails it; `cascade`, `expectedSettlingSeconds` and `relayDirection` are not read by the page, and `operatingPoint` is read and ignored (B1.4), so each may be null); a plant identifier that is blank; a declared analysis that breaks the declared-analysis rules (its plant, its parameters, its sweep, and the denominator at every gain); a parameter map that does not name, for each analysis parameter, one bound parameter of the same unit whose range holds the analysis's range, each bound parameter once; an analysis whose plant units are not the loop's signal units, a blank signal unit counting as `1`; a proportional law that holds keys beyond its kind and gain symbol; a parallel PID initialization that holds other than its six keys, and a Control.PID initialization that holds keys beyond its own; the declared states, at most 32 to a block (a switching controller's, a program's, and a Control.PID's seven states with its application states), each state checked as the page checks it, and a decoupled output whose matrix is not square of one to eight rows or whose index names no row (C7); a label (the parameter binding's task, mode, symbols, units and conventions, and the law's symbols) that is blank, longer than 1024 UTF-16 code units, or holds a character below U+0020; an address whose byte offset is not a safe integer, and an initial index that is not one; a state-feedback law that is not in the project's design, that takes a controller law, or that does not decode against the parameter record (C4), and a program's own law beside a controller law, a state-feedback law or an ON/OFF controller, or outside the project's design (C2); verification scenarios of the wrong structure; and an identification policy outside its limits (`loop_definition/contract_rules.rs`, `analysis_rules.rs`, `structure_rules.rs`, `checks.rs`, `identification_policy.rs`). A missing output limit or experiment duration does not refuse a loop: the campaign's preconditions name it (B3.4 step 1), and Identify chooses the durations (B3.3). `operatingPoint` is read and ignored (B1.4).

**B1.3 Setup.** **Configure control loop** sets up an existing program from the
compiler's metadata: symbols with their IEC types and optional addresses,
controller instances with their compiled types and owning tasks, task intervals,
the code links from each `<output> := <instance>.<method>(…)` call, and every
symbol each program body writes. The compiled process image lists every addressed numeric and BOOL variable (`booleans`, apart
from the plant candidates), so a declared state may be a bit such as `%MX40.0`. It prefills what the code states (a
`Control.PID`'s parameter record, setpoint, measurement, manual output and
output; a cascade where one PID's output is the only source of another's
setpoint; a startup flag guarding `Initialize`; the mode condition; manual
outputs that copy the output) and the engineer confirms every prefilled binding.
A link is evidence, never a silent choice. Setup saves only after a complete
review, never overwrites an existing definition, and cancelling writes nothing. The engine checks the draft of every structure. The form posts each draft as typed, with no check of its own: every number field is
posted as the text the engineer typed, which the engine reads (a blank field is not given, and a text that no finite number reads, such
as `1,5`, is refused under its field with the sentence that asks for it), and a blank optional name is absent. The draft carries every
field of the chosen structure and every row the form draws: the engine takes the fields that the law or the design needs and ignores the
others (the integral and derivative variables of an own P law, the derivative variables of a PI law, the estimator of pole placement and
LQR, the measured gain and further outputs of LQG), and it skips a row with no entry (a further measured output, a search coefficient or
kept value without its variable, an operating point with no entry, and the estimate rows after the states of the estimator's model). The
choices of the form (the structures, the Auto/Manual meaning, the hysteresis bands, the threshold policies, the designs, the laws and
where a PID law takes its derivative, the search families, the kept starts and the actions) are the engine's: their values
(`structures`, `choices`) and the words of each value (`labels`), which the page shows; it keeps no words of its own, and a value without
words would show as itself, never be left out. Save is always enabled, and a refusal shows under the field it names (`field`), or in the
form's message when it names none; a refusal that names a field of a section the form hides shows in the message instead. The engine
asks the questions in the order of the form and refuses the first blank one: a measured signal's symbol is chosen, its unit is given,
its minimum and maximum are numbers, and the maximum is above the minimum; each refusal names that signal's field. An Auto/Manual signal
that the form cannot offer, because the program has no BOOL without an I/O address, is refused with the sentence that says why. The
recording duration of an identify-only loop is posted as the text the engineer typed (`15 min`, `900`) and the engine reads it (C12).
The step-by-step setup asks its questions in the order the engine names them through `controlStudio/setupStep`: the engine decides each question's order, its choices, what an answer must be and the refusal of an answer, which the question shows again with its words; the extension shows each question in VS Code's picker or input box and sends the answer back. Back returns to the nearest earlier answer the engineer may change: the answers before it are replayed without asking, and the questions from it on are asked again with their earlier answers shown. A changed answer discards the answers after it. When every answer is in, the engine answers the draft and the sentence the engineer confirms, which says what the loop does with the program; `setupSave` then writes the draft with the structure `step-by-step`: the engine builds the loop from the answers (`setup/configure.rs`), checks it, writes it, and answers the open request of the loop, which opens Control Studio. The answers name their identities (a controller by its instance, a signal by its address, a symbol by its name), and the engine refuses an identity the compiled program does not hold under its question. An ON/OFF controller and several channels are parked (V7): the engine refuses them with its sentence. A save and a binding selection answer the open request of the loop they write or select (`openRequest`, `selectedRequest`); the page opens the loop from that answer and never reads the loop file.
A binding selection is refused while an operation of the engine works on the selected loop: the refusal names that operation and waits for it to end (A8.5). A binding selection opens the selected loop with its own study; no model, candidate or result is carried from the loop shown before (each loop's study holds its own identification, A8.2).
The discover answer names the structures the page offers, in order (`structures`), and the PIDs whose program needs the startup-done
flag at the settings the form starts from (`startupFlagInstances`, C1 Setup). The page shows the flag for a chosen instance in that list,
when a refusal names the flag, or when the engineer has chosen one; the engine checks the flag on save whatever the page shows.
The discover answer also holds the form's preselections (`defaults`), the row counts it draws (`limits`) and the words it shows (`wording`); the page renders them and keeps no default, limit or word of its own. The preselections are the structure the program shows (so a program with several library PIDs preselects `group`), the state-feedback design LQR, the search family MPC, a kept value of a search starting at zero, the Auto/Manual signal's TRUE value meaning Auto, the controller action direct, the ON/OFF band half-band and the threshold policy at-or-beyond, and the own law PI (PID for a trajectory feedforward, whose program law the form does not offer to change). The row counts are the generators' own limits: further measured outputs, estimator states, search coefficients and kept values, and the fewest group members (two; a group has no largest count, since its decoupler has one row and column per member). The gain schedule's operating-point rows are the engine's list (`schedulePoints`: four rows, since a schedule has two to four points, and each row says whether the setup asks for it: the first two are required and the others are offered once the point before has a name), and the shape of a PID group's decoupler is the engine's `decoupler` (a row and a column for each library PID the program holds). The page draws the rows and the matrix the answer lists and keeps no count, flag or shape of its own. The words are "TRUE means" and the two lines a missing value shows, "Not found in the code" and "Not stated in the code" (a task). The loop name the form starts from for a chosen controller is the last segment of the controller's instance path (`loopNames`: `P1.Controller` names `Controller`); the engineer may type another name, and a blank name is refused by the engine.
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
  compiled types; a PID-designed loop names the compiled block's type (`Control.PID`), which discovery compares with the
  compiled program, while a project-designed loop's type describes the controller its program owns (for example "LQR state
  feedback" on the program `Main`), so discovery checks only that its instance is compiled and runs in the loop's task;
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

Loop files are written at schema version 2. Where version 2 holds one fact in two fields
(the mode signal in `modeSignal` and in `parameterBinding.mode`, the period in
`taskPeriodMs` and `nominalPeriodMs`), the loader requires them to agree and
rejects a definition where they do not; setup writes both consistently.
`trackingSignal`, `maximumJitterMs` and `operatingPoint` are read and ignored,
and are no longer written. A version 1 file is read as version 2 without being
rewritten. It may not declare a key that only version 2 has (the parameter
binding and its purpose, the law, ON/OFF, state feedback, the program's own law,
PID groups, channels, verification scenarios, declared analyses, and the
experiment's abort, baseline and hold parameter); a file that does is refused
with the reason and **Configure control loop** offered. Its other keys mean what
they mean in version 2, so the version 1 observation loops of lessons 01, 02, 03
and 80 open as C12 loops.

### B2. Observing

When a study opens on a connected target, the page shows the selected signals
live, read by symbol (A5), before and during every operation. The chart grows
as samples arrive. A stale or unavailable value is shown as such, never as a
current value (A3.4). Live observation reads the runtime's coherent scans; an
editor poll is never evidence. The engine's `controlStudio/monitor` decides availability: a scan that has not advanced for more than the connection-loss timeout (5 s, or three resource cycles when that is longer), measured on the host's clock, is stale, and the page shows the engine's answer. The answer is given at once and never waits for the runtime: it is the sample
the last read gave for the panel's loop, or a pending sample until the first read has come back. A call that finds no read in
flight starts one on a background thread, so the next answer carries that read's scan.

### B3. Identify

**B3.1 Purpose.** Produce a provisional model of the plant from one live
campaign, from an opened recording, or from a supplied model.

**B3.2 Inputs.** The selected plant inputs and measured outputs, the method
(Automatic or a named method), the limits (plant input range, permitted
measured-output range, maximum duration, optionally the expected settling time)
and the identification policy. For a loop the engine serves, the Limits editor
checks no range itself: the engine refuses a range beyond the loop's declared
limits, and a duration outside one second to 24 hours or one that no time reads,
under the field of the editor that holds it (`inputMaximum`, `outputMinimum`,
`durationSeconds`; with several channels the field carries the signal's symbol,
as `inputMaximum:P1.MV`), and the editor shows that sentence under the field. A range bound is sent as the number or the text the
engineer typed, and a duration as its text: the engine reads them, and refuses a bound that no number reads (blank text included) under
the field of the range's maximum, as it refuses a time that no duration reads under the duration's field, so the editor refuses no
blank or unreadable value itself. The editor starts from the limits the open answer gives (`experimentLimits`): every plant input and
every measured output with its range, in channel order, and the maximum duration, which the answer writes in words (`durationWords`,
B9.9) that read back as exactly the duration: the editor shows them as they are, and the entry goes back as typed. They are the loop's declared limits with the study's own
record over them (A8.6), so the page merges nothing: each plant input's declared range, narrowed by its actuator range and, for the
manipulated variable, by the loop's output limits; each measured output's declared range; and the declared duration, where the study
records none of its own. The study keeps the ranges of its acquisition record by signal, one entry for each plant input and measured
output (`inputRanges` and `outputRanges`, by the signal's symbol), and the page sends every signal's range so: it encodes no single
range, no choice between the single and the per-signal form, and no reuse flag. A study kept before ranges were kept by signal still
reads: its `inputRange` and `outputRange` are the ranges of the loop's manipulated variable and process variable, a range named by symbol
takes precedence, its `reuseRecordings` is ignored, and its method `configured` is shown as Automatic. A duration that the loop does not
declare and the engineer has not given goes as the blank text of its field. No TypeScript rule computes the limits.

The Method list shows the method the study holds: the open answer names its entry (`heldInputs.identifyMethod`, by the word of the
method's offer), and a study that records no method holds the list's first entry. Each offer says whether the study records the method as
an acquisition setting (`records`): every live method does, and the Recorded-data fit, the supplied model and the project test do not,
because they start no campaign of the study's inputs and are the page's view. A method whose offer says so goes to the engine when the
engineer chooses it, with the limits as they stand, and the engine records it or refuses it with the rule it breaks: a relay without its
declared direction, or a duration that the loop does not declare and the engineer has not given, which is the Limits editor's refusal
(under the duration's field, with Limits as the next action). A refused change leaves the method the study held before, and Identify runs
that one, so the list shows it again once the engine has answered, and the refusal's sentence stays on the page. A method the engineer
has replaced by another choice meanwhile is not put back: the answer to the newer choice decides. A refusal of a limit is the Limits
editor's: the editor reopens with it, and its Done sends the method again with the corrected limits.

**B3.3 What Automatic chooses.**

- A loop in Auto with a declared setpoint: a **setpoint step** in Auto
  (closed-loop identification). Its size is the larger of 20 times the measured
  noise and 2 % of the measured output's range, capped at 10 % of the range and
  by the headroom to the permitted measured-output range.
- A loop in Manual, or an open-loop experiment loop: a **plant input step** in
  Manual (open-loop). Its size starts
  at 10 % of the plant input's range, capped by the headroom to the plant
  input limits. A **pulse** replaces the step when sustained holds are not
  permitted. An on/off actuator steps between its two levels. Several plant
  inputs are stepped one after another in one recording; **PRBS** is used when
  the policy asks for frequency information.
- A loop in Auto without a setpoint binding: no safe automatic method. The
  bumpless mode change of A6.3 tracks the reference, so without the setpoint
  binding Identify cannot put the loop in Manual; it says so and names the
  missing binding. A method the engineer chooses that needs the other mode is
  refused the same way.

The engineer may choose a method instead: Step, Pulse, Setpoint step, Feedback
relay, PRBS, Multisine, Recorded-data fit (through **Open recording…**) or
Supplied plant model. Every live method follows B3.4. Recorded-data fit is the manual method, with steps of its own: the
engineer's form, the check of a kept recording, the fit (which a Cancel stops), the sensor-lag structure where the comparison
recommends one (the page shows the engine's `modelStructure` whenever the answer carries one and checks none of its fields), and the
acceptance (§0.1a). The form starts from the engine's default numbers (`form` of the manual identification's
answer): the held-step windows of 5 s, the window limits and lattice steps of N2.2 to N2.6, the delay grid from 0 to 2 s, the
time-constant grid from 0.1 to 20 s, the additional-lag grid from 0.1 to 10 s, and the model error limits 0.2 and 0.1, each in
process units; the page's template holds no number. The engine decides each step and its label, and the fit reads a kept recording
without live excitation (B3.6). A step the engine refuses (a recording the check does not qualify, a validation the model fails)
answers its reason together with the evidence it scored, the step-response measurement and the prediction against the independent
recording; the page shows the reason in its status line, keeps that evidence in view beside it, and offers the primary action the
answer names, and the model is not accepted. The prediction's error rows are the engine's numbers: the fit's root mean square error and
the largest absolute residual of its samples (`maximumAbsoluteResidual` of the answer, absent when the answer shows no fit). The page
computes neither. While the manual identification awaits its independent recording (its stage is the preview),
a project test (a run of a project-scripted loop, which the page asks for with the Automatic method; C12) keeps its recording and selects it for
the validation role. It records and fits nothing, keeps no model of its own, and its answer says that the recording is the manual
identification's independent recording and that the model is not refitted. An automatic plant run keeps its model as before and
clears the manual section (B3.7).

**B3.4 The live campaign.**

1. **Preconditions.** A connected target (a project test's review that finds no
   runtime running is answered with the next action that applies, A2.8); the selected signals fresh, good and
   not forced, read from one coherent scan (a read that took over the runtime's inspection watch waits for the
   next scan, runtime protocol annex E3 rule 8, so another reader's watch never refuses a run); the limits present (otherwise **Limits…** opens at the missing
   field); physical approval on a physical target (A2.5). A Manual method
   switches the loop to Manual through a transfer that holds the present output
   (A6.3) and returns it to its starting mode afterwards (A6.2). The approval
   block of the review (B9.3) names what the campaign may do: each signal it
   writes with its limits (the plant input range of B3.4 step 1 for a plant
   input step, the setpoint's and the output's ranges for a setpoint step, the
   declared range of the writable disturbance of a load step, and the
   activation of a project-scripted test, whose limits are not declared),
   the amplitude (zero when Identify sizes the step after the steady baseline,
   B3.3), the maximum duration, the stop conditions of step 6 and of B3.9, and
   the restoration of step 5. It shows limits, not measured values: the steady
   baseline (step 2) sets the operating point, so the values are not known
   before the campaign starts.
2. **Steady baseline.** The baseline window is the longer of 5 % of the maximum
   duration and, when declared, 10 % of the expected settling time, capped at
   30 minutes, and never shorter than 20 samples: a slow process recorded at a
   long record period (A4.2) keeps the 20 samples its trend and noise are
   measured on, even when they span more than 30 minutes. The measured output is steady when its linear trend
   across the window moves it by no more than four times the measured noise,
   or 0.001 % of its range when that is larger. A tolerance tied to the range
   would let a short window call an output that still moves fast steady, and
   the step would then fit the earlier response's tail. Identify waits for a steady baseline for up to the longest
   of 5 minutes, 20 % of the maximum duration and two baseline windows, so that a steady output is judged over at least
   one whole window. A window here takes the maximum duration and the expected settling time, but not the twenty-sample
   floor, which the recording holds within the wait at the record period of A4.2 while one task execution is at most a
   twentieth of the wait. It then stops with the rate still observed, offering to wait or, for a loop in Auto, a
   setpoint step. A project-scripted experiment that declares a baseline waits for none of this (C12): the project owns its
   scenario and qualifies its own measurements before it actuates, so Identify starts the recording and the plan together, and
   the baseline that the project declares is the one its plan records. A project-scripted experiment that declares no baseline
   waits for the steady baseline as above, so that its fit has the samples before the first input change that it takes its
   operating point from.
3. **Step.** The runtime applies the step at the next execution of the loop's
   task (A3.5).
4. **Hold until settled.** The response has settled when, after it has left the
   noise band, it changes by less than the larger of twice the noise and 2 % of
   its change so far across the settle window: the longer of 20 samples and
   10 % of the hold so far. The hold ends when the response has settled or when
   the maximum duration, minus the restoration allowance of one baseline
   window, is reached. A project-scripted experiment records its declared
   duration, or the recording of its applied hold when its loop declares a hold
   parameter (C12), and leaves no allowance: its plan lasts that duration. The
   runtime counts a duration to the execution at which the restoration takes
   effect (annex E5.4), so the activation is in force from the end of the
   baseline, at the earliest the second execution of the loop's task, up to the
   last execution before the end, and the end is the one restoration sample. A
   declared 30.1 s is the 30 s hold of the project's program, all of its 300
   scans at a 100 ms task, and that sample. In a closed-loop setpoint step the plant input is part of the
   response: while a slow mode settles, the controller keeps moving the plant
   input although the measured output is back near its setpoint, and a fit
   ended there misses that mode's gain. The hold therefore ends early only
   when the plant input has settled too, by the same rule against its own
   mean and noise over the steady baseline. The maximum duration counts from the end of the
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
recording. The recording of a project-scripted experiment that declares a baseline has no wait: it covers the
declared duration from the start of its plan (C12). The chart grows as samples arrive. Abort stops the experiment through the runtime's
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
The study keeps the opened file as one of its recordings (`importRecording`), so
reopening shows the fit, and Fit refits it (§0.1a); a Recorded-data fit (B3.3) keeps its file in the same way, and its stage is
the study's `identifyManual` section (§0.1a). The file names no experiment that Control Studio ran, so its fit
reads its own samples as an observation (C12): nothing is written to the plant.
The engine gives each selector its channels (`recordingCatalog`'s `inputs` and `outputs`): a plant input is offered on the inputs
selector and a measured output on the outputs selector, and a signal that the loop declares on the other side is offered on neither;
an extra output that the loop does not bind stays selectable. A legacy file offers its applied input and its measured output, and the
engine words them (`label` of the catalog's channels, "Applied input" and "Measured output"). A loop
without a declared setpoint offers the setpoint choices of its open answer (`setpointChoices`): its additional signals with the setpoint
role and the process variable's unit, and no others (B3.6).

The binding selectors take their plant sides from the open answer (`liveSides`) and the loop a mapping names from the engine's mapping answer (`controlStudio/setupDiscover` with `mapping`, §0.1a): the page matches no loop and no recording itself. The first list at open names the loop the panel shows (`loopId`), and the engine answers that loop's own mapping, so the Controller selector offers the other compiled controllers that bind the same plant signals at once; that answer finds no problem and clears no message of the page. A changed mapping goes to the engine, which answers the loop it names, its controller choices, whether the mapping blocks the work (`blocksRun`), the sentence of an ambiguous or unmatched mapping, and whether a recorded selection changed; the page gates its actions on `blocksRun` and refits a recording only when the answer says that it changed. The Recorded-data fit reads the file it kept with the loop's own signals (B3.3) and takes no signal selection, so a changed selection of an opened recording starts no refit while Recorded-data fit is the chosen method, and its manual identification is never replaced by one. An identify request sends the setpoint identity that the engineer chose (`setpointSignalId`) whatever the method, and the engine reads the loop from the project, so the page builds no loop; the engine uses the signal for the setpoint step alone and runs another method as if none were named.

**B3.7 Supplied model.** A transfer function or state-space model with its
channels, units, operating point, delay and sample time enters as a provisional
model unless it carries evidence of independent validation (`importModel`,
§0.1a). It starts no campaign, so B3.4 does not apply to it. The study's current
model is the later of its Identify result and its supplied model (A8.4): `importModel`
makes the supplied model current, and a later Identify makes its fit current; the
earlier model stays in the study as history. A Recorded-data fit accepted by its manual identification (B3.3, §0.1a) is an
Identify result and becomes the current model in the same way. Reset clears both models and the manual identification's
section (A8.4). Any other model that becomes the current model clears that section in the same save: an Identify result from a
live run, a re-fit or a pair fit, or a supplied model. So reopening never shows a manual model that Tune does not read, and an
accepted manual model keeps its section, because it is the manual section's own model. A project test recorded for a manual identification's independent recording (B3.3)
keeps its recording and no model, so it changes no current model. Every stage
reads the current model, so Tune designs on a supplied model. A supplied model was
not fitted and has no normalized error, so Tune's error check (N5.7) does not apply
to it.

**B3.8 Result.** Identify shows the model type and equation with the identified
coefficients, and the measured response with the model's prediction (Identify
page specification). The model type, the key numbers and the equation split into its gain, its
denominator and its delay are the engine's words (`modelType`, `keyNumbers` and `equationFraction` of the
manual identification's presentation; `modelType`, `equation` and `keyNumbers` of an Identify answer): the page lays
them out as a stacked fraction and a line, and composes none of them. The status line under the result is the engine's sentence
(`statusSentence` of an Identify answer): "Model identified.", or "Provisional model identified." for a provisional model (B3.5); the
answer of a Reset carries "New identification study started." (A8.4). The page writes none of them. The rate gain of an integrator is in
the output's unit per the input's unit per second (numerical annex N0.3).

**B3.9 Failure states.**

| Cause | Next action |
|---|---|
| No target, or connection lost | Connect a runtime |
| A limit missing or invalid | Open **Limits…** at the field |
| A signal stale, bad or forced (named) | Wait, or remove the force |
| Physical approval declined | None |
| Physical target, and the campaign has no approval of its reviewed identity yet (A2.5) | Approve, from the approval block the review shows (B9.3) |
| The campaign changed since it was reviewed: its loop, its mode or its settings (§0.1a) | None; the review is asked for again |
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
Feedback relay record of the same study. The Tune page offers the relay rule beside Manual only for a `Control.PID` law whose study holds a qualified record: the open answer and the answer of each identification carry that offer as `methods` and `methodTitles`, because `setupDiscover` cannot read the study (C4, N5.R). A supplied model is not fitted, so it
has no normalized error to check (B3.7); the rest of the rules of its method
apply to it as to any model. The pinned settings of a request of `controlStudio/tune` or `controlStudio/preview` name the loop
revision they belong to and their values, not a family. The study gives the family: the structure the request names, else the
family of the study's candidate, else the family of the study's recorded pins. With none of them the request is refused with the
sentence `Choose a controller configuration before keeping settings.`, and that sentence is shown under the family field. A state
law's pins are its design's (C4): the engine ignores the pins a request sends for a state law, so such a request is never refused for want of a family.
The request names the family the engineer chose in the Controller selector as a word (`structure`), and none otherwise: the page sends the engineer's choice
only, never the family the selector shows by default. The engine reads the word: `P`, `PI`, `PD` and `PID` fix that structure, and any other word (the family of a
state law or of a program) fixes none. A request that names no structure designs for the structure the study's inputs record (A8.1), which the Controller
selector records when the engineer chooses a family (Automatic clears it); the manual method, whose entered settings belong to a family, takes with none
recorded the family the selector shows (the family of the study's candidate, else the family of its recorded pins), and so does the manual preview; every other
method, and Automatic, leaves the structure to its own design, so that an earlier candidate never fixes the next one (B4.4). The page sends the settings it
enters with the manual method alone, as typed, and the engine refuses settings sent with any other method with the sentence `Settings are
entered only with the manual method.`. The engine reads each entry as a number or as the text of one, in the change to the study's inputs
(`tuningPreferences.values`) as in the request of Tune: it records the entries that a number reads, and leaves out an entry that no number
reads, a blank row or a value still being typed, which Tune and the preview refuse under the setting's own symbol. A method word that the engine does not run is refused by the engine, with the sentence `The engine
does not run the tuning method the page chose.` when it arrives in the change to the study's inputs, which the page makes before it starts a
Tune. The route decides none of these.

The open answer gives the Tune page its controller (`controllerOffer`, C2, C4): the families the selector lists with their labels,
the family it shows until the engineer chooses one (the family of the study's candidate, else the family of its pins, else Automatic),
and the rows of the manual editor, each setting that a family takes with its label, unit and bounds. The page renders these rows and
keeps no rule of its own. An entry the engineer types is sent as typed: an entry out of its bounds, or one that no number reads, is
refused under the setting's own symbol, and the page marks that row (B4.5). The design form of a state law starts from the declared
matrices that the open answer gives (`stateDeclaredText`, C4) when the study holds no draft.

Every design's Tune answer carries its words (`words`): the single loop's, the state law's and the switching controller's alike, in the live answer of `controlStudio/tune` and in the candidate that `open` rebuilds, so the page writes none of them. The answer's `words.gainLabels` names the three gains of `Control.PID` (proportional, integral and derivative) for the structure tables of the Tune page (C6, C7, C9, C10). The page keeps no copy of those names.
Its `words.settingLabels` give each row of the PID path its label and its unit (`unit`, as the parameter binding declares it), and a state law's finite
horizon comes with its words (`words.horizonPolicy`, the line that names the steps and the policy, such as "Finite horizon: 3 steps · Repeated first
gain", and `words.horizonStages`, the heading of the stages, which a pin makes "Nominal Riccati stages"). A choice the engine offers comes with its words
and the page keeps no label table: the solver policy of the design form lists its options as the field's `choices` (`value` and `label`) with the one that
stands as `selected`, and the models of the Recorded-data fit as `modelChoices` (B3.3), each with its `label`. A value the engine does not state is
shown as none: a manual form with no family shows no row, a gain the answer lacks is a dash, and a setting without a unit shows none.

**B4.3 Requirements.** Declared rows: settling time, overshoot, steady-state
error and error band, input disturbance, measurement noise (when a filter is
tuned), and actuator, rate and state constraints. Overshoot is stated in
percent of the test setpoint change and is checked only when it is declared:
a blank overshoot row declares no overshoot limit, and Tune never generates
one. When a test criterion is missing, Tune fills it from the model as the Tune
page specification describes. Automatic criteria are marked as such, frozen before validation and
never relaxed by its results. A duration typed in a requirement (the settling time, the validation duration and each output's settling
time) is read by the engine as typed (`90 s`, `1 h 30 min` or plain seconds, as the Durations rule of the display section says); a time it cannot read is refused under its
field. The engine refuses each requirement value with its
own sentence under the field it names, and says whether a declared low-pass or
notch filter needs the measurement noise band. Settling times and observation
windows may be as
long as the limits of A7 allow.

The response of a formula method (N5.4) is the requirements' declared response,
`tuningObjective`. Automatic marks it when it fills it, and a response the engineer
declares replaces that mark. The verification scenarios that the Requirements editor
declares (B9.9) are part of the requirements the study holds: Validate runs them in
place of the loop's own declared scenarios, and a changed list leaves the validations
behind (A8.4).

A loop with several controlled outputs holds each output to its own requirements. The
record's `channelRequirements` holds one record for each controlled output, keyed by the
output's model channel identifier: its setpoint change, error limit, settling band,
settling time and overshoot limit, each checked by the rules above. A refused value
names its field with the output's channel, as the requirements page names that output's
row (`settlingBand:P1.Zone2C`). The record's own values are the first output's copy. The
validation duration outlasts the longest settling time of the record and of its outputs.
Validate holds each output to its own record, else to the loop's own requirement for
that output, else (for the process variable) to the record (C7). A change of the
requirements or of the declared scenarios is refused under `durationSeconds` when a
declared check ends after the validation duration, with the sentence that the campaign
gives for it.

The Requirements editor sends what the engineer typed, and the engine assembles the requirement record when it merges the change
(`controlStudio/inputs`, §0.1a). For one controlled output the rows go as the record's own values. For several, the editor sends
`outputRequirements`: for each output, in the editor's order, its `channel` and its rows (the setpoint change, the error limit, the
settling band, the settling time and the overshoot limit), with the shared values beside the list (the validation duration, the load
trial, the noise band, the gain uncertainty and the Auto-entry tolerance), and the declared response and the record of the generated values
as the engine answered them. The engine writes each output's rows under its channel in `channelRequirements`, writes the first output's rows as
the record's own values (the first output's copy), keeps the shared values, and refuses a list that is empty, an output without a channel,
a channel named twice, a row that an output does not have, or the rows given both ways. It reads every number given as text as it reads the
number (`number_or_text`), reads a duration as the Durations rule says, and takes a blank optional row, or `null`, as no value. A value that
breaks a rule is refused under its field, with the output's channel when it belongs to an output, as above. A row of plain numbers that no number reads, a blank required row included, breaks a rule too: it is refused under its field with a sentence that asks for a number (`Enter the error limit as a number.`), and the editor refuses no blank or unreadable entry itself. The editor's rows for each output
come from the engine's answers: from the record the page holds for the output's channel and, until the study holds one, from the open answer's
`requirementOutputs` (§0.1a). The page assembles no record, copies no output's values to another and decides no row's start; after the
engine refuses a change it shows the rows as they were typed, with the refusal under its field.

A change of the requirements keeps Automatic on each generated value that it leaves as it was: a value the engineer changes is no
longer generated, and the engine decides which values those are. The verification scenarios of the
Requirements editor are checked by the engine when they change: a list that no scenario reads is refused under the field
`scenarios`, and so is a scenario event written after the validation duration, and a timed change whose time is not a whole number of
the loop's task periods (Validate writes an event at a sample of the task, so the engine refuses the time when the list changes, not when
Validate runs). The page sends what the engineer typed, as text: each number and each name as text, and each initial value as a row of a signal and a value, so that a repeated signal reaches the engine. The engine reads a number given as text as it reads the number (`number_or_text`), and refuses under `scenarios` the first rule that the list breaks: a repeated scenario name, a blank or repeated signal, a number that no finite number reads, a timed change before zero, or a timed change off the task's samples. A new scenario is the engine's template (the process
variable's declared range as bounds over the validation duration, with no name, no initial values and no events). The page gives it no
name: a scenario whose name is blank takes the first identity `scenario-N` that no other scenario of the list holds, in the list's order, when the
list changes. A new initial value or timed change starts at the signal that the open answer names (`scenarioRowSignal`, the loop's
setpoint; blank when the loop declares none), and the page checks it no further: the engine refuses a blank or repeated signal. The
advanced fields of a scenario (its requirements, effects and model) travel as the text the engineer typed, in the scenario's `advanced`
key. The engine reads the text as a JSON object that replaces the scenario's other fields, and refuses under `scenarios` a text that is
not a JSON object, or that holds the scenario's name, initial values or timed changes, which the form edits. The page does not read the
text; while advanced text is typed it replaces the scenario's bounds, so their rows are not shown.

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
When a runtime is connected, the current settings are read from the running
controller as B5.3 reads them, and predicted beside the candidate on the same
conditions and time base; without a runtime only the candidate is predicted.
A failed read, values that give no proportional gain, or a run of the current
settings that cannot be simulated leaves them out, and Tune does not fail for them.
The settling time, the overshoot and every assessment use the candidate's full
prediction alone.
When the prediction holds more than 6,000 points, the response the answer
carries for the plot is reduced: the points are split into 1,000 consecutive
groups of equal size, and each group keeps, in time order, the points where
each trace takes its lowest and its highest value in that group; the first and
the last point are always kept, so no peak, step or limit the prediction
reaches is lost from the plot.

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
blocks Apply. The qualified model's validation recording is the check's
recording, and its validation error is the check's error over the whole
response: Tune reads that error for a qualified model (B4.2), and a reopened
study shows its candidate on it (A8.6). A supplied model has no estimation
recording, so its check runs the campaign that Automatic chooses for the loop
(B3.4), and the model revision the check names is the supplied model's (B3.7).

**B5.3 Actual-ST controller validation.** The current and the candidate settings
run in isolated simulations (A2.6) under one immutable validation-conditions
record (numerical annex):

- against the project's configured simulated plant when the target is simulated
  and that plant drives the loop's signals: the loop names the plant, and the
  plant writes the loop's measured output address and reads its plant input
  address directly;
- otherwise against the qualified model. A plant that reaches the loop only
  through the program's logic, such as a scaled transmitter signal or a
  heating sequence, is validated on the model, because an isolated run of it
  would start at time zero from the plant's configured state and the program's
  initial state (A2.6), not from the operating point. On the model, each run
  starts at the model's operating point, with the controller's states
  initialized there (B6.2).

Validate tests the candidate at the operating point Apply starts from (B6.2):
the selected model's operating point. An isolated run of the configured plant
starts from the plant's configured state. When the current settings'
target-response run starts with its measured output outside the settling band
around that operating point, as after an Apply whose target change stays in
force, the configured plant cannot show the response there: Validate runs every
case on the qualified model instead, and the result names the plant each case
ran on and why. Otherwise each case runs where it belongs: a case that keeps its own model or its effects, such as
a declared scenario with its own model or effects, or a generated model variation, runs on the selected
model from that model's operating point, and the other cases stay on the configured plant; the result
names the plant of each case.

Part C names structures that always validate on the model. The scenarios cover
the applicable setpoint step, load step, noise and model variations, and, for
Auto commissioning, the Manual-to-Auto entry. Long processes are simulated for
their full observation (A10.5). The candidate reaches the program as Apply
delivers it: a program that sets its own parameter record during its first
execution has the candidate written again at the task boundary after that scan,
and must keep it from there (protocol annex E8.3). The current settings are the
record's values as the running controller holds them, read before the runs, and
reach the program the same way. An Auto commissioning is a validation whose Apply
enters Auto, which every Apply does (B6.1, A6.2). Its Manual-to-Auto entry case, named
so in the table (B9.9), runs on the plant of the other scenarios, from the point where
that plant starts: on the model, the selected model's operating point; on the configured
plant, the plant's configured state, as the first sample of the current settings'
target-response run on that plant records it (its measured output and its plant input).
Where that sample has no measured value of either, the case starts at the model's operating
point. The case is planned after that run: its reference and initialized states are set for
its start, with the candidate's record and the declared initialization. It judges its first
output, mode, gains and signal bounds under the Auto-entry allowance of A6.4 that Apply uses;
Apply needs that case to pass (B5.4). A loop whose Auto entry cannot be planned still runs its other scenarios, and its case is Inconclusive with the refusal as its reason, so Apply is unavailable (B5.5). A failure to read the study or the measured noise is a failure of the validation itself: Validate ends with that failure, and the case is not answered Inconclusive.

**B5.4 Results.** Every requirement reports Pass, Fail or Inconclusive with its
measured value, limit, unit and source. A scenario that did not run is
Inconclusive; missing evidence never passes. **Apply** becomes available only
when the model check passed and every mandatory requirement passes for the
current candidate.

The result states these rows in these words (B9.9):

- The independent model check, when this validation ran it, is the first row:
  its normalized error and its limit in percent of the measured output's span
  (N3.4). The row is named Model agreement.
- A signal or state bounds requirement (N8.3: Operating range, State range, or
  a fixed parameter named "Parameter unchanged") has as its measured value the
  lowest and the highest good value of its channel in its window, and as its
  limit the declared bounds. When the two bounds are one value, the limit reads
  "= value". The verdict of the check is unchanged.
- A settling time or a maximum error that Automatic generated, as the criteria
  record names it (B4.3), is marked Automatic in its row. A declared limit and
  an overshoot limit are never marked.
- The candidate's overall row is named Candidate overall. Each scenario's runs
  are named Current settings and Candidate.

The Validate plot shows the measured output of the target-response case: the
current settings' run and the candidate's run, each as its isolated simulation
recorded it. The answer carries the plot bounded as B4.5 bounds the Tune plot
(B9.6); the exact samples stay with the run's record.

The plot and the plant reason are answer-only: the study keeps each run's verdict, its
requirement results and the plant each case ran on, but neither the samples nor the reason
the model replaced the configured plant. So a reopened loop (A8.6) shows the table and
whether Apply is available, and no plot and no plant reason, until Validate runs again.

**B5.5 Failure states.**

| Cause | Next action |
|---|---|
| Model check failed (error and limit) | Identify again; the message says what to change (method, step or duration) |
| Requirement failed (value and limit) | Tune, or open **Requirements…** |
| Inconclusive (the missing evidence) | As the reason states |
| Isolated simulation unavailable (reason) | As the reason states, for example reload the program or start the local analysis runtime |
| Isolated simulation failed or over its wall budget | None; the reason and any captured prefix are kept |

A failed requirement's failure names its measured value and its limit in the unit of
the requirement's result, written as the table writes them (B9.9). The recorded
target error reads `Recorded target error 0.8935 °C exceeds 0.1 °C.` A response that
did not settle reads `The recorded response did not settle within 15 s.`, and, when
it settled late, goes on `It settled after 20 s.` A requirement of declared bounds,
an operating range, a state range or the first output after a Manual-to-Auto entry,
names the range the channel took and the bounds it had to stay within (`Operating
range: 12–104 % is outside the declared 0–100 %.`, `First output after Auto entry: 5 V
is outside the declared 0.9–1.1 V.`), and a fixed value the value it differed from
(`Parameter unchanged: 3.5 differs from the declared 1.`). Any other requirement is
named by its label (`Maximum overshoot: 12.5 % exceeds the limit of 10 %.`; for a
limit the value must reach, `Minimum ON time: 3 s is below the limit of 5 s.`). A
failed requirement whose result holds no number says what it found instead. The
measured-disturbance case (C8) keeps its own sentence, which names the largest
deviation and the allowance.

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
validated and current value, writes nothing meanwhile, and continues by itself
once the outputs that do not rest have all stayed inside their bands for the
settle dwell, in process time. The settle dwell is the settle window of B3.4
step 4 with the expected settling time as the hold: the longer of twenty samples
at the loop's task period and a tenth of that settling time. A reading that
takes any output out of its band restarts the dwell. The wait can be cancelled
and lasts at most the longer of 60 s and three times the test duration, in
process time. An integrating output takes its present value as its operating
point and rests wherever it is, so it does not hold the wait. A PID group's plant
inputs and measured outputs are every member's, in the model's channel order
(C7), so Apply waits until every member's output has settled.

**B6.3 Observation.** After the transfer, the validated target change runs and
the runtime records the response for the test duration in process time. The
recording is planned longer than that by the confirmation window of the target
change, two executions of the task and three record periods (A4.2), because the
response starts at the first sample whose executions all lie at the target, up
to one and a half record periods after the change, and the recording stops once
the response covers the test duration from that sample. The observation is owned
by the runtime and reattached on reopening (A8.5). The
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

Restore is one operation for every case: in the same session, after the panel
or VS Code was reopened, after a measured output left its permitted range, and
after an Apply was interrupted at any step. Before its first write, Apply
journals its commissioning checkpoint (A8.5): the pre-Apply parameter revision
and values, the mode it found, the bindings it writes through, and each
transaction's identity as it is dispatched. Restore always starts from that
checkpoint and the runtime's current status: when the loop is in Auto it first
enters Manual through a transfer, then rolls the record back to the retained
revision (protocol annex E7.5), then returns the loop to the mode found before
Apply, entering Auto with its state initialized from current observations and
the Auto-entry allowance of A6.4, and then returns the references Apply found
through a reference step, as Apply's target change is made (B6.3, annex E6.4).
A step the runtime already confirmed is not repeated. A completed checkpoint authorizes Restore and Keep only while the
runtime still reports its parameter transaction and its confirmed Auto transfer
under their journaled identities; otherwise the settings changed since Apply and
Restore is unavailable. When the runtime was restarted or its program reloaded since Apply,
nothing of the checkpoint can be restored: Restore says so, the checkpoint is
kept as history, and the loop continues from a new validation. The engine
decides this when the Restore is asked for, with or without a review identity,
from the program revision of the parameter record, which changes with the
session epoch (annex E9.3), and answers at once: the Restore's outcome, not
restored, with the sentence that says so. It writes nothing to the runtime and
starts no operation. The loop's journal keeps the checkpoint as history,
retired and naming the replacement's program revision, until a later Apply
replaces it; the study keeps a receipt that says so beside the Apply's receipt
when it holds one (A8.1). A retired checkpoint grants nothing and holds no stage
back, so the block below ends: the answer carries the stages without the block,
and offers neither Restore previous settings, Keep in project nor Apply
until a new validation passes (B7). The engineer's Restore is the explicit confirmation that
annex E11.7 asks for a replaced runtime: it does not claim that the Apply
completed, and it replays none of its writes.

**Reopening and recovery (A8.6, B6.4).** When the panel opens a loop, the engine
answers the Apply offers without asking the runtime. Restore previous settings is
offered while the loop's journal holds the Apply's checkpoint and it is not closed
(recovered or retired), which is the record Restore starts from, so an interrupted
Apply offers it too. A Restore that the runtime confirms closes the checkpoint: the
study keeps the Restore's receipt beside the Apply's (A8.1), and the journal clears the
checkpoint. An Apply interrupted before its receipt was kept has the journal as its only
record, so the journal keeps its checkpoint as history, recovered, until a later Apply
replaces it: it grants nothing and holds no stage back, and it keeps the validation the
Apply applied, for which Apply is not offered until a new validation passes (B7).
Keep in project is offered while an Apply's settings stand (its checkpoint completed) and the study's Apply record allows it, as
`controlStudio/keep` decides it (B6.5). A journaled observation that the runtime
may still record, and that no operation of this engine follows, is offered as
**Resume**: the engine follows it to its end and writes nothing meanwhile (A8.5,
A8.6); the engine refuses the Resume while an operation of the same engine holds
the loop. The Restore and Keep reviews still refuse what changed since Apply.

An Apply or a Restore that stopped before the runtime confirmed its end leaves its
checkpoint unresolved (stage prepared, recording, parameters or transfer), and new
work cannot bypass it (annex E11.7). While such a checkpoint waits for its Restore,
and no operation of this engine works on it, the engine blocks the loop: the stages
whose action writes or changes the study (Test, Identify, Tune, Validate and Apply)
cannot start, and each of their statuses names the block's reason (`reason`, B9.4),
so the block holds after a reopen. Monitor, which only observes, is not blocked.
The engine refuses Identify, Open recording, Tune, Validate, Reset, the imports and
every change of the study inputs (`controlStudio/inputs`, A8.4) with that reason and
the next action Restore previous settings, and refuses a new Apply with the next
action Restore by its review. The reason reads "An Apply or a
Restore of this loop stopped before the runtime confirmed its end. Restore previous
settings first: until the runtime confirms it, the loop's other actions wait.", and
for an observation the runtime may still record "An Apply of this loop was
interrupted during its observation. Resume it to see its result, or restore previous
settings: until then, the loop's other actions wait." The offers name Restore
previous settings in the Apply position (`primaryAction`, B7), and a failure whose
typed next action is Restore (A11.2), such as a Restore that the runtime did not
confirm, shows that action as its control: both ask the engine for the Restore
review, and the confirmation is that review's own (A2.5, B9.3), which names the
review's identity, so there is one Restore path. The page shows the engine's reason
on each stage page it blocks and enables each stage's action from the engine's stage
answer (B9.4). It keeps no lock of its own on the controls that edit what the stages
read: they stay usable, and the page sends each change and each Reset for the engine
to refuse with the reason and the Restore action (annex E11.7). The answer that ends an Apply or a Restore carries
the stages as they stand then, so a confirmed Restore ends the block without a
reopen. Cancelling the approval of the Restore review
(B9.3) sends nothing and decides nothing: the block stays, and Restore previous
settings stays in the position.

**B6.5 Keep in project** is the separate, explicit action after Apply: the
values the loop's parameter record holds in the running PLC become the record's
initial values in the project, so a restart or a new download keeps them. It is
never a side effect of Apply or Restore and is not offered after a Restore.

- A record written by hand is edited in its own declaration: the kept fields of
  its initializer take the running values, written in the same form (`LREAL#…`
  stays typed); other fields stay as written; a missing field is added; a
  declaration without an initializer gets one. The engine edits the saved file.
  The panel passes the files that have unsaved changes in an editor; when the
  edit would change one of them, Keep is refused and asks to save or discard
  those changes first, so the user's own changes are neither overwritten nor
  saved for them. A file is written only while it still holds the text the edit
  was prepared from.
- A record that Create PLC code declared belongs to the equipment design: it is
  kept through the HMI Builder's equipment edit (Specification 39 §21.4 item 6),
  which writes `plant.toml` and the generated declaration together, under the
  same rule for unsaved changes. Values the equipment has no setting for are
  named as not kept.
- A record inside a function block instance is not edited, because one
  declaration serves every instance; Control Studio says to copy the values by
  hand.

The answer names the fields kept; a refusal keeps nothing and says why. Its review
carries the engine's sentence of what the Keep writes (`description`, A11.1): "Keep in
project writes these running values as the initial values of <record> in the project,
so a restart or a new download keeps them.", which the page shows as the review's
heading, as it shows the description of an Apply or a Restore review. The review
is confirmed with its own button on any target (`confirmation`: `explicit`), because
it edits the project's files and never writes the controller, and its result states
the offers that follow (`restoreAvailable`, `keepAvailable` and `primaryAction`, B7).

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
| Restore not confirmed | Restore again; dependent actions stay blocked until confirmed (the engine's block, B6.4) |
| Restore unavailable: the active record or the target changed since Apply | None; validate and apply again |
| The runtime restarted or reloaded its program since Apply | None; the checkpoint is kept as history; validate again |
| The runtime changed since the review (apply, restore or keep) | None; the review is asked for again |
| Keep: a file the edit changes has unsaved changes in an editor | Save or discard them, then Keep in project again |
| Keep: the project changed since the review | Keep in project again |

### B7. After Apply

After Apply the Validate page keeps showing the loop live, with **Restore
previous settings** in the Apply position. While an Apply's settings stand (its
checkpoint completed and not restored or retired), and after they were restored or
retired, Apply is not offered for the validation that was applied, also after Keep in
project; a candidate that passes validation after the Apply, or after a Restore, may be
applied, and then holds the position (the Validate page specification). A standing Apply's checkpoint is not an unfinished operation: it does
not hold back Identify, Tune or Validate. This version does not detect drift
automatically; a new identification is the engineer's decision.

The engine states these offers, and the page shows them and keeps no rule of them.
One rule decides `applyAvailable` in every answer that states it, `controlStudio/open`,
`controlStudio/inputs` and `controlStudio/validate` alike: the study's validation of the current candidate
passed (B5.4), no Apply's checkpoint is unresolved (B6.4), and the newest Apply applied
another validation, whether its settings stand, were restored or were retired. The newest
Apply is the one whose checkpoint the loop's journal holds, at any stage, and otherwise the
one of the study's newest Apply receipt (A8.1, A8.5). So a reopened loop does not offer the
validation that was applied, also after its Restore, and a validation that passed after the
Apply or after its Restore is offered: every answer after the end of an Apply or a Restore
states the Apply offer that end stated, until a new validation passes. Apply's review and its
confirmation keep the same rule: a request for the validation the newest Apply applied is
refused with the next action Validate, as the offer is withheld.
`controlStudio/open` also answers `restoreAvailable` and `keepAvailable` (B6.4). The
end of an Apply and the end of a Restore answer all three: neither end offers Apply
(`applyAvailable` is false), because the Apply's settings stand or it restored the
previous settings at once, and a candidate must pass validation after the application
before Apply is offered for it. The end of a Keep answers `restoreAvailable` and
`keepAvailable` from the study's receipts. Every answer that states the offers also
names the action that holds the position (`primaryAction`): `apply` while Apply is
offered; else `keep` while Keep in project is offered, which makes Keep in project the
page's primary action while Restore previous settings keeps the position beside it;
else `restore` while Restore previous settings is offered; absent when nothing is
offered. The page puts the action the engine names in the position, enables Restore
previous settings and Keep in project while the engine offers them, and enables Apply
from the Apply stage's `available` (B9.4) of the answer it received last: in every
answer that states the offers the Apply stage can start exactly when `applyAvailable`
is true, and every answer with stages renews it, so an answer that leaves no validation
current (A8.4) holds Apply back. The page combines no two answers. A refused Apply, Restore or Keep changes none of these offers:
the page shows the engine's refusal with its next action (B9.3), and the engine decides
the next press.

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
  achieved speed is shown. The runtime words the speed in its status report
  (`simulation_speed_text` and `simulation_speed_behind`, annex E1.3), by one rule
  of its own. The engine names the target from those words in its
  `controlStudio/monitor` answer (§0.1a), and the page and the other extension
  surfaces (the status bar, the home view, the Devices & Connections canvas) show
  the words as the runtime gives them. The runtime's HMI display shows the configured
  and the achieved speed the runtime decides, and translates the phrase
  "{achieved} of {configured}" for its page language.
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
  confirms it (A2.5). The review's `confirmation` says which of the two applies,
  and the engine fills the block's fields in its review. A
  signal's limits are the range its parameter declares; a signal without a
  declared range is shown as not declared. The stop conditions are those of
  B6.3 and B6.7: a measured output that leaves its permitted range, a failed
  requirement, a Cancel or a step the runtime refuses after the first write (for
  Apply); a step the runtime does not confirm (for Restore). A Restore records
  nothing, so its block gives no duration. Cancel leaves the approval without
  sending anything.
- **Progress.** A long operation MUST show its elapsed and remaining process
  time (A3.6). When the observed time scale differs from 1 by more than 5 %, it
  MUST also show that time scale and the expected finish in wall time. The
  progress display stays inside the action area.

#### B9.4 Navigation and operation state

- Navigation MUST NOT start work, overwrite a deliberate page choice or discard
  a result (B8).
- While work runs, its button becomes **Abort** (B8). The Abort control MUST
  stay reachable on every page, and every page MUST target the same operation
  identity. A refused change of the study inputs (`controlStudio/inputs`) is no
  operation's end: the page shows the engine's sentence on the editor's page and
  the running work keeps its **Abort**.
- Changing a view MUST NOT duplicate an acquisition or a parameter write.
- A stage the loop's structure does not use shows the status Not applicable and the engine's reason on its page, and the page's
  controls for that stage are inert (C12). The engine decides the status; the page only shows it.
- Each stage's answer also says whether the stage's own action can start now (`available`): Identify while the loop is bound, Tune
  with a model, a declared analysis or manual controller parameters, Validate with a candidate, and Apply with a candidate whose
  current validation passed (B5.4) and exactly when Apply is offered (B7): an answer that states the offers, or whose study may keep
  a validation current (the run, the Recorded-data fit and a project test), reads the loop's journal for the Apply stage, and an
  answer that leaves no validation current (an identification, a Tune, a reset) offers no Apply. The page enables a stage's action
  exactly then, together with its own pending-request states
  (a Reset in progress, a simulator start in progress, an incomplete plant mapping), and holds no copy of the rule. A change that
  invalidates only the validation (A8.4) leaves the candidate, so it leaves Validate available. A stage that cannot start for want of what
  it works on names the reason (`reason`): "Identify a plant model first." for Tune, which designs on a model, a declared analysis or manual
  parameters, and "Tune a controller first." for Validate, which runs a candidate; the page shows it at the top of the stage's page like any
  other reason, writes none in its template, and shows none once the stage can start. While an Apply's checkpoint waits for
  its Restore, the stages that write or change the study cannot start, and each names the engine's reason, which the page shows at the
  top of the stage's page without making it inert, so that Restore previous settings stays in reach (B6.4).

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
  The engine decides which value is drawn: a sample of a kept recording carries `plotted`, the value each plotted signal takes at the
  sample by the signal's identifier (`liveSides.plotSignals`, §0.1a), with null where the plot draws a gap, and a signal of the live
  sample carries `plotted` as a flag. A signal's value is drawn when it is there, finite, not forced and given good quality by the
  runtime (E4.4), and an additional signal's also when its channel's quality signals do not mark it bad; no other signal's quality
  enters. The page draws what the engine marks.

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
  in parentheses. Several channels of one kind are numbered in the engine's channel
  order (Measurement 1, Measurement 2); the page takes the channels and the identifiers
  of the auxiliary signals from the open answer (`liveSides`, B3.6, B9.6) and resolves
  no signal of the loop. The panel title names the loop. The full symbol appears on hover.
  Signal selectors show the symbol with its unit.
- **Durations.** A duration field accepts "90 s", "15 min", "2 h", "1 h 30 min" or a
  plain number of seconds. A duration is shown in the largest fitting unit: seconds
  below 2 min, minutes below 2 h, then hours and minutes. A duration that the engine answers as text is written by the engine in that form (the
  analysis tools' step and duration, and the maximum duration of Limits, `2 min 30 s`), reads back through the engine's reader as the same
  number of seconds (`duration_words`), and is shown as written; the page formats none of them and sends what the engineer typed, so a
  duration is never rounded by the way it is shown: 89.999 s is shown as `89.999 s`, and an entry typed as `89.999` goes back and is shown
  as `89.999`. The compact editor of the requirements shows a duration of the record, which the engine answers as a number of seconds,
  in the same form; the number goes back while the text is unchanged.
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
  small Keep toggle beside it. On a served loop the form asks for the symbols the engine names for the chosen family (`familySettings`
  of `setupDiscover`, C2); a loop the engine names none for keeps the page's reading of its law.
- **Analysis tools.** The tuning method list holds only tuning methods. Bode,
  Nyquist, root locus, pole-zero map, Routh–Hurwitz, lead/lag design and the
  frequency and multichannel tools are behind one **Analyze…** entry on Tune, shown
  once a model exists. A chosen tool shows its own few inputs in the result area
  and its plot in the plot area; closing it returns to the tuning result.
  The tools are those `controlStudio/analyze` answers (§0.1a): Bode (`frequency`);
  Nyquist and sensitivity, the two views of one `feedback` result, so switching
  between them needs no calculation; root locus (`rootLocus`); pole-zero map
  (`poleZero`); Routh–Hurwitz (`routh`); lead/lag design (`leadLag`); the measured
  frequency response (`measuredFrequency`); the initial-state response
  (`initialState`); the state structure (`stateStructure`); and the relative gain
  array (`relativeGain`). The page shows the engine's labels, default inputs, input
  refusals and results; it checks no input and computes no value. A stage's action is enabled when the engine's status for its tab is `ready` or `complete`, and the Identify button names the method the page shows (the project test's words, the recorded-data fit's, or Identify plant). Every field is sent as the
  text the engineer typed, and the engine reads it (B9.9): a field that
  is blank is absent, so that the engine's default applies, except that a frequency grid is
  read whole and refuses a blank entry; a field that is no number, whole number or list of
  numbers is refused under its name, with the sentence of what it takes. The inputs the page
  records for a tool (A8.1) are read in the same way. The editor then
  shows the inputs the engine read. When an Identify answer shows, clears or accepts a model, the page asks the engine which tools apply to the model
  now, and sends the answers it holds with the revision of the model each was calculated for; the engine lists the tools whose answers are no
  longer current (`stale`, §0.1a) and the page drops those answers and no others. The answer to a change of the inputs lists, as `stale`, the
  recorded analysis whose model is no longer the study's (§0.1a), and the page drops it and asks nothing. The page keeps no rule of its own for
  when an answer is out of date. The
  initial-state response shows the model's output over a duration and a step, from
  its initial state under a held input, on an open loop or through the loop's
  feedback (N4.0); its defaults are a step of 0.1 s, a duration of 10 s, a zero
  initial state, a zero held input and an open loop. The engine reads the Step and
  Duration as typed (`15 min`, `90 s`, `1 h 30 min`, or a number of seconds), and it
  refuses a text it cannot read with its sentence and the field it names (`response-step`
  or `response-duration`). The engine writes a duration below 1 ms in plain decimal seconds, never with an exponent, which its reader refuses. Reopening shows the tool last
  shown with its inputs (`controlStudio/page`). The measured MIMO path is one of these
  tools on a loop the engine serves (C7, **Measured paths** on the Tune page); a loop the engine refuses shows its refusal (§0.1a). Every Bode view, the feedback result's
  included, draws magnitude and phase, the phase on a second axis that spans whole
  45° steps around its data with a tick every 45°; a phase in a table or a tooltip is
  shown to 0.01°, so a value that rounds to zero reads 0.
- **Requirements.** The Requirements editor shows maximum settling time, maximum
  overshoot and maximum steady-state error, each with the band it needs, and an
  optional input disturbance. A collapsed **More…** holds the test design: the test
  setpoint change, the validation duration, the Auto-entry tolerance, measurement
  noise and scenarios, filled automatically and marked Automatic. The Auto-entry tolerance may not exceed the loop's output span (its output maximum less its output minimum, B4.3); a larger value is refused under its field with `Use <span> or less.` The signal column
  appears only for loops with several outputs. From Identify on, the editor holds the
  requirements Tune would design to for the identified model, with Automatic's values
  marked Automatic; a reopened study holds the requirements it recorded.
- **Validate.** Apply, or Restore previous settings, sits directly under the results
  table. Pass, Fail and Inconclusive are marked with an icon and colour as well as
  the word. Failed rows come first, then inconclusive rows, then passing rows, each
  group in the engine's order (the model agreement row stays first unless it fails),
  and the candidate's overall row is last. The passing guards, the checks the engine
  generated from the loop's declaration (operating ranges, state ranges, output
  rates, fixed parameters and the controller transfer, B5.3), are counted in one row
  after the passing rows, named "Ranges, states and transfer", which reads "N checks",
  "Within declared limits" and Pass. The rows of the Manual-to-Auto entry carry the case's
  words, "Manual-to-Auto entry", before their own names, as a scenario's rows carry their run's words.

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
- The engine keeps the experiment and the plant in the recording file of the run
  that made the recording, under `context`, when the run happens. A file whose run
  states no context writes no such field, so the identifiers of existing files
  stay as they are.
  - `context.experiments` lists the experiments the run wrote, one for each plant
    input the campaign stepped, in the order it stepped them. The engine notes an
    experiment when its hold ends, whatever it ended with (`completed`, `aborted`,
    `failed` or `failed_to_restore`, annex E5.7), from the plan the engineer
    reviewed and the runtime's last status of the experiment (annex E5.6). An
    experiment that the run aborts after a failure, and one that the engineer's
    Cancel of a journaled campaign or Test aborts, is noted as it ended in the
    same way. Each keeps `kind`, `affectedSignal`, `address`, `requestedValue` (a Boolean is 0 or
    1), `appliedValue` (the value the runtime read back after its last write,
    `last_applied`; absent when the runtime reports none), `durationMs`,
    `intervalMs`, `seed`, `frequenciesHz`, the commanded `trajectory`,
    `stopConditions`, the bounds `minimum` and `maximum`, `approval` ("Simulation
    confirmation" on a simulated target, "Explicit physical approval after
    topology review" on a physical one), `status`, `stopReason`, `aborted`, and
    `restoration` with its `value`, its `status` (`confirmed`, `failed` or
    `not_required`) and its `reason`. An experiment that the engine does not follow
    to its end, because the runtime stops answering, has no terminal status and so
    no context; its recording is kept as before.
  - `context.plant` is kept for a recording of a simulated target whose loop names
    a plant (`plantId`), and only when the runtime proves which configuration it
    runs: its `plant.status` (specification 34, §20.4) reports the plant active
    under the revision of the saved configuration that its `plant.read` (§20.2)
    returns. It keeps `id`, `configurationRevision`, `configuration` (the plant's
    entry of the saved simulation configuration), `model`, `effectiveSeed` (the
    runtime's effective seed as decimal text, never the word `random`,
    specification 33 §3.1), `uncertainties` and `scheduledEvents` (one canonical
    JSON text each; the events are those of the saved configuration whose `plant`
    is this plant), `actuator` (the plant's actuator entry as canonical JSON, or
    "Not declared") and `validityRange` (`PV 0…100 °C; MV 0…100 %`: the ranges the
    loop declares for its measured output and its plant input). A physical target
    has no simulation configuration, so a run on one states no plant; a runtime
    that does not prove its configuration states none, and the run is not refused
    for it. The Cancel of a journaled campaign or Test states the plant of the
    recording it keeps only together with the experiment it ended, which shows
    that the runtime still held the test.
  - A saved result that the engine opens as one recording (B3.6) keeps the
    experiment and the plant it states, those of the recording's own provenance
    when it has one, in the same fields. A part that the engine does not read is
    left out and never makes the file unreadable.
  - A recording file whose context breaks a rule is rejected as a whole when it is
    read (B10.4): a number that is not finite, bounds that are not ordered, a
    blank text, an abort state that contradicts the status, or a simulated plant
    in a physical recording (A9). A part that breaks a rule when a run keeps its
    recording is left out, and the recording is kept.
- Every observed sample and measured metric keeps the evidence source of its
  result. Simulated plant and Isolated simulation evidence MUST NOT be loaded,
  copied, compared or exported as Physical plant evidence. Model prediction MUST
  stay distinguishable from observed evidence after every save and reopen (A9).
- An opened recording keeps its original source and carries the opened marker
  (A9).
- A state-space identification of several plant inputs or measured outputs (C7,
  C8) is kept with its model and its signals and without a comparison
  presentation, and reopening reads it back as the study's current model. Its
  Identify answer gives the state model's words and no plot, and names the
  recording it was fitted from (`recordingId`), as every fit's answer does.
- A run with no acquired samples records `evidence.observation: "Not observed"`.
- A declared analysis is stored under `declaredAnalysis`. It keeps the exact
  definition, the calculated result, the selected parameter value, the fixed
  parameters and the model provenance. Reopening restores the result and the
  selection.
- A Routh–Hurwitz result is stored under `routh`: its definition, its selected
  value and its calculated result. A general feedback result is stored under
  `generalFeedback`: its definition (the plant and the controller, their channels,
  and the declared law when the controller is that law), its calculated result, and
  its margin and Nyquist evidence with their scope sentences. A measured frequency
  response is stored under `periodicFrequency`: the identifier and revision of its
  recording, the sample time, the startup samples, the segment count, the minimum
  coherence, the frequencies and the calculated points. Each section names the loop
  revision and the model revision it was calculated from; a section whose revisions
  are no longer current is not restored, and a section that refers to a recording
  the study does not list is refused. These sections are additive: the study format
  stays 1.
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
  kept: the stop reason, the abort state and the restoration result in the
  recording file's `context` (B10.2), the quality in its verdict, and the JSON
  export writes them (B10.5). Complete acquisition does not establish experiment
  qualification or a passed controller validation.
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
- A PNG export is presentation only. It is not a data record. The page renders the chart and sends its data URL; `controlStudio/chartExport` (§0.1a) checks the message against the panel's session, decodes the data, and names the file with its save filter. The file is at most 16 MiB, and its name is a plain name with the type's extension (`.png`, `.csv` or `.json`), or `control-studio-export` with that extension when the page gives none. The extension shows the save dialog with the engine's name and filter, and writes the bytes the engine produced.
- Exports MUST NOT contain credentials or runtime secrets.
- The engine writes both exports, as `controlStudio/export` (§0.1a) answers them, and the extension writes the text where the engineer
  chooses. The JSON export is the `controlStudio/open` answer of the loop without any key that names a secret, and under `recordings` the
  run context of each recording the study keeps whose run states one (B10.2): the recording's `recordingId`, `source`, `opened` marker
  and `evidence` verdict, its `experiments` with their stop reason, abort state and restoration, and its `plant`. A study whose runs
  state no context writes no `recordings`. The CSV export has one row
  per recorded sample of the selected recording, with the columns above in that order, then five columns for each additional channel
  (`value`, `unit`, `quality`, `channel`, `role`). Its numbers are written with the digits that read back as the same value, and a missing
  value is an empty cell (B10.3). The engine's recordings keep no safety channel and no event markers, so the safety state reads
  `Unavailable` and the event markers are empty; the units, the output limits and the rate limits are the loop's declared ones.

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
- The record may take its values from fresh task-boundary sources: `model-input`, `model-output`, `model-input-is`, `pid-integral`, `weighted-error`, `notch-history`, `scaled-model-input` (C8) and `decoupled-output` (C7), the kinds of protocol annex E6.4.
- An actuator input uses the bound applied feedback where it exists.
- With several actuators, the coupled transfer binds each actuator and its typed Manual command separately from the controlled references. The reviewed output-change tolerance of the primary output applies to each additional actuator as the same fraction of that actuator's declared span. Later target changes use the full reviewed spans. Every actuator command and state requires a compiled binding.
- Recovery restores the original reference vector and the parameter record.

#### Structure bounds

Each bound that a structure's setup applies to a gain, a time, a state or a count is named in numerical annex N10, with its value and unit. A structure cites the bounds it uses and takes their values from N10.

#### Families and zero terms

| Family | Mappings the adapter needs | Terms written as explicit zero |
|---|---|---|
| P | proportional gain | integral gain and derivative gain |
| PI | proportional and integral gains | derivative gain |
| PD | proportional and derivative gains | integral gain |
| PID | proportional, integral and derivative gains | none |

- The adapter declares a family only from its actual bindings. A P, PI, PD or PID choice is offered only when its mappings exist.
- The cascade's two controllers (C6), the PI on the output path of a measured-disturbance feedforward (C8) and the PI of a Smith predictor (C9) are PI. Those structures offer no other family.
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

**Back-calculation anti-windup.** A PID law MAY declare `antiWindup: { method: "back-calculation", trackingGainSymbol: "..." }`. The symbol names one unique bound `REAL` or `LREAL` parameter in reciprocal seconds. Its value is the tracking gain `1/Tt`. Zero is the untracked baseline. A candidate uses a finite positive `Tt` and writes `1/Tt` through the same complete record: a pinned tracking gain gives `Tt = 1/gain`; otherwise `Tt = max(Δt, √(Ti·Td))` when the candidate has derivative action and `Tt = max(Ti, Δt)` when it does not, with `Ti = |Kp/Ki|` and `Td = |Kd/Kp|`. The law's declaration gives the value its tracking meaning, not the parameter's name.

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
- An integrating model with a lag, `K′·exp(−L·s) / (s·(τ·s + 1))`, is a second-order model with no constant term. SIMC and AMIGO refuse it (N5.7), and Automatic places its poles as for any delay-free second-order model. Identify fits no such form; a supplied model of that form (B3.7) is tuned this way.

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

- the complete `Control.PID.ParametersValue` record, as the adapter maps it (C0), including β, Tf and the tracking gain where declared. The shipped `Control.PIDParameters` has no filter field, so a `Control.PID` loop declares no Tf; its `TrackingTime` is `max(Ti, Δt)` unless pinned;
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
- When identification returns the loop to Auto, the references return to their values from before identification (B3.4). A program that sets its reference from the measurement in Manual overwrites a reference written back there at its next scan, so the loop enters Auto at the measurement; a reference the loop then no longer holds returns in Auto, as a target change with the allowance of Apply's target step. A manual output that follows the output is not written back. Auto overwrites it, and writing it back would move the output.

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
- The engine serves a `controller.project` block whose seed is `parallel-pid` (or `trajectory-feedforward-pid`, C11; `feedforward-pid`, C8; `cascade-pid`, C6), whose family is P, PI, PD or PID, and which declares no `axes`, `components` or `modelParameters` (those are C5). Its families are P always, and PI, PD and PID where the seed names the integral and derivative variables. The page lists these families from the engine's `setupDiscover` answer, and takes its list from the TypeScript adapter only for a loop the engine does not serve. The page asks for the settings that `setupDiscover` names for each family (`familySettings`): for P its proportional variable, for PI also its integral variable, for PD its derivative variable, and for PID both, and Tune refuses a pin or an entered setting of any other variable. The other served forms are their structures' own: `cascade-pid` (C6), `smith-pid` (C9), `decentralized-pid` and `decoupled-pid` with their `controller.group` (C7), `feedforward-pid` (C8), `trajectory-feedforward-pid` (C11), a `parallel-pid` point of the `gain-scheduled` family (C10), and a block without a seed that declares tuning axes, the finite search (C5). Any other program's own law is parked by its name and refused with the cause `controller-capability`: a block without a seed or axes, a seed whose family its structure does not take, or a block with components or model parameters. A loop with a program's own law is never a C12 loop. A loop declares one controller structure: a controller law, the program's own law (with its PID group, C7), state feedback, or an ON/OFF controller; a loop that declares two is refused, because the engine cannot tell which one tunes it.
- A law with reset or derivative times, a derivative filter or further retained states uses the step-by-step setup.

#### Identify

As C1. The startup flag is set as for a PID; Identify chooses the step and its hold (B3.3, B3.4).

#### Tune

- Tune designs the gains from the model as for `Control.PID` (C1).
- It offers only the families whose gains the program has. Its answer is the program's candidate, `projectCandidate`: its structure, its gains, and the values of the variables of that structure, which `settings` keys by symbol; Apply still writes an explicit zero to each other variable that the seed names (C2 Apply). A Tune of a served loop is the engine's: the route sends the page's words as they are (the method, the family its selector shows, the pins and the settings), the engine reads a family word that names no PID structure as none and refuses what it cannot run, and the route refuses nothing itself; it never hands the Tune to the TypeScript session.
- The margins of C0 apply to the law's linear form. The result is checked on the actual ST (Validate).

#### Validate

- Validate runs the program's actual ST. Its location follows B5.3.
- Validation starts the integral term at the output that the loop has when it switches to Auto, with the setpoint at the measurement. The switch is therefore bumpless.
- The previous value starts at the error of that moment (zero) or at the measurement. The derivative therefore starts at zero.

#### Apply and Restore

- Apply writes the gain variables through the parameter binding. It starts the integral term and the previous-value variable as Validate does, and switches the loop to Auto, in one transaction (B6.1).
- The Auto-entry allowance is that of A6.4, with the law's own gains.
- Restore previous settings returns the gains and the mode from before Apply (B6.4).
- The route follows the variables that the seed names, not the structure Tune chose. A seed that names an integral or a derivative variable starts its declared states for every structure it offers, and writes an explicit zero to each term that the structure does not use (C0).
- A seed that names neither variable is a P law. It enters Auto through C13's tracking reference `r = y + u/Kp`, with its proportional variable as the gain, and its allowance is C13's. Such a law declares no law state: its application states (C1, such as a startup-done flag) are written with the entry at the same boundary, and a law state that a P law declares is refused by its name.
- Keep in project writes the running values of the gain variables into the program's declarations (B6.5): each becomes the initial value of its REAL or LREAL variable, written as `: <type> := <value>;`, and the other declarations stay as written.

### C3. Switching (ON/OFF) controller

The output is binary and switches between two levels, with hysteresis. The controller is project-owned (`design: project`). It has no PID adapter, so the generic PID Apply is not offered.

#### Setup

- The setup asks for these items:
  - the program instance that runs the controller;
  - its measurement and setpoint;
  - its numeric output, and the low and high levels it switches between;
  - its Auto/Manual signal and its manual output;
  - the three settings that Tune designs (below).
- **Hysteresis.** The hysteresis is in the measurement's unit. The convention is declared: half-band, where the value is the half-width of the band around the setpoint, or full-band, where it is the full width.
- **Minimum times.** The minimum ON time and the minimum OFF time are in seconds.
- **Output levels.** Setup asks for the low and high levels. They default to the output's limits, and the levels in use are shown.
- **Equipment limits.** Setup asks for the switched equipment's own minimum ON and OFF times, for example a restart protection. Each is a time in seconds, or zero when the engineer confirms that the equipment has no such limit; a negative time is refused under its field. A blank answer is never taken as zero, and Tune refuses until both are declared. The values are declared as the output channel's actuator dwell limits, and Tune never designs shorter minimum times.
- **Retained states.** The output latch starts ON when the output starts at its high level. A dwell timer, in seconds, is optional. It starts at zero.
- A switching controller whose output is `BOOL`, or that has other retained states, uses the step-by-step setup.

#### Identify

- Identification uses the actuator's full binary levels. Its timing changes between independent records, so that validation has data that identification did not use (B5.2).
- The design inputs of Tune are the measurement's noise and the settling band of the process requirement. Tune does not use a fitted model for them.

#### Tune

- Tune selects the threshold and the hysteresis from the measurement's noise and the settling band of the process requirement. It selects the minimum ON and OFF times from the process requirements. It never designs them shorter than the equipment's dwell limits.
- Tune proposes these values only from the declared actuator ratings, the process bounds and the measurement uncertainty. Setup asks for the equipment limits. Tune refuses while one of them is undeclared, and it never guesses one.
- The requested state changes when the measurement crosses a threshold on the side that the declared action requires. Inside the hysteresis band the requested state keeps its previous value. The threshold-equality policy is the declared field `thresholdEquality` of the `onOff` block, with two values: `at-or-beyond`, the default, in which the requested state changes at or beyond the threshold, and `beyond`, in which it changes only strictly beyond it (D-10; N10).
- The applied state changes only when the dwell condition holds: the time since the last change is at least the dwell.
- A minimum time that Tune designs is rounded up to a whole number of task periods, and the rounded value is never below the equipment's limit (N10). The hysteresis is the declared width of the band, which for `full-band` is twice the half-width. A pinned switching setting is one of its three settings, given by symbol in the study's `onOffPins` (B4.2). Tune designs the other settings, applies each pin after the design, and refuses a pin that is not one of the three settings or that its binding does not hold, naming the symbol. A pinned minimum ON or OFF time below the switched equipment's declared minimum of the same kind is refused, with its symbol as its field, since a dwell is never shorter than the equipment's limit. An accepted pinned minimum time is rounded up to a whole number of task periods, as a designed one is. Undeclared equipment minimums refuse Tune with or without pins (D-9).
- The candidate is generated in the controller's own conventions, and it is then run on the actual ST (Validate).
- No margins are shown for this law (C0).

#### Validate

- Validate runs the actual ST on the selected model. Each run starts the plant at a declared level, even when the equilibrium input lies between the two output levels.
- The scenarios are the events of B5.3 and a state-recovery scenario that perturbs the initial plant state once. A switching cycle is also run: the reference starts at its high level and steps to its low and high levels in three events, each a quarter of the run, so that the output crosses both thresholds. The run is four quarters long, each at least twice the longer minimum time. Its settling and error checks are not judged, and it refuses a reference range too narrow for both thresholds.
- The target case of a switching controller judges its levels but not its dwell times; the switching cycle judges the dwell times.
- Each run records every execution, so that the levels and dwell times are checked at every scan (A4.2). Where the sample limits make a run average its scans, the transition rule, the retained band and the aborts of that run are inconclusive, since they need each execution.
- The five criteria are judged as follows. Every recorded change of the requested state happens at its threshold, on the side the action requires, under the threshold policy: the transition rule. The requested state keeps inside its band at every scan after the first: a request that is ON does not sit at or beyond the OFF threshold, and an OFF request does not sit at or beyond the ON threshold: the retained band. Each change of the applied output follows the requested state; a change that the request does not hold is an abort, recorded separately, which fails the run and never counts as a normal dwell completion, so the dwell checks of an interval that an abort ends are inconclusive. The levels and the completed dwells are the requirements of the plan, and the requested state, the setpoint, the measurement and the applied output are read at every scan; a scan without one of them is inconclusive (B5.4).
- The criteria are these:
  - every recorded transition obeys the switching rule;
  - every completed normal interval is at least the dwell;
  - the applied command is binary;
  - the retained request stays inside its band;
  - an abort is recorded separately and never counts as a normal dwell completion.
- Output levels and completed dwell intervals are assessed by the shared requirement evaluator. A missing complete interval is Inconclusive (B5.4).

#### Apply and Restore

- The commissioning response of an Apply is judged with the switching limits: the two output levels and the reviewed minimum times, which the response's switching check uses (`criteria.switching`). Its expected settling time is none, since the output switches.
- Apply writes the hysteresis and the minimum times, and switches to Auto, in one task-boundary transaction (B6.1). The latch and the dwell timer are initialized as declared (C0).
- The Auto-entry allowance differs from A6.4. A binary output has no band. At the entry, the latch starts at the output's applied level and the dwell timer starts at zero. The output therefore keeps its level until the dwell has elapsed. The allowance is zero: a binary output has no band, so the first output is the applied level exactly.
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
- The setup holds at most three further measured outputs and, for LQG, at most three estimator states (D-B4). The design limit of 32 states is not reached through the setup.
- For each measured output, the setup asks for the variable that holds its gain, in the plant input's unit per that output's unit. For the controlled output it also asks for the variable that holds the reference gain `N`.
- The program applies `u = u₀ + N·(r − y₀) − Σᵢ Kᵢ·(yᵢ − yᵢ₀)` around the operating point at which the plant is identified.
- The Auto/Manual signal, the manual output and the startup flag are set as for a PID (C1); Identify chooses the step and its hold (B3.3, B3.4).
  The state-feedback record has no application states, so its startup flag is one of the record's declared states, as the shipped
  lessons 68, 69 and 72 declare it (settled in wave 4).
- The loop declares `controller.stateFeedback` with `family` set to `state-feedback`, `LQR` or `LQG`. Its convention is `sampled-deviation-negative-feedback`, meaning `du = −K·dx + N·dr`.
- The ordered `gainSymbols` and `referenceGainSymbols` matrices name the compiled `REAL` or `LREAL` parameter fields. These fields belong to one parameter binding and one task.
- Coefficient units follow the declared input/state and input/controlled-output coordinates. Auxiliary model outputs remain measurements, not extra controlled objectives.

**LQG adds the following.**

- The program estimates the states from the controlled output with a fixed model, in deviations from the model's operating point:

  ```text
  x̂⁻[k] = A·x̂[k−1] + B·u[k−1]
  x̂[k] = x̂⁻[k] + L·(y[k] − C·x̂⁻[k])
  ```

  The estimates feed back through `K` and `N`. The stored estimator coefficient `L` is the current-estimate Kalman gain, not `A·L`: the estimator poles are those of `(I − L·C)·A`. The steady-state gain iterates the covariance from `P = Q` with `P⁻ = A·P·Aᵀ + Q`, `L = P⁻·Cᵀ·(C·P⁻·Cᵀ + R)⁻¹` and `P = (I − L·C)·P⁻`, and stops when the largest change of `P` is at most `1e-10 + 1e-9·max|P|`; the published gains are those of this stopping rule.
- The setup asks for the model file in the project that the estimator implements. It is a state-space model with one input, the controlled output as its one output, sampled at the task period.
- The setup lists the project's model files that meet this and have at most three states, in the project folder and in `models/`. The saved loop holds the sampled model of the file it names.
- For each model state, the setup asks for the estimate variable, its gain and its observer gain (`observerGainSymbols`). It also asks for the process-noise variance of each state and the measurement-noise variance. These variances set the observer's trade-off between trusting the model and trusting the measurement.
- The estimates start at zero, the operating point.
- The binding declares the estimator's fixed `estimatorModel` matrices A, B and C, and its sample period. These MUST equal the selected sampled model. A newly selected plant MUST NOT silently tune an estimator that still runs different model coefficients.
- In this adapter, LQG requires zero direct measurement feedthrough.

#### Identify

- The plant is identified in measured coordinates, first order in each measured output (`identificationPolicy.maximumOrder` 1). Each gain then belongs to one measured signal.
- For LQG, the selected model MUST be the estimator's model. Supply the model file on Identify, or identify a model and write its matrices into the program and the model file before Tune.

#### Tune

- **Pole placement.** Explicit pole targets are discrete poles. A continuous model is converted by the exact zero-order-hold conversion at the actual task period. The gains place the sampled poles. Without explicit targets, the desired sampled poles are `zᵢ = exp(−(4 + i/n)·Δt/Tr)` for `i = 0 … n−1`, with `n` model states, the task period `Δt` and the response time `Tr` the requirements ask for. Explicit targets are one real pole inside the unit circle per model state.
- **Design limits.** At most 32 model states and 16 plant inputs, each plant input controls one output objective, and a finite-horizon LQR covers at most 512 steps. A design outside them, or an uncontrollable or singular model, is refused with the reason and the next action Choose method. A malformed weight is refused with its reason and no next action. A coordinate whose operating range has no span has no automatic weight, and is refused with the next action Identify. A finite-horizon design whose repeated first gain does not stabilise the loop, and an estimator with a pole that is not strictly inside the unit circle, are refused with the next action Choose method.
- **LQR.** LQR weights may be supplied. Automatic weights normalize each coordinate by its declared operating span. The steady-state and the finite-horizon designs check the weights alike: the state weight and the terminal weight are symmetric and not negative, and the input weight is symmetric and positive. The gain is the steady-state gain of the discrete linear-quadratic regulator for the sampled model, from the algebraic Riccati equation.
- **LQG.** Tune designs `K`, `L` and `N` by LQG on the selected model. The design needs supplied process and measurement covariances. The estimator follows the prediction and correction convention above.
- If a required mode is not controllable, Tune reports the infeasibility instead of a candidate.
- The controlled objectives are the model outputs whose loop channel names a setpoint signal, in channel order; their number must equal the number of plant inputs. The response time `Tr` of the pole targets is the requirements' maximum settling time.
- Tune of a state-feedback law designs on the study's current model and keeps the candidate unapplied (B4.1). Its answer carries the closed-loop poles (N7.11), the stages of a finite horizon (the gains and the Riccati matrices, by step, under the horizon policy), and the fields of the design form for the family with their labels (the state-design choices of §0.1a). A reopened study answers a kept state law with the same answer as its run: the margins come from the kept gains and the study's model, and nothing is predicted.
- A pin keeps explicit coefficients: a pin of any mapped coefficient (`K`, `N` or `L`) replaces its designed value within the parameter's bounds, after the REAL rounding. The mapped sampled poles are recomputed, and any optimality claim is discarded: a finite-horizon design then has the horizon policy `pinned-fixed-gain`.
- **Margins at the plant input.** For one plant input, the loop is broken at that input:

  ```text
  L = K·(zI − A)⁻¹·B
  ```

  This is evaluated on the model sampled at the task period, with the gains `K` on the fed-back outputs, up to half the sampling frequency. The delay margin is then the extra dead time at the plant input that the loop tolerates. The margin definitions are in C0. Until the margins of a sampled model are given, the engine answers the margins of such a model as unavailable, with that reason.

#### Validate

- Validate runs the actual ST (B5.3). The operating range is the intersection of the selected model's operating ranges and the bound signal and actuator limits.
- The state-recovery scenario perturbs the initial plant state once: each model state moves by one per cent of its operating range, toward the side of the range the move stays inside, and the case runs on the selected model from that state, judged on the nominal requirements.
- The record enters the same actual-ST validation and Manual staging operations as any other record.

#### Apply and Restore

- Apply writes the gains `K` and `N`, and `L` for LQG, through the parameter bindings. It switches to Auto in one transaction (B6.1).
- Apply starts from the validated operating point. Every measured output lies within its settling band `bⱼ` of that point (B6.2). With several actuators, the coupled transfer of C0 applies.
- **Auto-entry allowance.** The allowance differs from A6.4. A state-feedback law has no retained state that makes the Auto entry bumpless. At entry each reference follows its measurement, so the first output moves by the law's response to the process's deviation from the operating point. Unless a reviewed tolerance is set, the first-output check allows, for plant input `i`:

  ```text
  Σⱼ |Kᵢⱼ|·bⱼ + Σₖ |Nᵢₖ|·bₖ
  ```

  Here `j` runs over the model states and `k` over the controlled outputs. A state is a measured output when its column of `C` selects exactly that output and that output's row selects no other state, and `bⱼ` is that output's settling band (its channel's, when the channel declares one). A reviewed tolerance replaces this allowance. A model with any other state keeps the exact check. With several plant inputs, the allowance is the largest input's sum, and it is never below the exact check.
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
  - the objective, which names the requirement the search minimises (`target-error`, minimised); its reference and feedback are
    the loop's setpoint and measurement, and its error unit is the measurement's unit (as lesson 89 declares it);
  - the startup flag, as for a PID (C1); Identify chooses the step and its hold (B3.3, B3.4).
- A pinned setting keeps its value. It is not part of the grid.
- The setup refuses a value list it cannot read, and a grid of more than 64 combinations (the product of the list lengths, Tune and A7).
- The coefficient and gain variables MUST be `REAL` or `LREAL` and unaddressed (C0). Initial values and events MUST NOT target a swept variable. A duplicate or ambiguous write is rejected. A `BOOL` value of 0 or 1 may start a project-owned sequence.

#### Identify

As C1. The search runs on the selected model.

#### Tune

- The grid is the Cartesian product of the value lists. The trial count MUST equal that product. It MUST NOT exceed 64 (A7).
- Each trial is one combination. It runs the project's actual ST against the selected model, in an isolated simulation with complete resets. Every trial uses the same initial values, the same plant initial state, the same events, time points and seed. A search never runs against the selected physical runtime.
- **Mechanism.** Each trial is one isolated simulation run, started through the runtime's simulation runs (`control.simulation.run.start`, `status` and `cancel`): the trial's values are the run's parameter overrides, and the selected model is its `simulation_override`. That is how Validate runs a candidate (B5.3). The runtime's search job (E8.5, `control.search.*`) runs against the project's own simulation, not the selected model, so C5 does not use it. Each run is journaled before its start (E11.2, A8.5), and the search runs as an operation that claims the loop, reports after each trial and stops at a cancel.
- Bounds are checked before ranking. A trial that fails a constraint does not qualify. Missing or bad evidence never qualifies. Only qualifying trials are ranked.
- **Selection.** Keep the combinations that meet every mandatory requirement. Choose the one with the smallest error after the settling time. The error is the trapezoidal sampled integral of squared error, in error-unit squared times seconds, with the actual step duration as the sample interval:

  ```text
  J = Σₖ ½·(eₖ² + eₖ₊₁²)·Δt
  ```

- Of equal errors, the first combination in the listed order is used.
- A `REAL` variable's values are rounded to the nearest `REAL` before the trial runs.
- The search stops on any change of the program revision. Every attempted trial is retained. A trial that is not attempted, after a cancellation or an exhausted budget, stays marked as not attempted. Nothing is truncated silently. A cancelled, changed or failed search keeps no candidate in the study; its answer carries every trial. A search that completed, or ran out of budget with a qualifying trial, keeps its candidate in the Tune section, with its trials that were not attempted marked.
- J's samples are the target-response run's samples from the settling deadline (the response deadline of the requirements, B4.3) to the end of the run. The reference is the loop's setpoint and the feedback its process variable.
- The trial and job budgets follow A7.

#### Validate

Validate follows B5.3 for the chosen candidate. The trials of the search are Tune evidence on the selected model.

#### Apply and Restore

- A search result is never applied implicitly. Selecting a trial populates the parameter review only when every declared tunable and fixed parameter is explicitly present in that trial's grid or initial conditions.
- Apply writes the chosen coefficient values through the parameter record (C0). It switches to Auto in one transaction. The retained values start as the setup names them, zero, the output or the measurement, as the validated trial started them.
- No first-output formula exists for this law, so Auto entry requires a reviewed tolerance (A6.4).
- Restore previous settings returns the previous coefficient values and the mode from before Apply (B6.4).

### C6. Cascade

A cascade has two controllers in series: the outer controller's output is the inner controller's setpoint. The structure is one loop with two `Control.PID` instances, and both controllers are PI (C0). The two-loop-file form, two loop files each with a `cascade` declaration, is not served.

#### Setup

- The Cascade structure of the setup writes one loop for two `Control.PID` instances. The inner controller drives the plant input and measures the inner measurement. The outer controller measures the outer measurement against the outer setpoint, and its output is the inner setpoint. Setup recognizes a cascade when the outer controller's output is the only source of the inner controller's setpoint (B1.3). Recognition counts sources, not names (D-3): a setpoint with a second source is no cascade, and the outer controller, by its instance, is the only source when the setpoint is fed by that instance alone.
- The setup draft has no manual output for the outer controller, because nothing in the structure uses one.
- A program that writes a seeded value of either controller, or initializes one, needs the startup-done flag, as a single loop does (C1). The seeded values of both controllers are checked, and the setup refuses the draft that names no flag.
- **Modes.** The structure binds the outer controller's mode as an `additional_modes` member of the transfer (see Apply and Restore).

#### Identify

- The inner controller is put in Manual, and its manual output is stepped. One model is fitted from the plant input to both measurements. It is first order in each measurement (`identificationPolicy.maximumOrder` 1), so the outer measurement follows the inner one as in the process.

#### Tune

- **Inner loop first.** Tune designs the inner PI on the path from the plant input to the inner measurement, for the inner requirements, with the family fixed to PI (C0).
- **Outer loop second.** Tune predicts, on the model, the closed inner loop's response of the outer measurement to a step in the inner setpoint. It reduces that response to first order with delay by the two-point method (C7). It then designs the outer PI, with the family fixed to PI (C0), for the outer requirements.
- Without declared inner requirements, the inner loop is designed to settle in one fifth of the outer settling time. The outer settling band is scaled to the inner measurement's range. The outer design's assumption that the inner measurement follows its setpoint then holds. Without them the inner duration is two fifths of the outer settling time; both fractions are bounds of numerical annex N10 (D-7).
- The requirements are those of the outer measurement, the loop's controlled output. Automatic is the only method: a named method, a structure other than PI, or a pinned setting is refused.
- The candidate is one unapplied record of both PIs: the proportional and integral gain variables of the inner and the outer controller, by symbol. The derivative variables are written as zero (C0). The answer predicts no outcome and shows no margins: C0 gives margins to a single linear law only.

#### Validate

- Validate runs the actual ST of both controllers in Auto, from the model's operating point, with the outer requirements. The run includes a load at the plant input that both loops reject. This validation always runs on the model. Each run starts both Auto signals in Auto: the inner controller's mode is the loop's mode, and the outer controller's is the `additionalModes` member of the program's block.

#### Apply and Restore

- One task-boundary transaction writes both parameter records and switches both controllers to Auto. The outer controller's mode is an `additional_modes` member of the transfer. `additional_modes` takes up to eight further controllers' modes. The modes are observed like `mode`, written with it at the same boundary (Auto when the transfer leaves Manual, Manual for a Manual transfer), and confirmed after the first scan.
- The inner controller starts at the present plant input, with its setpoint at the inner measurement. The outer controller starts with its output at the inner measurement. Neither switch bumps.
- Restore previous settings returns both parameter records and both controllers to Manual, in one transaction.

### C7. PID group (decentralized PI) and static decoupler

A PID group is two or more `Control.PID` instances whose processes interact. Each member's output moves several measured outputs. The group contract `controller.group` pairs each member's input channel with one output channel, and shares one Auto/Manual signal. The members therefore change mode together.

#### Setup

- The setup offers **PID group** when the program has two or more library PIDs. The engineer selects the members. For each member the engineer confirms the compiled measurement, output, setpoint, manual output and parameter record, as for a single loop.
- The group has one shared Auto/Manual signal. Each member's input channel may declare its own identification step (`experimentStep`); otherwise Identify chooses it (B3.3). The maximum duration comes from the limits. A member's own step is the identification step of its input. A step given for the whole group is the step of each member that declares none, and it alone sets the group's experiment amplitude: the first member's step never does. Without a group step, the group's experiment is the automatic request, an observation of amplitude 0 for the group's duration, and a duration may be given without a step. With no step, no duration and no member step, the group has no experiment (D-15, as revised).
- Setup writes one group loop. Its model channels follow the member order.
- The group's startup-done flag, when the draft names one, is a compiled BOOL that is none of the group's signals: the Auto/Manual signal, and each member's setpoint, measurement, manipulated variable and manual output. Setup refuses another with "X is not a compiled BOOL symbol." or "The startup-done flag is one of the loop's signals, so it cannot mark the startup as done." A loop declares at most 32 controller states per controller instance (numerics annex N10), and a group's members are its instances: a group of sixteen members declares at most 512 states.
- **Decoupler (when the program has one).** The setup asks for the decoupler's variables `D_ij`, for plant input `i` and member `j`. The unit of `D_ij` is plant input `i`'s unit per member `j`'s unit. In Auto the program writes `u = D·v`, where `v` are the members' `Control.PID` outputs. In Manual it writes `u = v`, so that identification and Manual operation act on the plant itself. Behind a decoupler, a member's plant input is the signal that the program writes from `D·v`, never the controller's own output: Setup refuses a member whose plant input is its own output, with the sentence "Behind a decoupler, <instance>'s output is the plant input the program writes from D·v, not the controller's own output <own>. Choose the plant input.", and names the member's plant-input field. Behind a decoupler a member's output is
  the plant input that the program writes from `D·v`, not its controller's own output: the setup refuses a member whose output is its own controller's output, and asks for the plant input.

#### Identify

- The group is identified in one campaign: one recording holds one experiment per plant input. An input that declares its own identification step (`experimentStep`) is stepped by a manual-output step of that size, checked against the identification policy as a declared `mv_step` is; an input without one takes the method Identify chooses for the loop (its declared method, or Automatic's choice in the loop's mode), so a group without a group step still steps each member at its own step, and when such an input would take a setpoint step in Auto beside a manual-output step, the campaign is refused before it records, since one recording holds one controller mode. The result is one multivariable model: the multichannel ARX fit of the recording, with orders up to the policy's maximum order and the policy's error limit on the independent record.
- With a decoupler, the identification is the same, in Manual.
- **Mode.** A method that needs the other controller mode (B3.4 step 1) moves the whole group at once, through the transfer of the group's own binding that Apply writes through (Apply and Restore, **Group**): it holds every member's output, and the setpoint and Manual command of every member are its states. Its tracking names, for every member but the loop's own, its setpoint with its own process value (`additional_references`) and its Manual command with the output it holds (`additional_commands`), and every member's process value (`additional_measurements`); every other state of the group is carried as it stands and bounded (A6.4), so that the runtime accepts the tracking (E6.4). A manual output that the program copies from the output (C1) is declared for the loop's own member only. When the campaign ends, fails or is cancelled, the group returns to the mode it was found in (A6.2). In Manual the setpoint and the Manual command of every member are written back, except the loop's own Manual command when the group returns to Auto and the program copies the output into it (C1). The allowance of that write-back is the larger of the mode change's own and the largest change that a restored Manual command makes to the output it holds, each change taken in the span of the loop's own output, as the runtime scales the tolerance of an additional output by the ratio of the output spans (E6.9). A group found in Auto then enters Auto, and one step in Auto returns every setpoint that the program overwrote in Manual to its found value, with the allowance of Apply's target step (C1).

#### Tune

1. **Steady-state gains and the relative gain array.** Calculate the steady-state gain matrix `K` (outputs × inputs) and the relative gain array `Λ = K ∘ (K⁻¹)ᵀ` for the members' pairing. `Λ` needs a square, non-singular `K`. A pairing whose diagonal relative gain is not positive, or whose `K` is singular, is refused. When every diagonal relative gain is positive, Tune names the members' pairing (D-12). When one is not, Tune refuses and names the pairing with positive diagonal relative gains closest to 1 (N4.5) as the pairing to declare, or says that no pairing has positive relative gains. A group of more than eight pairs names no pairing (N4.5). The members' wiring is the program's, so Tune never changes it.
2. **Two-point reduction.** For each member's own path `yᵢ/uᵢ`, let `t₂₈` and `t₆₃` be the times at which the step response reaches 28.3 % and 63.2 % of its final change. Then:

   ```text
   τ = 1.5·(t₆₃ − t₂₈)
   θ = t₆₃ − τ          (if θ < 0: θ = 0 and τ = t₆₃)
   ```

   The two-point reduction is also used by C6, C8 and C9.
3. **Effective gain and SIMC.** With the other loops closed, member `i` sees the effective steady-state gain `Kᵢᵢ / λᵢᵢ`. SIMC (C1) designs the member's PI for that gain, for the path's time constant and delay, and for the member's own requirements: its response is the one that C1's Automatic chooses from the SIMC responses alone, and a member that no SIMC response meets refuses the group with the shortfall, with no pole placement. That refusal names the member's requirements, so its next action is Requirements (`TaskInfeasible`, as C6's cascade); a refusal of the model, the pairing or the decoupler opens loop setup.

The result names, for each member, its path model, its `λᵢᵢ` and the resulting gains. Each member's response is the declared one, or More robust when the requirements declare none, which is Automatic's response (C1; D-13).

**Static decoupler.**

- Tune calculates `D = K⁻¹·diag(K)`, from the steady-state gains `K` of the group model. Through the decoupler, each member's output moves only its own measurement in steady state, with the gain of its own path. A singular `K` is refused.
- A two-member group whose program realizes the decoupler with the library's `StaticDecoupler2x2` (Specification 37 §4) is configured with the gain matrix `G = diag(K)⁻¹·K` and `ActuatorBias` zero. Its `ToActuators` then computes `u = G⁻¹·v = D·v`, and its `ToVirtual` computes `v = G·u = D⁻¹·u`, the members' start at Apply. A `G` that the block's singularity check refuses is refused by Tune with the block's reason. The check uses the relative-determinant tolerance of N10 (`BLOCK_SINGULARITY_TOLERANCE`), and the answer gives `G`, its determinant and its inverse, which equals `D` (D-14). Larger groups use the program's own `D_ij` variables.
- Each member's PI is designed as in step 3, on the model seen through the decoupler. Its relative gains are then one.
- The decoupler corrects steady-state interaction only. It does not cancel dynamic interaction.
- Tune shows `D` and the per-member design. The engine names each member's plant input and measured output, and the rows of `D`, by the symbol of the
  signal the loop binds to the channel, else by the channel's identifier (`inputName`, `outputName`, `inputNames` of the group's answer); the page
  resolves no signal.
- Centralized designs (C4) are separate structures.

#### Validate

- Validate runs the actual ST of all members against the full multivariable model. It uses every member's requirements, and each setpoint alone. This validation always runs on the model.
- With a decoupler, the decoupler is part of the run. Each member's retained output starts at its share of the operating inputs, `v = D⁻¹·u`, as Apply starts it (C0).
- Validate also runs one case for each controlled output, identified `independent-reference:<output channel>`. In that case the output's setpoint steps by its own reference change (its own requirements, or the loop's task for the process variable), and every other controlled setpoint holds at its operating value. Each held output keeps its settling and error checks against that held value, and every output keeps its bounds. Each case runs on the model with the candidate's ten values and starts the controller's states from the candidate's declared profile (C0). A group with one controlled output has no such case.

#### Apply and Restore

- **Group.** A PID group's transfer holds the loop's manipulated variable as its output and each other member's manipulated variable as an additional output in member order, each within its own input's actuator limits; the runtime holds at most sixteen outputs. Each other member's setpoint and Manual command are states of the transfer, and each member's process value is a guard. A PID group writes its values by symbol through its parameter record and takes no gains. Its declaration, its program, its shared mode and its parameter and state owners are checked when the adapter is built, and a group that fails them is refused before tuning or Apply; a group never falls through to a scalar PID adapter. Apply writes all members in one transaction with one rollback. Only the group changes. Other PIDs' parameters and retained states do not change. A group update is never presented as independent PID writes. The transfer's tracking names, for every member but the loop's own, its setpoint with its own process value (`additional_references`) and its Manual command with the output it holds (`additional_commands`), and every member's process value (`additional_measurements`), so that the Auto entry sets each setpoint to its own measurement and each Manual command to the output it holds (E6.4); the runtime accepts the tracking only when it names every state and every additional output once. Apply's target change moves every controlled setpoint to its target, the output's operating value plus the reference change that Validate holds the output to (its own record in the requirements, else its channel's, else, for the loop's process variable, the requirements themselves), and the review keeps every member's setpoint, which Restore writes back.
- **Decoupler.** Apply writes the PI gains and `D` in one transaction, and switches the group to Auto. Each member's PID starts at its share of the present plant inputs, `v = D⁻¹·u`, using the `decoupled-output` initialization of C0. The state's source is named `decoupled-output`; its value at the operating point is row `index` of `D⁻¹·u`, and each transfer reads its fresh value from the plant inputs of the group's channels in channel order, with the same matrix and row. The runtime's `decoupled-output` formula takes at most eight rows, and a decoupler's size is the group's member count, so a decoupled group has at most eight members. The product `u = D·v` therefore does not change at the switch.
- Restore previous settings returns all of them, and the group to Manual (B6.4).

**Measured paths.** A loop whose channels give as many inputs as outputs, two or more and at most eight, has its paths measured on one recording
of its study (`controlStudio/analyze`, `measuredPaths`, §0.1a). The request gives one settled trial for each input: the input's baseline
window, before its step, and its settled window, after it, both in seconds from the recording's first sample. Each window is finite,
ordered and inside the recording, and every sample in it is good for every channel (A4.2). The engine refuses an incomplete trial, one
with a window end left blank, and names the windows that are missing. For each trial:

- the mean of each input and each output over the baseline window and over the settled window gives its change: the settled mean minus
  the baseline mean;
- the excited input must hold its level. A window that spans more than 1 % of the excited input's step contains the step, and the trial is
  refused;
- when the request gives a drift tolerance, no input that was not stepped may change by more than it during the trial.

With one trial for each input, the input changes form a square matrix `U` and the output changes a matrix `Y`, one row per trial. Each output
is fitted by least squares on its own, `U·kᵢ = Yᵢ`, and the steady-state gain `K` (outputs by inputs) has row `kᵢ`. The fit is refused when `U`
has fewer independent steps than inputs, or a condition number above 10¹². The answer gives the gain matrix, the relative gain array
`Λ = K ∘ (K⁻¹)ᵀ`, the static decoupler `D = K⁻¹·diag(K)` (inputs by outputs), the pairing that `Λ` names (N4.5), the rank and the
conditioning of `U`, the conditioning of `K` and the residual norm. It names each channel with its symbol and unit, gives the trials, and
gives the number of samples between each baseline's start and settled window's end, each counted once. A singular `K` refuses the answer,
and no pairing is then named.

### C8. Measured-disturbance feedforward

The program adds a feedforward from a measured disturbance `d` to the `FeedForward` input of a `Control.PID`. Its feedback controller is of family PI (C0).

#### Setup

- The setup asks for the single-loop fields, and also for these items:
  - the measured disturbance: its compiled variable, unit and range;
  - the variable that Control Studio steps to move the disturbance during identification, and the step. That variable MUST be an explicitly actuated simulation or test input; Setup refuses any other before a write. The step is in the disturbance's unit, and Setup refuses one that is not positive or exceeds the disturbance's span (its maximum minus its minimum). In a simulation this is usually the simulated load itself;
  - the program's gain variable `K_ff`;
  - optionally, the lead time `T_lead`, the lag time `T_lag` and the filtered disturbance `z` that the program keeps. These three are all present or all absent. Their bounds are those of numerical annex N10.
- The explicitly actuated test (D-5, settled in wave 2b): the command is a `REAL` or `LREAL` variable that the program only reads and that sits at a memory address `%M`, where the simulation or the test writes it; a variable the program writes, or one at any other address, is refused by name before the write.
- The seed names the PI's three gain variables, the feedforward gain variable and, when the program has both, the lead and lag variables; the family is PI, and no two of these variables are the same variable (the rules of `validFeedforwardSeed`).
- The startup-done flag, when the draft names one, is an application state of the controller (C1), not one of its declared states.
- The program computes:

  ```text
  FeedForward = K_ff · (z + (T_lead / T_lag)·(d − z))
  ```

  where `z` is `d` filtered by a first-order lag with time constant `T_lag`. This is the lead/lag `K_ff·(T_lead·s + 1)/(T_lag·s + 1)` applied to `d`, with unit steady-state gain. Without lead and lag, `FeedForward = K_ff·d`.
- The disturbance is measured. The controller never writes it.

#### Identify

- The PID is put in Manual. The manual output and then the disturbance variable are stepped. Identify runs one campaign with one experiment per input: each has its own interval in one recording, as the inputs of a PID group do (C7). The rule of C7 Identify applies: the disturbance is stepped at its own step, and the manual output at its own step when Setup gives one, otherwise at the loop's declared `mv_step`, which Setup writes.
- The disturbance is stepped through its declared write binding. Such a binding exists only for an explicitly actuated simulation or test input.
- One model is fitted from both inputs to the measurement. It is of at most second order (`identificationPolicy.maximumOrder` 2), so that each path keeps its own time constant.
- The fit MUST agree with an independent recording within 5 % (`identificationPolicy.maximumNormalizedError` 0.05). A first-order fit that shares one time constant between the two paths is not accepted.

#### Tune

- Tune designs the PI, with the family fixed to PI (C0), on the path from the controller output to the measurement, for the requirements.
- Tune is Automatic only (C8, settled in wave 2b): a named method, a structure other than PI and pinned settings are refused by name until they are served. The requirements are filled on the output path as a one-input view of the loop, and the PI is designed on that path by the engine's own predictor and placement.
- A reduced path is a synthetic identified model of its form (wave 2b, settled): a first-order path is `FirstOrderDeadTime` with its delay, or `FirstOrder` without one, and an integrating path is `Integrating`; a second-order path has no synthetic model, so it is not predicted and has no placement form.
- The feedforward is designed from both paths. Each path is reduced to first order with delay by the two-point method (C7): `(K_u, τ_u, θ_u)` from the controller output, and `(K_d, τ_d, θ_d)` from the disturbance.

  ```text
  K_ff   = −K_d / K_u
  T_lead = τ_u
  T_lag  = τ_d + max(0, θ_d − θ_u)
  ```

- The lead cancels the lag of the output's path. The lag reproduces the disturbance's path. A disturbance delay longer than the output's delay is approximated as further lag. When `θ_d < θ_u`, the disturbance reaches the measurement before any action can. That part of the deviation remains for the feedback.
- Tune shows both path models and the three values. On the model it shows the largest deviation of the measurement after a disturbance step equal to the identification step. It shows this with the PI alone and with the feedforward. The answer states the unit of each gain it shows: the unit of the measurement per the unit of the path's input for each path, the manipulated variable's unit per the disturbance's for `K_ff` (`1` when the two units are the same), and the units of the disturbance and of the measurement (`gainUnit`, `disturbanceUnit`, `measurementUnit`), all taken from the model's channels. The page composes no unit.
- The feedforward acts outside the loop. The margins of C0 are therefore those of the PI on the output path.
- The candidate holds the PI gains and the feedforward values by their variables: `K_ff`, and the lead and lag when the seed names them; the derivative variable is written as zero by Apply, as for any PI (C0).

#### Validate

This validation always runs on the model, and every case runs on the full two-input model. The model's inputs and outputs are bound through the loop's channels (B5.3): the controller output drives the heater input, the measured disturbance is written to the variable that its channel's Manual command names (the variable that the run's initial values and events write), and each model output is read from its measured signal. The disturbance input starts at its operating value unless a case writes it. The cases are the single-loop cases of C1 and the declared scenarios, with the candidate's PI and feedforward values written as parameter overrides, and a measured-disturbance case. In that case, from the operating point with the setpoint unchanged, the disturbance steps by its identification step at the start. The measurement MUST stay within the error allowance by the settling time: every sample from the settling time to the end of the test is within the requirements' maximum error of the operating value (settled in wave 2b; C1 states the same allowance for the target). A failed measured-disturbance case ends Apply (B5.4; settled in wave 2b). A declared scenario that supplies its own two-input model runs on that model's plant, from that model's own operating point, with the controller's initial values at that point (B6.2), so its case is planned on that model (lesson 84's campaign builds on it).

#### Apply and Restore

- Apply writes the PID gains and the feedforward values in one transaction, and switches to Auto. The `FeedForward` state starts at `K_ff` times the measured disturbance. `z` starts at the measured disturbance. The switch therefore does not bump.
- A measured disturbance that is a model input is read as a measurement, never as an actuator (C0).
- The `FeedForward` state's gain is bound in its own unit per the disturbance's unit (`%/kW` for lesson 84): a gain in another unit is refused at the operating point (C8, wave 2b).
- The gains and the feedforward values are one candidate's values: Apply and Restore write and restore the PI gains, `K_ff`, and the lead and lag when the seed names them, each through its own bound variable and bounds (N10).
- Restore previous settings returns all of them (B6.4).

### C9. Smith predictor

The program feeds the controller `y + c` instead of the measurement `y`. The correction `c` is a value that the program keeps. The PI around the predictor is of family PI (C0), and Automatic chooses no other family (D-1). A program keeps its own predictor, or uses the library block `Control.SmithPredictorFOPDT` (Specification 37 §2); the setup asks for the block's variables only in the second case.

#### Setup

- The setup asks for the single-loop fields, with the measurement being the process measurement `y`.
- A program that writes a seeded value of the predictor's PI, or initializes its PID, needs the startup-done flag, as a single loop does (C1); the setup refuses the draft that names no flag.
- It also asks for the program's variables for the model gain `K`, the time constant `T`, the delay `θ` in seconds, and the correction `c`. Each is a `REAL` or `LREAL` variable. The setup refuses two of these variables that are the same, or that are one of the loop's signals.
- When the program uses `Control.SmithPredictorFOPDT`, the setup asks also for the variable that holds the block's `Update` result (`SmithPredictorOutput`), for the variable of its `MaximumCorrection`, and for the variables of its `OperatingInput` and `OperatingOutput`. The four are asked together, and the seed names them (D-16, D-18). The `MaximumCorrection` is the program's own declaration: Apply never writes it.
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
- Design the PI, with the family fixed to PI (C0), on the model without its delay. The settling time used is the requirement's settling time less the delay. A settling time that is no longer than the delay is refused, with that reason. Tune refuses a named method, a structure other than PI and pinned settings for this structure: Automatic designs the PI.
- The prediction and the margins that Tune shows are those of the PI on the delay-free model. With a correct model the predictor makes the loop respond as that loop, so this is the loop the predictor gives.
- A program that uses the library block `Control.SmithPredictorFOPDT` (Specification 37 §2) takes the reduced model as its `ProcessGain`, `TimeConstant` and `DeadTime`, and the controller output and the measurement at the operating point the model was identified at, as its `OperatingInput` and `OperatingOutput`. Apply writes that operating point to the block's variables (D-18). Its `MaximumCorrection` stays as the program declares it: it is the application's protection, not a tuning value.
- Show the model values, the operating point and the gains.

#### Validate

This validation always runs on the selected model. It covers the single-loop cases of C1, run as actual ST. Each candidate run writes the candidate's model values and, with the library block, its operating point, as parameter overrides (B5.3).

With `Control.SmithPredictorFOPDT`, each isolated run observes the block's `Active` and `HistoryComplete` flags, its `Correction` and the program's `MaximumCorrection`, as channels of the run; a flag is read as 0 or 1 (D-16). The run fails when `Active` is FALSE at any sample, with the reason that the history is incomplete when `HistoryComplete` is FALSE, and that the predictor is not enabled when `HistoryComplete` is TRUE. It also fails when `|Correction|` reaches `MaximumCorrection`, which bounds the correction only when it is positive (Specification 37 §2.2). A bypassed or clipped prediction is not a valid one (Specification 37 §2.5). The predictor's requirement is one row of every run, with its own sentence. Its flags cannot be judged from averaged samples, so a run whose samples are averages is Inconclusive for that row. A program that keeps its own predictor has none of these channels, and its runs are judged by their requirements alone. The channels are ordinary observed symbols, so the runtime protocol does not change.

#### Apply and Restore

- Apply writes the PID gains, the model values and, with the library block, the block's operating input and output, in one transaction, and switches to Auto with `c` at zero. It writes no other variable of the block.
- The Auto-entry allowance is that of A6.4, applied to the PI on the measurement.
- Restore previous settings returns all of them (B6.4).

### C10. Gain scheduling over declared operating points

The program sets the proportional and integral gains of a `Control.PID` each scan, from a schedule table. The gains are interpolated between declared operating points by a scheduling variable, such as the measurement.

#### Setup

- The setup asks for the single-loop fields, the scheduling variable (its compiled variable and unit), and two to four operating points. Each point has a name, its value of the scheduling variable, a tolerance on that value in the scheduling variable's unit, and the program's proportional and integral gain variables for that point. The values MUST be strictly increasing in the order the points are given, as the library's `PIDGainScheduler` requires (Specification 37 §3.1); setup refuses another order and names the first point out of order. The setup also asks for the record the PID is configured from (`parameterRecord`), whose `ProportionalGain` and `IntegralGain` hold the active gains that the program interpolates. The scheduling variable MUST be `REAL` or `LREAL`, and the loop keeps the type that setup read from the compiled program, because the runtime reads a variable only as its exact type. Each tolerance MUST be above zero; setup gives it no default (N10). The startup-done flag follows C1 Setup. Required: the per-point gain variables that Apply writes, and the active gains that the program reads back in its parameter record. The library block `Control.PIDGainScheduler` (Specification 37 §3.1) is the recommended way for the program to interpolate them, with each point's tracking time taken from the PID record (`<parameterRecord>.TrackingTime`); the program may interpolate by its own code instead.
- A point with no value or no tolerance is refused under its name, and nothing is written; the form sends no row number, so the refusal does not name a field.
- Setup writes one loop per operating point, named after the loop and the point. Its parameters are that point's two gain variables. Its project family is `gain-scheduled`, and its seed is the `parallel-pid` seed of C2 with family PI and that point's two gain variables. Its `schedule` block holds the whole table: the parameter record, the scheduling variable, the points in the order given (name, loop, value, tolerance and gain variables), and the name of this loop's point. Every loop of one schedule carries the same table, and setup writes all of them or none (B1.3).

#### Identify

At each point, bring the process to that point's value of the scheduling variable in Manual first, then identify. The model then describes the process at that point. When Identify starts, the engine reads the scheduling variable from the connected runtime and records its value with the identification. Identify refuses when that value differs from the point's value by more than the point's tolerance; a difference equal to the tolerance is accepted, with an allowance of one part in 10⁹ of the tolerance for the rounding of decimal values (D-21). A value that cannot be read is refused.

#### Tune

Automatic designs each point's PI on that point's model (C1). The fixed design is the PI that Automatic designs on the first declared point's model, kept with the same gains at every point; it fails when its predicted response on any point's model breaks a requirement. Tune recommends a scheduled design only when the fixed design fails a requirement at one of the declared operating points. Otherwise it recommends the fixed design (D-19). The fixed design's prediction is made on each point's model against that point's requirements, and the scheduled design is each point's own PI. Tune needs an identified model for every point; a point without one is refused by its name. Tune records one table for the schedule: each point's candidate holds the gains of every point (the values Apply writes), so Apply and Validate of any point write the whole table. The answer to the page names the choice (`fixed` when every point has the same gains, otherwise `scheduled`), the reason, the scheduling variable and each point's gains. The choice is read from the table, so a reopened study gives the same answer as the live run.

#### Validate

This validation always runs on that point's model. It runs the actual ST with the program's schedule active, from the operating point. The gains that the program interpolates as the process moves are therefore the ones validated. Each run starts the scheduling variable at that point's value and writes the proportional and integral gain of every point of the table, as Apply writes them, so the run tests the schedule as a whole. The run records the active gains, the record's `ProportionalGain` and `IntegralGain`. The active gains MUST change continuously across the schedule. The continuity check is this: between two consecutive recorded samples of an active gain, the change is at most the continuity fraction, 10 % (N10), of that gain's span over the schedule's own points. The span of a gain is the largest minus the smallest value of that gain in the table that the candidate writes at the points. A gain that has the same value at every point has a span of zero, and it MUST NOT change between recorded samples. A run whose recording averages its samples (`record_every` greater than 1) cannot be judged from that recording, so the row is Inconclusive, as ON/OFF's scan-level rows are. A schedule's candidate runs record every scan when the run fits the sample and cell limits of A7, so their rows are judged on every scan; only a run too long for that is averaged. A hidden or discontinuous schedule fails.

#### Apply and Restore

- Apply is one transaction at one task boundary (B6.1). It writes the proportional and integral gain of every point of the table, each from the gains designed for that point (the fixed design's gains at every point, or each point's own design), and switches the loop to Auto. The start of the integral term follows C2.
- Restore previous settings returns every point's gains to the values they held before the Apply (B6.4). Restore into Manual is refused for a schedule: the runtime's transfer check (`transfer.rs:1052-1090`) requires every binding state to equal its applied value at the next scan, so a Manual restore over the whole binding fails whenever the program moves an owned state, such as the PI integral. The refusal stands until a table-only transfer exists.

### C11. Trajectory feedforward

The program moves its reference along a planned trajectory `r(t)` when the setpoint changes, for example a motion profile. It adds a feedforward through the `FeedForward` input of `Control.PID`.

#### Setup

- The program computes the planned rate `ṙ` and the planned acceleration `r̈`. The feedforward is:

  ```text
  FeedForward = K_v · ṙ + K_a · r̈
  ```

- Setup asks for these items:
  - the trajectory choice: one of the loop's further signals (`signals.additional`, role `state`), the planned trajectory that the feedforward follows;
  - the feedforward term: the program's variable that holds the feedforward output, declared as a law state that starts at zero, so that the switch to Auto does not bump;
  - the gains: the PID's three gains, and the two feedforward gains `K_v` and `K_a`, each with its range.
  The planned rate `ṙ` and acceleration `r̈` are the program's own variables. The loop names neither.
- The project controller declares the `trajectory-feedforward-pid` seed. It has exactly seven keys: `kind`, `family`, the three PID variables, `rateSymbol` and `accelerationSymbol`. Those are five distinct bound parameters. The seed names neither the setpoint nor the trajectory: the setpoint is the loop's `signals.setpoint`, whose change starts the trajectory, and the planned trajectory is the loop's `signals.additional` entry. Lesson 90 keeps its seed as the shipped file writes it.
- The startup-done flag is an application state (C1, `controller.project.initialization.applicationStates`), as for every program's own law.

#### Identify

As C1. The feedforward is designed from the same model.

#### Tune

- The PID for the chosen family is designed as Automatic designs `Control.PID` (C1). For lesson 90's axis, `1 / (s·(s + 1))`, an integrator with a lag, C1 places the PID's poles, because SIMC has no rule for that form; its feedforward gains below are `K_v = 1` and `K_a = 1`.
- For a model `n / (a·s² + b·s)` from controller output to measurement, an integrator with a lag when `a ≠ 0`:

  ```text
  K_v = b / n
  K_a = a / n
  ```

  With these values the model follows the trajectory exactly. For example, for a torque-to-angle model `1 / (s·(J·s + B))`, `K_v = B` and `K_a = J`.
- A model of another form has no trajectory-feedforward rule, and Tune says so. A discrete model has z-plane poles, so it has no such rule either. A manual entry of the PID settings takes the rate and acceleration feedforward gains too (C4 and B4.2 name them as the page's settings): the page asks for them through `familySettings`, and a manual candidate without both is refused, so Apply and Validate never write a zero that the engineer did not enter.
- The feedforward values are written to the candidate beside the PID gains, by their variables (C2).
- A delay is not anticipated. Its effect remains for the feedback.
- The feedforward acts outside the loop. The margins of C0 and the Auto-entry allowance of A6.4 are therefore those of the PID.
- Tune's predicted response runs the program with the current values and with the tuned values (B4.5).

#### Validate

Validate runs the actual ST in the single-loop cases of C1. Its location follows B5.3. A change of the setpoint that the loop's `signals.setpoint` names starts the program's trajectory. The planned trajectory that the loop's `signals.additional` names is observed with the response, and the candidate's `K_v` and `K_a` are written to the run as parameter overrides beside its PID gains.

#### Apply and Restore

- Apply writes the PID gains and both feedforward gains in one transaction through the project adapter, and switches to Auto. Each of the five values is checked against its binding's bounds (C0). The planned rate and acceleration are zero at rest, so the switch does not bump.
- Restore previous settings returns all five values.

### C12. Open-loop experiment loops and project-scripted experiments

This structure has no controller law. No controller is designed or written, so the Tune, Validate and Apply and Restore stages do not apply: the engine answers each of them with the status Not applicable and the reason, and the page shows that status in place of their actions. The restoration of the experiment's own outputs is part of the experiment, as described below. The engine serves this structure: `open` opens it, and `identify` runs its experiment. The automatic progression of §0.1a (`controlStudio/run`) takes Identify alone for it and ends there, with no failure.

Two kinds of loop follow this section:
- an **open-loop experiment loop**, with `topology: "open-loop"` and `design: "project"`, as Setup describes;
- an **observation loop**: a closed loop whose controller is the program's own and is not bound as a law (`design: "project"` with no `controller.law`, no `controller.project` and no other structure block). Its experiment is project-scripted observation, and Identify fits the plant from the recording as B3.5 says for closed-loop data. Unlike an open-loop loop it may name its setpoint, which the recording keeps as a measured signal; Control Studio never writes it.

#### Setup

- **Topology.** An open-loop experiment loop declares `topology: "open-loop"`. It has a process variable and a manipulated variable, and no setpoint. The loop MUST NOT carry or invent a setpoint, and it exposes no error or setpoint metric. An observation loop declares no topology. Both require a project-owned experiment or controller.
- **Execution owner.** The loop's execution is one of two kinds:
  - `control-studio`: Control Studio writes the reviewed excitation through the runtime experiment service;
  - `project-scripted`: the project owns the scenario. Control Studio observes it with zero external amplitude, records the declared duration, and writes no duplicate excitation. The project owns the experiment kind and the activation. When the project declares a baseline, the recording starts with the plan: Control Studio waits for no steady baseline (B3.4 step 2), so the recording is the declared duration from the first sample on, and its end is the restoration sample (B3.4 step 4). When it declares none, Control Studio waits for the steady baseline first, as for its own experiments, and the plan, still the declared duration, starts after it.
- **Activation.** A project-scripted scenario MAY declare one writeable `BOOL` activation binding, named by symbol or by address (A5.1). Without it, the scenario is observation only. When it exists, the authorized start writes it TRUE through the runtime experiment service. Abort, timeout, failure and normal completion restore it FALSE. The restoration is confirmed by the third execution of the loop's task after the restoring write (A7). The plan keeps the activation as the project's authority. It is not SP, not MV, and not a second excitation.
- **Parameter purpose.** A project MAY declare `parameterPurpose: "experiment"`, with a `parameterBinding` and its Boolean activation symbol (`manual_value: false`). This is valid only for project-scripted execution with an activation binding. Parameter updates preserve typed bounds, explicit authority, observed values and rollback through the existing owner. The experiment settings are frozen through review, baseline and execution: the project test's review binds the values that the runtime holds when it is made, because its identity digests them (§0.1a), so a setting changed between the review and the start gives another identity and the engine refuses the start (E11.8); from the start the engine refuses a write of the settings (Experiment settings, Frozen through the test). That ownership is released only after terminal restoration. The engine serves these settings through `controlStudio/parameters` (Experiment settings, below).
- **Baseline.** A project-scripted experiment MAY declare `baselineSeconds`. The reviewed maximum duration includes the baseline, the held experiment and the restoration.
  - During the baseline the runtime recorder runs with activation disabled. The timed-trajectory service enables the project only after the baseline.
  - A project test that declares a baseline waits for no steady baseline. Its recording starts with its plan, so the samples of the declared baseline are the first of the recording, and everything that a baseline gives elsewhere comes from them: the noise that a return to Auto allows (A6.4), the window that the fit takes its operating point from (numerical annex N1.3) and the baseline of the record, which runs from the recording's start to its last sample before the plan enables the project: the project reads the activation and reacts in the same execution, so the sample at the instant the plan enables it already holds the first change of the plant input. A project that declares no baseline waits for the steady baseline of B3.4 step 2 before its plan starts, and that baseline gives the noise, the fit window and the record's baseline as it does for a campaign: without it, the samples before the first input change would be fewer than the three the fit needs when the project reacts to its activation at once.
  - The runtime recording is planned for the declared duration and the restoration margin of a Test recording, at least 10 s or 10 % of the duration, so that its limit never cuts the restoration sample, which comes at the end of the declared duration. It plans no wait when the project declares a baseline, and the wait for a steady baseline of B3.4 when it declares none. The run stops the recording once the experiment has ended.
  - The project stays responsible for qualifying its actual measurements before actuation.
  - The baseline and the response samples are saved together. A baseline MUST NOT be rebuilt from nominal values or from a qualification flag.
  - The plan retains the baseline duration. A maximum duration that leaves no sample interval after the baseline is rejected.
- **Hold parameter.** When the engineer chooses how long the project holds its step, the loop names that parameter in `holdParameter`. It is a declared `parameterBinding` parameter in seconds, under `parameterPurpose: "experiment"`. Review reads the applied value from the runtime. It records the baseline, every scan of the hold, and one restoration sample. For example, 5 s + 30 s + 0.1 s = 35.1 s. `durationSeconds` is then the maximum, and it MUST record the longest permitted hold. Review refuses a hold that cannot be read, lies outside its declared bounds, or exceeds that maximum. The duration the review shows, the timeout among its stop conditions and its identity are that recording's. The start reads the applied value again before it records anything, refuses it as the review does, and plans the same recording as its duration; so does the Identify stage of `controlStudio/run`, which has no review. A hold that cannot be read is refused with the sentence that says why: the runtime cannot be reached, or it does not report the parameter as a finite number (the engineer then inspects and applies the experiment settings first); a request that carries no connection is refused with the next action Connect a runtime. A hold outside its bounds is refused with no next action, because the engineer applies another value with the experiment settings. A hold that exceeds the maximum is refused with Limits as the next action (B3.9).
- **Maximum duration** follows A7: 1 s to 24 h of process time. The engine reads the duration as typed in Limits and Requirements (`15 min`, `1 h 30 min` or plain seconds) and refuses a time it cannot read with its sentence, under the field it names.
- **Abort.** The loop MAY declare `experiment.abort` alongside `experiment.activation`. The declaration names a writeable `BOOL` binding, by symbol or by address (A5.1), with `direction: "write"`, `iecType: "BOOL"` and `assertedValue` (a boolean). The abort and activation bindings MUST be distinct. Other experiment kinds MUST NOT accept an abort declaration.

#### Declared analysis parameters

A C12 observation or open-loop loop whose declared analysis maps its parameters (`analysisParameterSymbols`, C0) has the engine's parameter tool, `controlStudio/parameters`. The engineer uses it to apply the value that the analysis selects, or a value within the analysis's bounds, to the program's parameters, and to restore the values from before. The page shows this tool on the Tune page, after the analysis it serves (the course's Tune → Root locus step), with its own step button, which needs no selected project test; the project test itself stays on the Identify page. Its fields are the parameters that the engine's answer names, with the bounds that the analysis allows, and the page names none of its own. The rules are these:

- **Which variables.** A write sets only the symbols that the mapping names, one for each analysis parameter. Every other value of the parameter record keeps the value the runtime reports, because the runtime writes the whole record (E7.3). The mapping is valid when it names each analysis parameter once, with a different parameter of the loop's binding whose unit is the analysis's unit and whose range contains the analysis range. Any other mapping refuses the tool (E7.8).
- **Bounds.** A value lies within the analysis parameter's own minimum and maximum, after the IEC rounding of its symbol's type: a REAL is rounded to single precision (C0). A value outside those bounds is refused before any review.
- **Selected values.** The values the analysis selects are its swept parameter at the selected value and each other parameter at its declared value. The engine answers them as a draft that a review can start from; the engineer may enter any other value within the bounds.
- **Review.** A review writes nothing. It names the identity of what its confirmation writes: the loop's revision, the program, binding and active revisions of the record, the action, the revision that a Restore returns to, the symbols written and the whole record (A2.5, E11.8). The confirmation (`confirm`) names that identity. The engine reads the runtime again and makes the reviews the identity may name: the Apply of the values that the confirmation sends back, and the Restore of the values that the runtime retains. The write is the one whose review gives the named identity, so the engine, not the page, decides whether a confirmation applies or restores; the identity binds the action, so the two never give the same one. A record or a loop that changed since the review gives another identity, so the write is refused and the review is asked for again. The values of a review go to the engine as the text the engineer typed (`json`, at most 64 kB): the engine reads it as a JSON object of numbers by symbol, refuses a text that is none with its own sentence (`The parameter values are not valid JSON.`, or `Provide exactly the declared finite parameter values.` for JSON that is no such object), and answers the values it read. The page reads no JSON. It sends the identity it shows and the values the last review answered, if it answered any, and the engine decides whether they still give that identity: a confirmation whose identity neither the engine's new review of the Apply nor of the Restore gives is refused by the engine with its own sentence, "The parameter record or the loop changed since the review. Review the values again before the write.", and the page shows that sentence (E11.8).
- **Write.** The identifier of the write is journaled before it is sent (E11.2). The write goes through the runtime's parameter transaction (E7.3) and is confirmed on the record that the runtime reports (E7.4). A physical target needs the engineer's approval (A2.5, E10.2): a review of a write to a physical target says so (`physical`), the page shows an approval box with it, the tool's step button waits for the box, and the confirmation carries the approval (`physicalApproved`); the engine refuses a write to a physical target without it. While a write is pending, no other write is started. A journaled write whose record is no longer pending is cleared on the next request (E11.4). The write's final answer says how it ended (`status`): `applied`, with the engine's sentence "Reviewed values observed after ST execution.", or "Previous values restored and observed after ST execution." for a Restore, and the values and the record that the runtime confirmed; or `failed`, with the engine's sentence of what stopped it: the runtime's reason, "The reviewed values were not confirmed after controller execution." when the runtime does not report the written values after the owning task executed, or the refusal before the write. A failed write shows no values as written. While the write waits for its task boundary, its operation reports its progress in the engine's words, "Waiting for the owning task and observed values…", which the page shows with the tool. The page shows the engine's sentences, writes no progress sentence of its own, and decides no outcome; while its own request is in flight it only names that request on the tool's button.
- **Offer.** Each answer of `inspect`, `review`, `reviewRestore`, `invalidate`, `released` and of a write names the tool's offer (`offer`): the step its own button runs next (`nextStep`) and every action the engine takes now (`available`), from the engine's state, the loop's claim and journal and the review the answer gives. While the settings are frozen (Experiment settings, Frozen through the test) the tool only inspects, and its next step is Inspect. After a review its next step is the confirmation of that review, with Inspect, Review and Restore beside it. Otherwise, after an inspection, an edit or a write, its next step is a review, with Inspect and Restore beside it. Before the engine's first answer the tool's step is Inspect, which reads only and which the engine takes in every state: the open answer's project test form names this first offer (`projectTestForm.parameterOffer`), so the page starts the tool from the engine's offer and holds none of its own. The page renders the offer: its step button, its fields while a review is offered, and its Restore while a Restore is offered. It keeps the identity of the engine's last review and no phase of its own, and its controls wait only while its own request or its own project test runs.
- **Restore.** A Restore reviews and writes the values that the runtime retains from before the last write (E7.5), under the same identity and confirmation rules. A Restore of a record that retains no previous values is refused.
- **Edit.** A change of the values after a review (`invalidate`) spends that review, and the engine answers the sentence the page shows, "Values changed. Review again before applying.", with the tool's offer. An inspection answers "Current values loaded. Edit them, then review the exact change.", the review of an Apply "Review the exact values before applying them.", and the review of a Restore "Review the values from before the last write before restoring them." A draft (`draft`) answers its copy note, "Selected analysis values copied for review. The runtime has not changed." Neither reads the runtime (C12, E11.1).
- **Test.** The bounded Test that compares the responses is the project test of this loop (above, Setup and Identify): its duration is between 1 s and 24 h (A7), and its baseline, stop conditions and restoration are those the project test declares. The values the tool applied stay frozen through the test and are released only after its terminal restoration (Parameter purpose). The final answer of a finished project test, whether it carries a model or the fit's refusal, carries `settingsReleased`, the sentence that says so, exactly when it carries the restoration the runtime confirmed (or that was not required). When the test ends, whether it finished, was aborted or failed, the page asks the tool (`released`, which reads no runtime), so the end of every test is answered alike: the engine answers `settingsReleased` exactly when no operation of the engine works on the loop and the loop's journal holds no unfinished operation, which is the case once the test's restoration is confirmed or none was needed (A8.5), and otherwise answers the reason in `message`, and both answers name the tool's offer that follows. The page shows the sentence with the tool and renders the offer; an end whose question finds no answer leaves the tool as it was.
- **Other loops.** The engine serves this tool for no other loop. A loop with a controller law, or with the program's own law, that declares an analysis is refused with the cause `controller-capability`, and its parameter requests are refused with it.

#### Experiment settings

A C12 loop that declares `parameterPurpose: "experiment"` (Setup, Parameter purpose) and maps no analysis has the engine's parameter tool for the settings of its project-scripted experiment, `controlStudio/parameters`, as a declared analysis's parameters have it. The engineer uses it to read the settings that the runtime holds, to apply values within their declared bounds, and to restore the values from before. The page shows the tool inside the project test on the Identify page, from the project test's form (`projectTestForm.parameterTool` is `experiment`); its fields are the parameters of the form's `parameterBinding`, and the page names none of its own. A loop that also maps an analysis has the analysis's tool, and no other. The rules of the declared analysis's tool (Review, Write, Restore, Edit and Test, above) hold, with these differences:

- **Which variables.** Every parameter of the loop's `parameterBinding` is a setting, and a write sets all of them: the values name exactly the binding's symbols, each finite, and the runtime writes the whole record (E7.3). The binding's mode is the activation with `manual_value: false` (Parameter purpose), so the runtime refuses a write while the activation is TRUE.
- **Bounds.** A value lies within its parameter's own declared minimum and maximum, after the IEC rounding of its type (C0). A value outside them is refused before any review, with the sentence that names the symbol, the value and the range. The loop's hold parameter (`holdParameter`) is one of the settings, in seconds.
- **No draft.** The settings have no analysis whose values could be copied, so a `draft` request is refused.
- **Frozen through the test.** The project test holds the settings. Its review binds the values that the runtime holds when it is made (Parameter purpose, §0.1a): a setting changed between the review and the start gives another identity, and the engine refuses the start (E11.8), so the page locks nothing for the review. From the test's start the engine refuses a review or a write of the settings, and a Restore, while an operation of the engine works on the loop (the test runs, A8.5) and while the loop's journal holds an unfinished operation (the test's restoration is not confirmed, A8.5, A7). It refuses with the loop-busy sentence in the first case and with the unfinished-operation sentence in the second. The same refusal holds for a declared analysis's tool.

#### Identify

- **Excitation.** `control-studio` execution follows B3 and A6.5. `project-scripted` execution is observation only. Its recording covers the declared duration and the project's scenario. Identification of that recording follows B3.5. Identify (Automatic) runs the project's scenario whatever the controller mode; an explicitly chosen method is refused, because the project owns the experiment kind and Control Studio writes no duplicate excitation. A project-scripted loop is one observation even when it has several plant inputs: the engine plans no step for any input, so no input needs a Manual command (C12, Setup), and the recording keeps every signal the loop declares for its channels (the process variable, the manipulated variable and each additional signal).
- **Timing.** Experiments follow the timing of B3.4, not a fixed fraction of the duration (A10.2).
- **Review.** The review shows the abort assertion together with the activation and its restoration. Starting an experiment never clears an asserted abort. The review also names the stop conditions the runtime applies (E5.5), so the engineer sees them before the start. It states the test signal in the engine's words (`testSignal`): the activation with the TRUE that the start writes to it, such as "P1.ExperimentEnabled · TRUE while the test runs", or "Observation only: no signal is written" when the project declares no activation. On a physical target its approval block carries the sentence the page shows with it (`message`): "Physical target. Approve the reviewed test before it starts; nothing is written before your approval." The page shows these words and infers none.
- **End of the test.** The answer of a finished project test states how it ended (`testOutcome`): "Project test completed.", followed by "Outputs restored." when the runtime confirmed the restoration of what the test wrote. A cancelled test ends with the sentence of the cancelled update ("Identification cancelled."), and a failed test with its failure's text. The page shows the engine's sentence as the test's status and composes none.
- **Terminations.**
  - A normal completion of the duration restores the activation value, without asserting the abort.
  - A user abort, a runtime fault, invalid data (a forced, missing or out-of-range value) or any other failed termination asserts the declared abort input, and then restores the activation.
  - Without an abort binding, a failed termination restores the activation only.
  - The ST program owns the resulting actuator policy. For example, a normal completion may hold an output at 75 %, while an abort latches it at 0 %. The runtime invents no controller-specific policy.
- **Restoration status.** The status of the abort binding is `not_requested`, `pending`, `observed` or `failed`. Restoration stays pending until an unforced scan observes both the asserted abort, when one was asserted, and the restored activation. The restoration is confirmed by the third execution of the loop's task after the restoring write (A7). Missing, mistyped, forced or unobserved data never confirms success.
- **Fit refusal.** A project test is Identify with the Automatic or the declared method on a project-scripted loop (§0.1a). Its
  recording is fitted as B3.5 says. When the recording cannot yet support a model, the fit refuses, and the refusal does not fail
  the test: the test finishes when its recording is complete and its outputs are restored, and its answer carries the kept
  recording, its recorded samples, the restoration the experiment confirmed, and the refusal in full as the note that the page
  shows under the completed test. The study keeps the recording and no model, so its current model stays as it was (B3.7). A fit
  that the recording supports is kept as an Identify result, as before. A cancellation, an incomplete recording or an unconfirmed
  restoration still fails the test.

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
   - The declared application states of a program's own law (C1, `controller.project.initialization.applicationStates`) are written at this same boundary, inside this transaction: at an Auto entry each takes its declared value (the startup-done flag, TRUE), and a Manual or reviewed-target transfer keeps the value the runtime reports, as the declared-state transfer does when it does not align its output.
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
tank at 45 °C, a process whose slowest mode, the tank and the room heating
together, has a time constant of about four minutes, so the journey runs in
well under an hour at ten times real time. Against that project's runtime started outside VS Code in
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
