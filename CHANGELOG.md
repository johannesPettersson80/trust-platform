# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog and this project adheres to Semantic Versioning.

## [Unreleased]

- Project-import name collisions (`duplicate imported declaration`) are reported
  at a declaration in the file itself, or else at the first reference to the name
  in that file. They were reported in every project file at the offset of the
  declaring file, so one duplicate showed up once per file at unrelated text.
  A global or POU that reuses the name of a standard function block (`TON`,
  `R_TRIG`, ...) is reported at its own declaration and at the uses of the name in
  other files, with a message naming the standard function block, instead of at the
  start of every other file and with a "previously declared here" link to nowhere.
- Untyped integer literals are contextualized to bit-string targets: `b := 16#FF`,
  `w := 65535`, `d AND 1`, `d = 0` and `CASE b OF 16#FF:` are accepted for
  `BYTE`/`WORD`/`DWORD`/`LWORD` when the value is representable, as they already
  were for integer targets and bit-string initializers. The runtime gives such a literal
  the full range of its target, also in declarations and array and structure initializers,
  so `d := 16#FFFF_FFFF` and `d : DWORD := 16#FFFF_FFFF` no longer fail to compile with
  "integer literal out of range". Runtime CASE labels are evaluated in the selector type,
  so an untyped label matches a bit-string selector.
- The integer literals `0` and `1` are Boolean values where a `BOOL` is expected, as IEC
  61131-3 §6.3.2 specifies: `x := 1`, `F(flag := 1)`, `x = 1`, `x AND 1`, a structure or
  array default such as `active : BOOL := 1` and `BOOL#1` are accepted and run, while
  `x := 2`, `x := INT#1` and `x := 1 + 0` stay errors.
- `trust-lsp check [--project DIR] [--format text|json] [--file PATH]... [--deny-warnings]` prints the diagnostics the
  editor shows for every source file of a project, without an LSP client (docs/specs/14-lsp.md §7.7): the project is
  loaded from `trust-lsp.toml` exactly as the language server loads a workspace folder, paths are relative to the
  project in the platform's form, and the exit status is 0 without errors, 1 with errors (or warnings under
  `--deny-warnings`) and 2 for a usage or I/O problem, including a `--file` that is not one of the project's source
  files.
- Editor and project diagnostics are faster on large projects: a file's analysis no longer rescans every project
  file for global constants and `VAR_CONFIG` entries, reuses the project symbol merge it already computed, and imports
  other files' symbols without re-indexing the whole table per import (measured on a 400-file project: diagnostics after
  an edit in 0.14 s instead of 1.8 s). The reported diagnostics are the same; symbol-table walks now run in declaration
  order, so lists built from them, such as unused-symbol warnings, quick fixes and the runtime web IDE's symbol
  search, follow the declaration order, and the errors for incomplete addresses without a `VAR_CONFIG` mapping are
  listed in declaration order (they followed hash order). `scripts/perf/project_scaling.py` measures project-size
  scaling over LSP.
- Control course: all 89 lessons follow the lesson template and the lesson map (`docs/specs/control-course-lesson-template.md`,
  `docs/specs/control-course-lesson-map.md`). Each lesson starts from the previous lesson's result and explains its new
  idea in four steps: a plain definition, what it means in words (with a comparison a PLC engineer knows), the formula
  with every symbol explained, and a worked example on the lesson's own plant. It ends with **Check yourself** (three
  questions with answers) and shows one figure drawn from its own Control Studio run, with numbered marks that answer its
  predictions. Every number comes from a real run or is labelled as calculated.
  Lessons that introduce a plant show it in **Devices & Connections**; lessons 10 and 45 change the plant there (a sensor
  bias, a starting level) instead of in `simulation.toml`. Thirteen lessons explain a new mathematical tool in a **Math you
  need** box. Control Studio now does the design in the lessons whose map says so: lead/lag design (56), the cascade
  (63), the Smith predictor (67), the PID group (71) and corner-case validation (86, 89). Trajectory feedforward is now
  lesson 61, directly after lesson 60 on the same axis, and feedforward control lesson 62. Lesson 22 designs from a saved
  heater model so its result is repeatable. Lesson 40 gained its missing debug configuration. Stale values were corrected
  against the recorded runs: for example, lesson 25 now quotes lesson 20's measured 4.42 s ultimate period, and lesson 50
  its recorded lowest velocity.

- Control Studio designs a trajectory feedforward (Specification 35 §17.16): for a PID whose program adds `Kv·ṙ + Ka·r̈` of a planned trajectory, Tune designs the PID as for `Control.PID` and sets `Kv = b/n` and `Ka = a/n` from a model `n/(a·s² + b·s)`, which for torque to angle are the friction B and the inertia J. The PID's margins and Auto-entry allowance apply, a gain outside its declared range is refused, and Tune names all five values.
- Control Studio Validate names an error measured from a reference other than the setpoint after that reference, for example "Maximum error from planned angle", instead of calling it a steady-state error.
- Control Studio's Manual tuning names a project controller's gains as its Automatic result does, instead of showing their raw conventions.
- Control Studio's measured frequency response lists and plots its tones in ascending frequency with the phase unwrapped, as on a model's Bode plot; a resonance's phase turn was shown as a jump from −177° to +171°.
- Control Studio Requirements offers **Measurement noise** whenever the loop declares a measurement filter (a low-pass or notch) among its tuning components; that filter's validation needs the noise band.
- Control Studio designs P for a delayed integrating process (SIMC's Kp = 1/(K′(λ + L))) and checks it like PI; Apply on an integrating process starts from the level where it rests instead of waiting for the identification's level to return.
- Control Studio Requirements always offers an optional **Input disturbance**, a constant load added to the plant input in its own trial; Automatic tuning checks that load trial on the model for every family and refuses a design whose gains leave their declared range, naming the gain it needs.
- Control Studio identification fits an integrating process with its delay, so SIMC's integrating PI is available for an identified tank.
- Control Studio says why P is refused: unavailable on a delayed process, with the gain its offset would need, or unable to damp a shaft by the settling time; these refusals point to choosing another controller.
- Control Studio Automatic tuning starts from the More robust response also when every requirement is entered; before, a fully entered task started from Balanced while generated criteria started from More robust, so the same requirements could design different controllers.
- Control Studio Requirements refuses a validation duration shorter than the latest check of a declared scenario and names the duration it needs; Validate refuses such a campaign the same way instead of failing with a generic sample-window error.
- Control Studio: the Auto-entry allowance of a program's own P, PI or PID covers the measured noise through its gains, as for `Control.PID`; a noisy measurement no longer fails the transfer check at a near-zero tolerance.
- Control Studio: when Automatic tuning cannot meet the requirements, the next step it names is revising the requirements, and the Tune page no longer shows the previous candidate beside the refusal.
- Control Studio applies a static decoupler: a PID group whose program writes `u = D·v` in Auto has Tune calculate `D = K⁻¹·diag(K)` and each PI through it, and Apply write the gains and `D` together, each controller starting at its share of the plant inputs (Specification 35 §17.15). The runtime's controller transfer gains the `decoupled-output` initialization formula, and an isolated run records up to 64 channels (was 32), so a group's transfer check fits; the channel-sample budget is unchanged.
- Control Studio setup form: **Gain schedule** sets up a `Control.PID` whose program interpolates its gains between two to four operating points; it writes one loop per point, identified, tuned, validated and applied at that point with the schedule active (Specification 35 §17.14).
- Control Studio setup form sets up LQG: the state-feedback structure's LQG design takes the program's estimates, their gains and observer gains, the noise variances and the model file the estimator implements (Specification 35 §17.1).
- Control Studio Tune shows a state-feedback candidate's gain, phase and delay margins and Ms for its loop broken at the plant input (Specification 35 §17.10).
- Control Studio setup form: **Smith predictor** sets up a `Control.PID` that reads the measurement plus the program's predicted correction (Specification 35 §17.13). Tune writes the model's gain, time constant and delay into the program and designs the PI for the process without its delay, refusing a settling time no longer than the delay. Tune now shows the margins beside every candidate on the Tune page itself: the page rebuilt the candidate without them.
- Control Studio setup form offers as Auto/Manual signal only BOOLs without an I/O address, which Apply can switch, and says why when a program has none; previously an addressed mode was accepted and Apply refused it after identification, tuning and validation. Tune's peak sensitivity Ms is the peak over every frequency: a strictly proper loop's sensitivity tends to one beyond the plotted range, so Ms is never shown below one for it.
- Control Studio setup form: **Feedforward** sets up a `Control.PID` whose program adds a feedforward from a measured disturbance (Specification 35 §17.12). Identify fits one model from the controller output and the disturbance, Tune designs the PI and the feedforward gain, lead and lag from both paths and shows the predicted deviation with and without feedforward, Validate adds a measured-disturbance step, and Apply starts the feedforward state at the measured disturbance so the switch does not bump. The runtime's controller transfer gains the `scaled-measurement` initialization formula.
- Control Studio: a cascade set up in the form is one loop tuned together (Specification 35 §17.11): one identification of the plant input to both measurements, the inner controller designed first and the outer one through the predicted closed inner loop, one validation of both in Auto, and one Apply and Restore that switch both controllers together. The runtime's controller transfer switches additional controller modes at the same task boundary (`additional_modes`).
- Control Studio setup form: **Other law** sets up a law Studio has no design formula for (MPC, ratio, split range, override, gain schedule, …): Tune searches up to 64 combinations of the program's coefficient values on the actual ST and uses the best one that meets every requirement (Specification 35 §17.1). Family names read as words ("Paired PI", "State feedback").
- Control Studio setup form: **Own P, PI or PID** sets up a P, PI or PID law the program computes itself from its own gain variables (Specification 35 §17.1). Tune designs the gains to the requirements as for `Control.PID`, offers only the families the law has, and checks them on the actual ST; validation starts the integral term and the derivative's previous value bumplessly.
- Control Studio Tune shows every PID-family candidate's gain, phase and delay margin and peak sensitivity Ms (Specification 35 §17.10). Automatic tuning places the closed-loop poles for the declared settling band when no formula response settles in time, so tight bands on delay-free models are met instead of refused.
- Control Studio accepts loop signals without an I/O address: the controller output and setpoint may be program variables, and the measurement may be one the program only reads (Specification 35 §17.1). Plant simulation reads and writes such program variables by their symbolic path (Specification 33 §13), so validation runs on them. A measurement the program computes itself is refused with the reason.
- Control Studio: each member of a PID group has its own identification step, and Tune predicts every member's response with the current and tuned gains by running the program on the group model (as for switching, state-feedback and other project-owned controllers). Applying a cascade's inner loop is refused, before anything is written, while its outer controller is in Auto, and the runtime rejects the transfer if the outer controller switches to Auto before it takes effect. A loop whose gain stays below one now gets its predicted response, with an unbounded phase margin. Setup names the controller action by the direction the output moves.
- Security: dependencies updated for new advisories: rustls 0.23.45 (RUSTSEC-2026-0285), salsa 0.28.5 (RUSTSEC-2026-0308; the compiler's value-returning queries keep returning values), event-listener 5.4.2 (RUSTSEC-2026-0221), ratatui 0.30.2 with lru 0.18 (RUSTSEC-2026-0253), non-yanked spin 0.9.9 and 0.10.1, and the VS Code extension's `brace-expansion` 2.1.7 and 5.0.12 (GHSA-q2hr-2g5m-vwhr, GHSA-qhr7-859c-m2p7, GHSA-6j4f-fj2g-mc7p). The seven tracked advisory exceptions were re-reviewed (no upstream fix for opcua 0.12, tiny_http 0.12 or zenoh's `paste`) and renewed until 2026-12-29.
- Control Studio's Automatic tuning designs PI, PD and PID to the requirements, as it already did for P. It uses the response nearest the declared one (More robust, Balanced, Faster) whose predicted response settles within the settling band by the settling time and stays within the allowed error; when none can, Tune names the requirement the fastest response misses and by how much. It previously always used the declared response, so a PI could miss the settling time that Validate then failed. The PID example's README follows the current workflow with measured results.
- The examples gallery offers the five Control Studio examples (PID loop, cascade, interacting loops, ON/OFF control, state feedback), and the Control Studio guide links them.
- Control Studio stays responsive during long studies: the host no longer resends every recorded identification run with each status message, sends the model-check recording only with results, and posts Apply's waiting progress only when the reading changes. A study with several long recordings could freeze the page for seconds at a time.
- Apply observes its test duration in the runtime's own time, bounded to max(3 × the duration, the duration + 60 s) of wall-clock time. It waited a fixed wall-clock time, so a runtime running behind real time (under load, or on a slower controller) was stopped early and a correct controller failed with "The recorded response does not cover the reviewed duration". `control.transfer.status` now reports `runtime_time_ns`, the runtime clock of the scan it reports.
- Control Studio no longer contains built-in physical model families: the thermal-balance and gravity-tank models, their identification, the step-by-step setup's unit-based choice of them ("Thermal energy balance" for W and °C, "Gravity-tank volume balance" for m³/s and m) and their native simulation are removed. Models are transfer functions or state-space models; a physical plant is supplied as one, identified from recordings or simulated by the project's plant. Lessons 68 and 69 supply the equivalent linearized two-tank state-space model.
- Tune shows the predicted response of an ON/OFF or state-feedback candidate and of the current settings, as it does for `Control.PID`: it runs the program's own ST with both settings on the identified model in isolated simulation. Nothing is written to the running controller.
- Apply waits, bounded and cancellable, for the process to return to the operating point the settings were validated at, showing the measurement's validated and current value, and then continues on its own. It asked the user to press Apply again. A manual output that differs from the validated point is refused at once with the value to return it to, because the process cannot settle by itself; Abort ends the wait without writing anything.
- Apply of a state-feedback (LQR or pole-placement) law no longer fails its Auto entry when the process rests inside the settling band but not exactly at the operating point: without a reviewed Auto-entry tolerance, the first output may differ by the law's response to that accepted deviation, Σ|K|·band plus the reference gain's share (Specification 35 §12).
- Control Studio names a PID group member's gains as it names a single PID's ("P1.Zone1 · Proportional gain (Kp)"), shows each member's path gain and its gain with the other loops closed in their unit (for example °C/%), and names a group's one-setpoint validation runs after the setpoint they change ("P1.Zone1C setpoint alone").
- Validate lists each check once: an ON/OFF controller's switching levels and dwell times are no longer repeated by its on-off output's identical checks, and the dwell rows read "Minimum ON time" and "Minimum OFF time". Result labels show program symbols exactly as declared ("P1.Tank2SetpointM", not "P1.Tank2Setpoint M").
- Control Studio setup records a program instance's program type (`Main` for `PROGRAM P1 WITH MainTask : Main`) in the loop file; it recorded the instance name.
- The setup form sets up an existing state-feedback law in the program, designed by LQR or pole placement: the instance that runs it, the plant input, the controlled output with its setpoint, and one to three further measured outputs, each with the variable that holds its gain, plus the reference gain. Studio identifies the plant with the measured outputs as the model's states, so every gain belongs to one measured signal. `examples/control-state-feedback-commissioning` holds the second of two tanks in series with both levels fed back to the pump.
- The setup form sets up an existing ON/OFF controller in the program: the instance that switches the output, its measurement, setpoint and output (switched between the output's minimum and maximum), Auto/Manual signal and manual output, the hysteresis and minimum ON and OFF times that Tune designs, and the retained output-on state and optional time in state. It also asks for the switched equipment's own minimum ON and OFF times, such as a compressor's restart protection; Tune never proposes shorter times. `examples/control-onoff-commissioning` is a cold room held by a switching compressor. The step-by-step setup remains for P-only controllers and imported adapters.
- Tune shows a PID group's result: every member's designed gains and, per member, its own path (gain, time constant, delay), the relative gain of the pairing and the gain the member has with the other loops closed. The setup form names a group's model channels after their signals, as Identify does, so a group set up in the form can be tuned from its identified model.
- Control Studio validates a cascade's outer loop on its identified model, which contains the closed inner loop, instead of on the project's plant: that run started the program from its declared values, with the inner controller in Manual, so the outer loop could never pass.
- The setup form offers **PID group** when a program has several `Control.PID` instances: the included members, each filled in from its own calls, share one Auto/Manual signal and are written as one group loop that Tune designs from the group model.
- A PID loop's bias is bounded as an offset of up to the output span either way. It was bounded to the output range, so a cascade's outer controller, whose output is a temperature setpoint, could not be validated with its zero bias.
- A variable mapped to `%M` memory (for example with `VAR_CONFIG … AT %ML16`) starts from its declared or `VAR_CONFIG` initial value at a cold start, and from its retained value at a warm restart, as IEC 61131-3 §6.5.1 and §6.5.6.1 require. Previously the first cycle replaced it with the zero memory image, and a cold restart kept the image from before the restart, so programs had to set such variables in startup code.
- Simulation plants may take a program variable in `%M` memory as a command source, not only a `%Q` output (Specification 33 §13.1). Control Studio uses this to validate the outer loop of a cascade: its model is driven by the inner setpoint the outer controller writes.
- Control Studio's SIMC rule tunes second-order models by Skogestad's half rule (τ = τ₁ + τ₂/2, θ = θ + τ₂/2) instead of asking for a first-order model; a second-order model with complex poles is still refused with the reason.
- PID groups can be tuned from their identified multivariable model (Specification 35 §17.9): each member's PI is designed by SIMC on its own path, reduced by the two-point method, for the gain it sees when the other loops are closed (Kᵢᵢ/λᵢᵢ). A pairing with a non-positive relative gain is refused with the pairing to use. A group declares this with the project seed `{ "kind": "decentralized-pid", "family": "PI" }`.
- The setup form names a controller variable it cannot offer because it has no I/O address, and how to map it (`AT %I`, `%Q` or `%M` in `VAR_CONFIG`), instead of leaving the field empty without a reason.
- Two example projects for Control Studio setup: `examples/control-cascade-commissioning` (a jacketed vessel under a two-`Control.PID` cascade) and `examples/control-two-zone-commissioning` (two heated zones sharing a wall, one `Control.PID` each). Their READMEs say how to set them up and in which order to tune.
- The setup form fills in each controller from its own compiled calls when the program has several PIDs: choosing a controller brings its parameter record, setpoint, measurement, manual output and output, and both controllers of a recognised cascade are filled in. Fields the user has chosen are kept. Previously only a program with a single PID was prefilled.
- Control Studio's setup form asks for the identification duration next to the identification step, instead of writing a fixed 60 s experiment into every loop. The duration depends on the process: a cascade's inner loop is identified over a few seconds from its fast response, a slow process over minutes.
- The setup form asks for the startup-done flag whenever the program itself writes a value that validation starts from (the setpoint, a manual output, the Auto/Manual signal or the parameter record) or calls the PID's `Initialize`, for a single loop and for both loops of a cascade. The language server reports every symbol a program assigns (`controlSymbols.writes`). Previously only parameter-record writes were found, and the inner loop of a cascade whose startup code reset the manual outputs was validated with a setpoint step that never reached the process.
- Control Studio's **Configure control loop** opens one form instead of a chain of prompts. The form is prefilled from the compiled program: the language server now reports how controllers are called (`controlSymbols.links`), so a `Control.PID`'s parameter record, setpoint, measurement, manual output and output come from its own `Configure`/`Update` calls, and a cascade (one PID's output feeding another's setpoint) is recognised and saved as two linked loops. The form asks only for what the program cannot say (units, ranges, the Auto/Manual signal, the identification step, and the startup-done flag when the program writes its own parameters) and writes the complete loop definition, including the seven retained `Control.PID` states. Loop discovery now compiles projects with their declared library dependencies; previously every project that used a vendored library failed setup with "cannot resolve type 'Control.PID'".
- Control Studio's additional-lag warning speaks of the process and the sensor instead of water, the unreachable code of the earlier Studio panel (71 files) is removed, the Routh–Hurwitz details show stability boundaries as decimals with the exact values on hover, and calculation tables use the shared table style.
- Tune explains a requested test change beyond the identified range: it names the range the model was measured in and the largest change it supports, instead of "Tune again".
- Control Studio webview tests that had to be enabled by hand now run in the VS Code suite (29 more rendered journeys); a rendered run in which no test passes fails.
- Control Studio keeps the validation results visible after a failed Apply. A rejected Auto entry that recovered previously replaced the passing rows with "Inconclusive · Scenario did not complete." for every validated scenario, and the failure and its recovery now read as two sentences.
- The control course was reviewed as a sequence. Lessons 11–13 and 15–17 are rewritten in the course's concise format with recorded results: Lesson 11 teaches the integral time T_i = K_p/K_i and keeps a Ki = 0.6 candidate recorded in Control Studio, and Lesson 21 compares that candidate. Lessons 5–10 have concise procedures. Derivation steps that were skipped are shown (complex roots and overshoot in Lesson 33, critical damping in 34, the PID characteristic equation in 35, transform pairs in 44, determinants in 70–71, the Riccati cost in 73 and others), terms are defined before use, wrong lesson references are corrected, and a corrupted formula in Lesson 77 is repaired. The gallery texts of the lessons follow their READMEs again, without the spaces an earlier conversion had removed.
- Control Studio's response chart keeps each row's axis name within its row. When a name such as "P1.ObservedKp (percentage-point/°C)" is wider than a short row, its unit moves to a second line; previously the names of stacked rows ran into each other.
- Control Studio's Identify waits, bounded and abortable, until the measured output is steady before it excites the plant, as the step-identification workflow requires. Pressing Identify plant again right after an experiment previously stepped a plant that was still falling, so the fit failed or its operating point was off.
- Identify with the Automatic method fits the simple models to a closed-loop setpoint step, as it already did for step, relay, PRBS and multisine tests; the fit now follows the experiment that actually ran instead of the requested method.
- Validate's Auto-entry check starts the controller where the project's plant actually rests, measured by holding it at rest, instead of at the fitted model's operating point. A small offset in the fitted operating point previously failed the exact transfer check.
- When Apply is refused because the process has not yet settled at the validated operating point, Apply stays available for the "Apply again" the message asks for.
- Tune shows why it is unavailable beside the Tune controller button when the ready model cannot be tuned by the selected controller. A recovered failed Apply now says "previous controller parameters restored" for every controller type, not "PID".
- Control Studio writes powers of s as superscripts (6s² + 5s + 1) in model equations and Routh texts, and analysis tables align their headers with their columns and separate the columns.
- Measured MIMO path analysis rejects a baseline or settled window that contains the input step instead of averaging both command levels, and explains every rejection in plain words.
- Tune accepts a supplied model on a loop that declares no identification experiment.
- Control Studio's Requirements editor has an optional Auto-entry tolerance in the controller's output unit, as the auto-commissioning specification requires; blank keeps the automatic allowance. A state-feedback or LQG controller with a noisy measurement can now be applied with a reviewed tolerance instead of the 1e-6 default.
- Control Studio refuses a validation run that exceeds the runtime's recording limits before starting it and names the limit: at most 32 recorded channels and 250,000 channel samples per run. The runtime's own refusal also names the exceeded limit instead of "execution exceeds cycle, sample or declaration bounds".
- The runtime scheduler keeps its cycles on the cycle-interval grid: a late wake-up no longer delays every later cycle, and a stall longer than one interval moves the grid instead of replaying the missed cycles. With 0.5 ms of wake-up latency, 100 cycles of 10 ms previously ended 50 ms late.
- Control Studio's Tune page names the recommended controller configuration as soon as a model is ready ("P1.Controller · PI — recommended") and keeps it marked when another configuration is chosen. Its predicted response compares the tuned controller with the one running in the project (Tuned / Current), and units read with a multiplication dot, such as %/(°C·s).
- Control Studio Apply recovers when the controller's first Auto output breaks the reviewed transfer tolerance. The runtime now fails that transfer, publishes the output held at the transfer instead of the rejected one, and keeps running, so Studio's Manual recovery and the restoration of the previous gains complete. Previously the rejection faulted and halted the runtime, and recovery ended with "The parameter-operation identity remains unresolved".
- Control Studio refuses to tune a model without measurement evidence or independent validation and now says so, with "Identify model" as the next step instead of "Tune again". This applies alike to supplied models and to models returned by Identify.
- Control Studio Apply now accepts a process that has settled at the validated operating point within the requirement settling band, instead of demanding an exact match that a live or noisy process never reaches. When the process has not settled, Apply names the measured and validated values and offers "Apply again" rather than "Tune again".
- Control Studio Validate shows the control requirements and any failed or inconclusive check as rows, and summarizes the passing generated range, rate, controller-state, unchanged-parameter and transfer checks in one row. Requirements declared by a lesson or project scenario always keep their own rows. A refused Apply no longer reports the completed transfer check as missing.
- Control loops treat a signal declared without a unit as dimensionless, so identification and validation recordings no longer fail with "Invalid run channels or sample structure".
- Lesson 19 (model-based PI tuning) now uses Control Studio's Tune, Validate and Apply with the real `Control.PID`: its program keeps applied gains and the operator setpoint, designs its startup gains by coefficient matching, and declares the setpoint-peak and 30 kW load trials for validation.
- After a Feedback relay identification, Control Studio shows the measured relay cycle under the identified model (number of cycles, relay amplitude d, cycle amplitude a, period Pu and Ku = 4d/(πa)), or why the cycles are not usable. When the cycles qualify, Tune offers **Relay rule (Ziegler–Nichols PI)**, which proposes Kp = 0.45·Ku and Ti = Pu/1.2 for validation. The relay results stay visible through Validate.
- Control Studio writes °C instead of degC in relay results and in the recorded-error and settling failure messages.
- After Apply, Control Studio keeps the study draft instead of warning "Could not retain the workflow draft": a commissioning recording may include the loop's additional signals, and only duplicate channels are rejected.
- Control Studio keeps large studies in workspace storage by storing each recorded run once and its samples column-wise, instead of reporting "Study evidence is invalid" after several validations.
- Control Studio identifies a relay, PRBS or multisine recording by first fitting the simple models (static, integrating, first-order and first-order with delay), simulated under the recorded input from the baseline before the first change, and keeps the simplest adequate one. Relay recordings of delayed processes no longer fail Identify with "No identifiable adequate state model"; they give the same delayed first-order model as a step test.
- The Feedback relay test measures the process noise during its baseline and switches just above it (mean ± twice the noise half-range) instead of at ±0.5 % of the measurement range, so the relay cycle meets the small-hysteresis assumption of Ku ≈ 4d/(πa).
- Validate's Auto-entry check allows the controller's proportional response to the measured process noise (twice Kp times the noise half-range from the model's recording) unless a transfer tolerance is reviewed; noise-free plants keep the exact check.
- Lesson 20 (relay tuning) now runs Control Studio's bounded Feedback relay test (d = 10 points) on the same heater and real `Control.PID` as Lesson 19, derives the relay-rule PI candidate in Tune, validates it against the Lesson 19 setpoint and load trials, and applies it.
- Control Studio Validate can report command activity, the total movement of the applied command (Σ|Δu|, in percentage points for a % command), as a measured row, or check it against a declared maximum.
- The Validate results table is taller, so a complete results list fits without an inner scroll bar.
- Lesson 21 (tuning comparison) validates the manual, model-based and relay PI candidates on the same heater in Control Studio, with the same setpoint and load trials, settling within ±0.1 °C and command activity, and selects by the declared rule.
- A project step test records the hold the learner applied. When a loop names its hold parameter (`experiment.holdParameter`), Review project test reads the applied value and plans the baseline, the whole hold and one restoration sample (35.1 s for a 30 s hold) instead of a fixed maximum.
- Control Studio Tune offers **Setpoint weight (β)** beside the PI and PID gains when the controller law declares the `Control.PID` setpoint-weight field; the predicted response and the validated and applied parameter record use the entered β, and calculated candidates keep β = 1. Dimensionless settings no longer show a stray "1" as their unit.
- Lesson 22 (setpoint weighting) runs the Lesson 21 controller (Kp 13, Ki 3.5) on the same noisy heater with the real `Control.PID`, compares β = 1, 0.5 and the calculated 0.3 in Control Studio and explains why the integral's growth during the delivery delay still saturates the heater at β = 0.5.
- Control Studio Tune offers **Measurement filter time (Tf)** when the controller law declares the program's measurement filter; the predicted response and loop analysis include the filter lag, Tf is written with the gains, and calculated candidates bypass the filter (Tf = 0).
- Lesson 23 (measurement filtering) filters the heater measurement before the real `Control.PID`, compares Tf = 0, 1 and 2 s on setpoint, load and noise trials in Control Studio (93 % less noise-driven command movement at 1 s; 2 s no longer settles), and starts the filter from the fresh measurement at Auto entry so Apply stays exactly bumpless.
- Lesson 24 (sampling and timing) updates the heater's real `Control.PID` on the measured time between acquisitions, rejects intervals that are not positive or longer than half the loop's 4.46 s ultimate period (holding the command and restarting the filter), and compares normal, 1 s, late and wrongly timed updates under the Lesson 23 load requirements in Control Studio. Its first scan no longer divides by a zero interval when Control Studio's verification starts from preset controller states.
- The Requirements editor limits scenario event times by the validation duration currently entered. It previously used the duration present when the dialog opened and, with none entered, rejected every event after time zero ("Value must be 0.").
- Lesson 25 (controller initialization) replaces the booster-pump example with the heater and its real `Control.PID`: every retained value (outputs, setpoint and measurement references, rates, filter and timing) is initialized from the running heater at startup or once per request, automatic control waits for that initialization, Manual no longer updates the controller, and Control Studio compares an initialized start with one whose stored values are 0 before Apply initializes the controller at the Auto-entry boundary.
- Lesson 26 (bumpless transfer) replaces the booster-pump example with the heater: while Manual commands the heater the real `Control.PID` tracks the applied command, setpoint and measurement (I = u_a − P), and the transfer scan reconstructs the manual command with no integral step. Control Studio shows a 60 % manual hold switched to Auto without a jump (60 % → 60 %, recovery settled in 18.7 s) next to the untracked jump to 87.7 %.
- Lesson 27 (parameter updates) replaces the booster-pump example with the heater: the program adopts a changed parameter record together with its filter time only as a whole and only if it is valid (an 80 %/20 % limit pair or a negative filter time changes nothing), `Control.PID`'s incremental form keeps the command across a gain change (I re-based by P_old − P_new), and Control Studio refuses Kp 150 before validating and applying a Kp 10 / Ki 2.7 set.
- Control Studio's manual Tune message for an out-of-range value keeps the field's symbol as labelled ("Enter proportional gain (Kp) between 0 and 100." instead of "(kp)").
- Lesson 28 (measurement validity) replaces the booster-pump example with the heater: a sensor section supplies the reading with a quality flag and its acquisition age, readings with bad quality, outside 0–80 °C or older than 0.5 s latch a fault that removes heat in the same scan, freezes the controller and filter and clears the Auto request and manual demand, and recovery needs an acknowledgement, three consecutive fresh valid acquisitions and a new Auto request. Control Studio validates an invalid-value and a stale-sensor trial (0 % from detection, recovered within 0.034 °C).
- Lesson 29 (heater commissioning) replaces the four-heater capstone with the complete Lesson 28 heater controller: a capacity check from the energy balance (75, 90, 90 and 105 % holding commands; 37 °C at full power under 30 kW), a commissioning record declared before any result, and one Control Studio trial per requirement (setpoint, load, startup, manual transfer, saturation, capacity, stale sensor) before the Lesson 21–23 controller replaces the introductory PI through one parameter update.
- Lesson 30 (actuator tracking) replaces the software-only rate example with a hot-water valve whose stem moves at 0.5 %/s and publishes its measured position: the program distinguishes requested, bounded and measured position and tracks the measured stem in the integral (I_next = I* + Δt/T_t (x − u_r)). Control Studio compares it with bounded-target tracking (peak 37.03 °C and 25.4 s settling versus 37.74 °C with the request at 100 %).
- Control Studio records a declared actuator's measured position as the applied input and the manipulated variable as the requested output, so Identify fits the plant from what the actuator reached and plots the actual stem ramp, named after the actuator signal, instead of an ideal request step.
- Lesson 31 (valve characteristics) replaces the valve-bench example with the Lesson 30 hot-water valve in two pressure arrangements (regulated 100 kPa and a fixed 128.125 kPa supply with pipe loss 12.5 q²) driven by the same stem: a project-scripted staircase in Control Studio records stem, flows and pressures, and the program keeps only steady points to derive the installed slopes (0.020 versus 0.015 kg/(s·point)) and the heat at stated temperatures.
- Lesson 32 (valve stiction) replaces the flow-loop example with two hot-water valves probed from Control Studio: one ignores request changes below a 1-point command deadband, the other accepts every request but holds its stem until the positioner force F = k_s (u_t − x) exceeds 50 N and stops where it falls to 20 N. The recording shows the unchanged accepted target, the 55 N breakaway at 80.5 %, the stop at 78.5 % and the path-dependent 74 % after a reversal.
- Control Studio's response chart changes an auxiliary signal's line style after every three colours, as the live monitor plot does, so two displayed signals never draw with the same stroke.
- Lesson 33 (position control) replaces the lesson-function torque calculation with `Control.PID` in its P configuration on a simulated rotary axis (J = 1 kg·m², B = 1 N·m·s/rad, ±20 N·m): the program latches the velocity at the first arrival at a new target and removes torque outside ±2 rad or ±10 rad/s. Control Studio identifies the axis from a supplied state-space model, predicts and validates Kp 4 and 16 against the declared peak, late-error and torque limits (overshoot 0.46 and 0.72 rad, arrival at 1.27 and 3.29 rad/s), and Automatic tuning reports that P cannot provide the damping.
- Control Studio's Tune predicts the response of a supplied integrating plant, alone or behind a lag, such as a shaft driven by torque (θ/u = 1/(s(Js + B))), using the exact held-input step.
- Control Studio no longer blocks the editor for several seconds when a controller is tuned again after a validation: the retained draft is not read back and decoded again after each store, and evidence is compared through a single-pass canonical text.
- Control Studio names a failed operation by its page ("Tuning failed: …", "Identification failed: …") instead of its internal action.
- Control Studio's charts label a fitted axis at its regular ticks only, so the padded ends (such as "1.58" above "1.5") no longer crowd them.
- Lesson 34 (PD control) replaces the two lesson-function shafts with the Lesson 33 axis under `Control.PID` in its PD configuration (Kp 16, Kd 7 = 2√(J·Kp) − B, derivative on the measured angle). Control Studio compares P with the program's PD, shows the derivative kick of differentiating the error (366 N·m requested, 0.02 s at the 20 N·m limit), validates and applies the critically damped PD (no overshoot, 1.52 s), and Automatic tuning proposes Kp 1.56, Kd 1, which meets the 4 s requirement with a tenth of the torque.
- Control Studio's Tune offers **Derivative setpoint weight (γ)** for PD and PID when the loop's law declares `derivativeSetpointWeightSymbol`: γ = 0 differentiates the measurement, γ = 1 the error; calculated candidates use γ = 0, and the prediction includes the resulting derivative kick.
- Control Studio retains the workflow draft once after a burst of settings edits instead of for every edit, so typing controller settings after a validation no longer blocks the editor; results and resets are still retained at once.
- Lesson 35 (PID load rejection) replaces the self-scripted trial program with the Lesson 34 axis lifting a 2 kg weight on a 0.1 m drum (τL = 1.962 N·m, a second plant input) under `Control.PID` in its PID configuration (Kp 16, Ki 12, Kd 7, β 0.3) with ±0.001 rad angle noise. Control Studio shows PD holding the load 0.124 rad low, PID holding it within 0.0015 rad, β = 1 overshooting the unloaded step by 0.23 rad and β = 0.3 passing every requirement and being applied.
- Control Studio's Auto-entry check now allows the full controller response to measurement noise, including derivative (Kd·4n/Δt) and integral (Ki·Δt·n) terms, and, when no identification recording measured the noise (for example with a supplied model), measures it with a one-second resting run of the transfer trial's plant.
- Lesson 36 (derivative filtering) replaces the self-scripted two-filter program with the Lesson 35 controller plus a first-order measurement filter on the angle (α = Δt/(Tf + Δt)), so `Control.PID` differentiates the filtered angle and D uses the filtered velocity. Control Studio compares Tf 0 (183 N·m torque activity per step), 0.5 s (0.27 rad overshoot from late braking) and 0.1 s (29 N·m, unchanged step), and applies Tf = 0.1 s.
- Lesson 37 (discrete PID) replaces the hand-written positional controller that drove the axis with `Control.PID` (Kp 16, Ki 4, Kd 7, the Lesson 36 filter) and records the positional reading of every update: error, P, trial integral, D, request and the retained integral. A project experiment in Control Studio couples the drum load and steps the target to 1.5 rad; the recorded updates, including the clamp at 20 N·m with tracking, follow the discrete order exactly.
- Lesson 38 (positional and incremental PID) replaces the positioning-table program, whose incremental path was a P-only shadow calculation, with the Lesson 37 axis under `Control.PID` (incremental with Ki > 0) and derives the increment Δu = Kp·Δe + Ki·e·Δt − Kd·Δω_f + γ from two consecutive positional requests. From the same inputs the program runs a positional form with its tracking, Manual (I = u_a − P − D) and gain-revision rules, a second `Control.PID` with Tt = Δt that keeps the applied torque, and the positional form without the gain rule. A project experiment in Control Studio clamps the Auto-entry request at 20 N·m and revises Kp from 16 to 24: the positional torque equals `Control.PID`'s at every update, while keeping the applied torque leaves a constant 4.49 N·m difference after the clamp and skipping the gain rule a 0.72 N·m one.
- Control Studio's response plot on Identify, Tune and Validate shows hovered values rounded like its other charts (1.9 instead of 1.9017502305516345) and keeps the tooltip inside the plot.
- Lesson 39 (servo commissioning) replaces the flexible indexing-table capstone with the drum axis of Lessons 33–38 and one integrated program: encoder validity with a latched fault (zero torque, then acknowledgement, three fresh readings and a new Auto request), whole-set parameter updates, one torque writer, initialization, bumpless transfer and the filtered `Control.PID`. The filter now runs in every mode, because restarting it at the Auto entry made the incremental D brake a falling load the wrong way (−5.5 N·m) after a fault. Control Studio validates the Lesson 36 candidate against declared positioning, load, torque-limit, startup, transfer and measurement-fault trials and applies it.
- Control Studio runs a loop's declared verification scenario on the project's plant even when its id matches a generated model case such as `load-rejection`; such a trial previously ran silently on the supplied model, without the plant's other inputs.
- The debug-session simulator that Control Studio runs keeps its cycles on the task-period grid: a late wake-up no longer delays every later cycle, and a stall longer than one period is skipped rather than replayed. Simulated time previously fell about 2.5 % behind wall time even when every cycle fitted its period, so Apply on a 20 ms loop failed with "The recorded response does not cover the reviewed duration".
- Lesson 40 (volume balance) replaces the net-flow program, which wrote the balance into a single-input integrator, with a tank plant that has separate inlet and outlet pump inputs (A = 0.5 m², ±0.0005 m level noise) and a model level the program advances with the balance from the measured level. A Control Studio project experiment raises the inflow to 0.012 m³/s for 10 s: the level reaches 0.2896 m against the model's 0.2902 m and then holds 0.29 m with equal flows.
- Control Studio draws the applied output in its response plots as steps, held from one update to the next, instead of joining samples with ramps that misplace a command step by a whole update.
- Lesson 41 (gravity outflow) replaces the program that calculated the outflow from the measured level with two models advanced from the applied inflow: the gravity balance A·dh/dt = q_in − c·√h, which stops at a negative level instead of repairing it, and Lesson 40's constant outflow. In a 200 s Control Studio experiment at 0.011 m³/s the tank settles at the predicted (0.011/0.020)² = 0.3025 m, the gravity model follows within 0.0009 m, and the constant-outflow model ends 0.35 m too high.
- Lesson 42 (linearization) replaces the program that simulated its own tangent against unused tanks with two identical gravity tanks stepped to 0.011 and 0.015 m³/s from the same state, each with a tangent model built at 0.25 m (g = 0.020 m²/s, K = 50 s/m², τ = 25 s). The Control Studio recording shows the small step within 0.0028 m of its tangent and the large step's error growing to 0.061 m, in line with the exact equilibrium error v²/c².
- Lesson 43 (gain scheduling) replaces the hand-written scheduled tank controller with a `LevelController` block around `Control.PID` (Kp = A/λ, Ki fixed at the 0.25 m design level or scheduled as c/(2λ√h) from the measured level; the retained request keeps the holding flow when Ki changes). Six tanks compare fixed and scheduled gains at 0.09, 0.25 and 0.49 m in Control Studio: the fixed loop needs 48 s at 0.09 m against the 30 s requirement, the scheduled loop 23–24 s at every level.
- Lesson 44 (Laplace transforms) replaces the gravity-tank program, which did not match the lesson's heater, with the heater under `Control.PID` P control (Kp 5) and two predictions of a 35 → 36 °C step: the transfer function H(s) = 0.2·Kp/(5s + 1 + 0.2·Kp) without the delivery delay, and the same loop recomputed with the 1 s delay. In Control Studio the heater settles at the predicted 35.50 °C with 77.50 %; the delay-free rise is off by up to 0.157 °C, while the recomputed loop follows within 0.021 °C.
- Lesson 45 (poles and zeros) replaces four abstract response models with the lesson's heater and shaft: the heater returns from 36 to 35 °C in open loop and the shaft steps to 1 rad under `Control.PID` P control (Kp 4). The program calculates the poles from the physical values (−0.2 s⁻¹; −0.5 ± j1.936 s⁻¹) and writes their predicted responses. In Control Studio the water follows its pole within the ±0.02 °C noise and the shaft swings every 3.24 s as predicted; its 0.016 rad larger swings match the pole shift of the 20 ms torque hold (σ = −0.48).
- Lesson 46 (characteristic equations) replaces the crane stability project, which belonged to Lesson 48, with the shaft under `Control.PID` PD control (Kp 16, Kd 7) knocked by a 10 N·m, 0.1 s external torque. The program forms s² + 8s + 16 = (s + 4)² and, for the sampled law (P 10 ms and D 20 ms late), 0.86s² + 7.84s + 16 = 0, and predicts the motion after the knock from both. In Control Studio the shaft returns without passing its target; (s + 4)² is up to 0.0096 rad off, the sampled-law roots −3.08 and −6.03 s⁻¹ follow within 0.0012 rad. The heater's PI loop with its integral is worked as a calculation with both modes.
- Control Studio's root locus on **Tune** draws the pole paths across the varied gain's declared range, labels the inspected gains, marks the selected candidate and names the fixed parameters. It previously showed only unconnected points for the inspected gains, with the selected value in the caption.
- Lesson 47 (root locus) declares the shaft with its braking connected, G₇(s) = 1/[s(s + 8)], with Kp varied and Kd = 7 fixed, and runs the chosen gain and an alternative on two identical shafts. In Control Studio Kp = 16 settles within ±0.02 rad in 1.46 s, Kp = 12 needs 2.16 s and Kp = 20 settles in 1.04 s with its first torque at the drive's 20 N·m limit.
- Control Studio's Routh–Hurwitz result names the method that completed a table with a row of zeros (the auxiliary polynomial, not ε), and its calculation details list the first-column signs of the table they come from together with every special case, such as the origin pole and the replaced row.
- Lesson 48 (Routh–Hurwitz stability) is shortened around the specified cases Kp = 1, 0 and −1: the table, the interval Kp > 0 with the excluded boundary, and the three recorded shaft trials.
- Lesson 49 (periodic excitation) replaces the flexible-drive multisine, which did not match the lesson's heater, with three copies of the heater driven by `PeriodicExcitation`: u = 75 + 5·sin(ω·t) % for five periods at 0.1, 0.2 and 0.4 rad/s. In Control Studio the first cycle carries the start (mean up to 35.15 °C), and cycles 3–5 repeat about 35.000 °C with half swings of 0.91, 0.72 and 0.46 °C.
- Control Studio's measured frequency response works on a completed project experiment: the recording is offered to the analysis, a first scan without a cycle time no longer disqualifies it, the fit per segment is exact for a sine about any operating point (it read 0.46 instead of 0.14 °C/point before), and leftover samples no longer reject the record. Model Bode plots keep the phase continuous instead of wrapping it past ±180°.
- Lesson 50 (frequency response) replaces the flexible-drive project with the heater driven by Lesson 49's sine at 0.2 rad/s and a supplied heater model. Control Studio measures M = 0.1416 °C/point and φ = −57.07° against the model's 0.1414 and −56.46°.
- Control Studio's Loop response sensitivity view draws S, T and CS with their own theme colours and strokes; CS was a dim default colour on dark themes.
- Lesson 51 (sensitivity functions) replaces the precision-drive project with two PI-controlled heaters (Kp 5, Ki 1): one receives a 10 kW sinusoidal heat load, the other 0.05 °C of sinusoidal measurement noise in its reading. In Control Studio the load moves the water by 0.559 °C and the noise moves it by 0.040 °C and the command by 0.28 %, as |S| = |T| = 0.790 and |CS| = 5.59 at 0.2 rad/s predict.
- Lesson 52 (loop shaping) replaces the single-trial heater program with nine heaters: the PI with the lead element (1 + Tz·s)/(1 + 2s) for Tz = 2 (F = 1), 3 and 10 s, each on a target step, a 30 kW load and 0.05 °C of noise, started together by Control Studio. The lesson calculates the crossover, phase margin and noise-to-command gain of the complete loop. In Control Studio only Tz = 3 s meets every requirement: the baseline lets the load pull the water to 33.75 °C, and Tz = 10 s moves the command by 1.41 % under the noise and saturates on the step.
- Control Studio's Nyquist view states the count as the criterion does: P open-loop right-half-plane poles, N clockwise encirclements of −1 and Z = P + N. It previously printed a counterclockwise count without P or Z. The plot keeps equal units on both axes, so circles stay round and distances to −1 read true. Arrows show the direction of both frequency branches, and −1 is marked.
- Lesson 53 (Nyquist stability) replaces the conveyor-oven program, which did not match its heater text, with two heaters under `Control.PID` P control (Kp 10 %/°C about 75 %) with 1 s and 8 s delivery delays. The lesson derives Z = P + N from the angle of 1 + L and calculates the critical delay of 6.05 s. In Control Studio the 1 s loop counts N = 0 and settles; the 8 s loop counts N = 2, Z = 2, and its recorded oscillation grows 1.44 times per 22.6 s period.
- Control Studio's pole-zero map keeps equal units on both axes, like the Nyquist view: the unit circle of a sampled model stays a circle, and a pole's distance from it reads true.
- Lesson 54 (sampled-data stability) replaces the positioning-table program, which did not match its shaft text, with two shafts under the same PD (Kp 16, Kd 7) with one update of computation delay, updated every 0.02 s and every 0.6 s, and simulated in 10 ms steps for both. The lesson derives the exact held-torque update and the characteristic polynomial in z. Control Studio's pole maps of the two sampled loops show all poles inside the unit circle for 0.02 s and a pair at |z| = 2.41 for 0.6 s. The recording shows the 0.02 s shaft settling at the 0.01 rad target without overshoot and the 0.6 s shaft growing until the abort at 2.82 s.
- Lesson 55 (feedforward control) replaces the conveyor-oven program, which did not match its heater text, with three PI heaters (`Control.PID`, Kp 5, Ki 1) under the same 30 kW extraction. The first runs PI alone; the second adds the feedforward D_m/2 through `Control.PID`'s `FeedForward` input; the third takes it from a 36 kW estimate. The lesson derives the ideal feedforward −G_D/G_u = 0.5·e^(+s), which is not realizable, and the unavoidable 0.5438 °C drop. In Control Studio both feedforward heaters fall only to 34.456 °C, against 33.75 °C for PI alone; with the wrong estimate PI trims its part to 72 %.
- Lesson 56 (cascade control) replaces the reactor program, which did not match its jacketed-vessel text, with five jacketed vessels. A single-loop PI and a cascade each run the 35 → 37 °C setpoint step and a 30 kW jacket loss, and one cascade runs its inner loop alone. The outer `Control.PID` outputs the jacket setpoint in °C and the inner one the heater in %, with model-based gains 0.5/0.12 and 5/10 against the single loop's 2.5/0.1. In Control Studio both pass the setpoint test. On the jacket loss, the cascade keeps the vessel above 34.80 °C while the single loop falls to 31.74 °C. The inner loop reaches 63.2 % in 0.5 s, against 9.7 s for the vessel.
- Lesson 57 (ratio control) replaces the blending-skid program, which did not match its dosing text, with two calibrated additive pumps under ratio stations (R = 0.1, `Control.PID` PI 10/10 from the model-based rule). One carrier steps from 100 to 150 L/min, the other stops. Below 5 L/min of carrier the station commands 0 % in the same update, evaluates no ratio and sums the tail volume. In Control Studio the ratio is back within ±0.005 after 0.9 s, and the stopped pump still delivers 0.175 L, below the 0.20 L limit.
- Lesson 58 (split-range control) replaces the reactor program, which did not match its text, with three jacketed vessels from Lesson 56 fitted with a cooler. The split-range cascade's inner `Control.PID` requests net heat, limited to −100…200 kW, which is then split into heater Q/2 and cooler −Q, never both. With 60 kW entering the jacket, Control Studio shows the split range handing over to the cooler in one update and holding 35.40 °C at 30 % cooler, while the heater-only cascade lets the vessel rise to 48.3 °C.
- Lesson 59 (override control) replaces its reactor program with three heater-only jacketed vessels under a 42 °C jacket limit: the vessel PI alone, and an override with and without tracking. The override low-selects between a vessel PI and a jacket-limit PI (setpoint 41.8 °C), and the controller not selected runs in `Control.PID` Track mode. In Control Studio only the tracked override keeps the jacket at or below 42 °C (41.83 °C). It holds the vessel at the feasible 38.16 °C for an infeasible 40 °C request, and gives the heater back in the same update when 37 °C is requested. Without tracking the jacket reaches 43.17 °C and the vessel stays stuck for 19.8 s.
- Lesson 60 (Smith prediction) replaces its program with four heaters. A fast PI (λ = 1 s) runs on the measurement and inside a `SmithPi` predictor fed with its delay-free and delayed heater models; the same predictor and the slow PI then run on heaters whose delay is 1.5 s. In Control Studio, with the matching model the predictor's residual stays below 5·10⁻¹⁰ K: the step reaches 63 % at 2.0 s with no overshoot, against 56 % overshoot on the measurement. It rejects a 10 kW load no better. With the 1.5 s heater it overshoots by 17 %, while the slow PI does not.
- Lesson 61 (vessel commissioning) replaces its single-vessel trial selector with five jacketed vessels and one `ReactorTrain` block per vessel. The block owns the heater, tracks with the controllers not in charge, keeps the transfer update as a tracking update, and latches zero heat on an invalid reading. The heat balance rules out cooling, and no jacket limit is specified, so the lesson compares the single loop and the cascade on the same requirements. In Control Studio both meet the 35 → 37 °C step; on the 30 kW jacket loss the single loop falls to 31.74 °C, while the cascade holds 34.80 °C. The recommended cascade then passes the lifecycle case: 15.000 % on the transfer update and 15.144 % on the next, and 0 % in the update of a 0 °C sensor reading, still latched after the reading recovers. Lessons 53–61 also replace the lesson metadata's baseline and change summaries left over from the programs these lessons replaced.
- Lesson 62 (multivariable process paths) replaces its distillation-column program with the README's two vessels coupled through an 8 kW/°C wall. One recording steps heater 1 and then heater 2 with the other fixed, and `CoupledVesselModel` advances the supplied balances from the old temperatures. Control Studio's **Measured MIMO paths** reads the four paths from settled windows: gains 0.1380, 0.0607, 0.0611 and 0.1376 °C per point against K's 0.1385 and 0.0615, all rises within 0.005 °C of the prediction.
- Control Studio's **Measured MIMO paths** summary shows channel units in the course notation (°C instead of degC), and the pairing line no longer touches the **Static decoupler** disclosure.
- Lesson 63 (relative gain array) replaces its distillation-column program with two pairs of the coupled vessels: isolated steps on one, and on the other the same steps with the other heater corrected by −(4/9)·5 points so that its vessel ends where it started. In Control Studio the compensated gain is 0.1109 against the isolated 0.1380 °C/point, a ratio of 1.245 against 81/65 = 1.246, and **Measured MIMO paths** gives the array 1.2426/−0.2426: the diagonal pairing is chosen. The fixed correction holds the other vessel only at equilibrium; it dips to 34.906 °C at 3.0 s.
- Control Studio's **Measured MIMO paths** shows each matrix with one precision, so a relative gain row visibly sums to one (1.2426 and −0.2426 instead of 1.24 and −0.2426), and the relative gain array and decoupler tables use the gain matrix's table layout.
- Lesson 64 (decentralized PI) replaces its distillation-column program with four pairs of the coupled vessels, one `Control.PID` PI per vessel, on both target directions and a 10 kW load on each vessel. The gains are Control Studio's declared experiment settings, so each gain set is its own run. With Kp 5 and Ki 1 every changed- and loaded-vessel requirement passes, but the unchanged vessel strays +0.284 °C, beyond ±0.2 °C. The bounded retunes (Kp 6: +0.272 °C; Ki 1.2: +0.288 °C) do not help, so the lesson keeps Kp 5 and Ki 1 for static decoupling.
- Lesson 65 (static decoupling) replaces its distillation-column program with six pairs of the coupled vessels: paired PI on the vessel-1 target and load, and `DecoupledPi` on all four events. `DecoupledPi` uses the same gains through u = 75 + M·v, M = [[1, −4/9], [−4/9, 1]]: virtual integrals start at zero, the bias is added after the map, and physical limiting is tracked through M⁻¹. In Control Studio the map lowers heater 2 to 70.47 % in the first update and keeps the unchanged vessel within −0.152 °C, against +0.284 °C paired, so decoupled PI meets every Lesson 64 requirement. Under the loads it moves the other vessel more, −0.155 against −0.068 °C.
- Lesson 66 (multivariable design selection) replaces its distillation-column program with three sets of the coupled vessels, with walls of 6, 8 and 10 kW/°C. The declared experiment setting Coupling Case selects which set a run exercises. Both architectures run in one block, `TwoVesselPi`, with cross gain 0 for paired PI and 4/9 for Lesson 65's decoupler, frozen for 8 kW/°C. In Control Studio the decoupler passes all 12 of its cases, both directions and both loads at each coupling, with the unchanged vessel at −0.179/−0.152/−0.131 °C. Paired PI fails the unchanged-vessel limit in all six target cases (+0.233/+0.284/+0.328 °C), so the decoupler is chosen for the tested couplings.
- Lesson 67 (state-space models) replaces its single-cascade program with two copies of the two-tank gravity cascade and `CascadeModel`, the README's local model advanced by one explicit one-second step from both old states. The lesson first calculates the supplied model's **Initial-state response** in Control Studio: from tank 1 at 0.27 m, tank 2 rises on the stored water to 0.007358 m above rest at 25 s. It then records both tanks through a ±0.001 m³/s pump change, with both model levels within 0.00057 m of the tanks while the pump is changed. A level outside 0.05–0.45 m or a clock interval outside 1 ± 0.05 s latches zero flow.
- Control Studio's **Initial-state response** draws **Total** as the wide process-value line, with **Zero-state input** and **Initial condition** over it as dashed and dotted auxiliary lines, so a part that equals the total stays visible; before, all three used the chart's default colours and the total was faint. Its calculation table shows each column in one notation (0.000769 instead of 7.69e-4 above 0.001477).
- Lesson 68 (controllability) replaces its program with Lesson 67's cascade and model, steered by a pulse pair designed from the model: 0.011 m³/s for 20 s, then 0.009 m³/s for the 11 s after which the model returns tank 1 to rest. Control Studio's **Controllability and observability** shows rank 2 of 2 with [B, AB] = [2, −0.08; 0, 0.08]. In the recording, tank 1 is back at 0.2501 m with tank 2 at 0.2603 m at 31 s. Tank 2 then drains, because no constant flow holds unequal levels.
- Control Studio's **Controllability and observability** reports each rank out of the state count. Under **Calculation details** it lists the declared state, input and output order, the matrices A, B, C, D, [B, AB, …] and [C; CA; …] with one precision per matrix, and what the ranks do not establish. Before, the second matrix column appeared in exponent notation (−8.00e-2 for −0.08), and the order and the scope were missing.
- Lesson 69 (observability) replaces its single non-equilibrium cascade with two copies of Lesson 67's cascade, both driven by Lesson 68's pulse pair. Tank 1 is reconstructed from tank 2's readings alone as y + 25·Δy, from exact readings on one copy and on the other from readings 5 mm off in alternate directions. The true tank 1 level is read only to score the reconstruction. In Control Studio the observability rank is 2 of 2 with [C; CA] = [0, 1; 0.04, −0.04]. The exact readings reconstruct tank 1 within 1.3 mm (RMS 0.00047 m); the noisy ones are off by 0.255 m RMS, since each 5 mm error enters through a derivative multiplied by 25 s.
- Lesson 70 (state feedback) replaces its magnetic-bearing project with two copies of the two-tank cascade. It is the first lesson to use Control Studio's state-feedback design end to end. The supplied model declares only the physical cascade; Studio derives the local model with both levels measured. **State-space design** places z = e^(−0.08) and e^(−0.12) and calculates K = [0.0562, 0.0369] m²/s and the reference gain 0.1131. **Validate** runs the actual program against the nonlinear cascade (settling 33 s and 25 s), and **Apply** commissions Auto. After a 0.01 m³ slug into tank 1, tank 2 rises 3.2 mm with feedback against 7.3 mm without, and both levels are within 0.35 mm of rest 50 s later.
- Control Studio's control requirements, and the requirements it proposes automatically, now apply only to outputs with a setpoint. A model that also measures an auxiliary output, such as the upstream tank used by state feedback, no longer receives requirements for it. Before, those requirements made the loop invalid, and **Tune controller** stayed disabled without an explanation.
- Lesson 71 (state observers) replaces its magnetic-bearing project with Lesson 67's cascade and four `TankObserver`s. It uses L = [0.48, 0.28] s⁻¹ for the error poles −0.16 and −0.20 s⁻¹ and the stronger L = [2.52, 0.64] s⁻¹, each fed exact and ±5 mm tank-2 readings, after a slug that the observers do not see. Control Studio checks the supplied error model: **Poles and zeros** shows −0.16 and −0.20 s⁻¹, and **Initial-state response** shows the predicted decay. The taught gain estimates tank 1 within 2 mm from 22 s with exact readings and from 30 s with noisy ones (RMS 1.43 mm, against 255 mm for Lesson 69's differentiation). The stronger gain converges by 12 s but carries 9.2 mm RMS of noise.
- Lesson 72 (Kalman filtering) replaces its weigh-feeder project with Lesson 67's cascade and a two-state Kalman filter of tank 1 from tank 2's reading, which carries 5 mm of pseudo-random noise restarted with each experiment. The filter calculates its gain from the model's and the reading's uncertainty every second and updates its covariance in Joseph form. It also reports the RMS error it claims, next to the actual RMS error scored against the true level. With the true reading uncertainty, tank 1 is estimated within 2 mm from 25 s after the unseen slug, with an RMS error of 0.23 mm against a claimed 0.54 mm. Assuming 0.25 mm raises the gain on tank 1 to 19.5 and gives 1.01 mm while the filter claims 0.34 mm. Assuming 100 mm gives a smooth but late estimate (1.87 mm).
- Control Studio's finite-horizon **Riccati stages**, the Routh table's entries, the pole view's cancelled factor and allowed-level limits now show each list or matrix with one precision. Before, the digits followed each entry's position in its row, so equal values read differently (14.5 beside 14) and small gains appeared in exponent notation (7.84e-2).
- Lesson 73 (LQR control) replaces its suspension project with Lesson 70's cascade under a gain designed by Control Studio's finite-horizon LQR. The design uses 20 steps, Q = diag(400, 400) with the same terminal weight, and R = 40000 or 160000. The program adds the level cost 400(x1² + x2²) and the effort cost 40000(q − 0.010)² of the applied flow for 60 s after a slug, the same scales for both designs, and records the largest level deviation 50–60 s after it. K0 = [0.0784, 0.0245] m²/s keeps tank 2 within +3.0 mm (level 0.489, effort 0.288); four times the effort weight gives [0.0344, 0.0098], +4.4 mm (0.913, 0.100). Both meet the 2 mm late requirement, and each design has the lower cost by its own weights.
- Control Studio's state-space design fields show a draft error only under the field it concerns, such as **Terminal weights**, and clear it once the design reads. Before, one missing entry put the same message under every filled field, and the messages stayed after the design became valid.
- Control Studio writes channel counts in the singular where needed ("1 input, 2 outputs") and shows a dimensionless tuned value without a unit, for example an estimator gain of 0.2393 instead of "0.2393 1".
- Lesson 74 (LQG control) replaces its suspension project with two cascades under the same gains from Control Studio's LQG design. The design supplies a one-second model that measures only tank 2, and gives K = [0.0787, 0.0239] m²/s with a steady Kalman gain L. One cascade measures both levels; the other feeds the gain with its estimate, predicting from the last applied command and correcting with the reading. During the project experiment both read tank 2 with the same 5 mm noise, and after a slug the estimator is not told about, the full-state loop stays within 0.58 mm 50–60 s later. With the model's error alone as process covariance, L = [0.0023, 0.0068]: the estimate misses the slug and the loop is 5.3 mm high. Allowing tank-1 upsets (L = [0.239, 0.106]) brings it to 1.7 mm.
- Lesson 75 (model predictive control) replaces its building-zone project with two of Lesson 13's heaters that share a 100-point supply. Every second the program predicts both heaters three seconds ahead for the 231 command pairs on a 5-point grid within the supply. It rejects plans that leave 26–33 °C, scores squared temperature errors plus a 0.002 move weight, and applies the first move of the cheapest plan; the cost splits into one part per heater, which keeps the search to 42 predictions. Raising heater 1 from 28 to 31 °C, the first plan borrows supply from heater 2 (70/30), and both targets are met within 8 s at 55/40. With 30 kW more taken from heater 2 the targets need 110 points: the plans use the whole supply, 50/50, and both heaters settle 1 °C low. The old test that loaded the building project is retired.
- Lesson 76 (Lyapunov stability) gets a program and a matching plant: an upright arm (J = 1 kg·m², B = 1 N·m·s/rad, G = 4 N·m, ±20 N·m) under τ = −16φ − 7ω, with V = ½Jω² + ½Kpφ² + G(cos φ − 1) and its certificate V ≤ 0.1 J published every 20 ms. Before, the project had no program, a mismatched arm simulation and a library copy. A Control Studio experiment setting chooses the start angle; the experiment holds the arm there with gravity compensation and then releases it. From 0.1 rad the state stays inside the certificate, V never increases, and the torque peaks at 1.6 N·m. From 0.2 rad, outside the proven region, the arm also returns and enters the region after 0.46 s: unproved, not unstable.
- Lesson 77 (feedback linearization) replaces its industrial-joint project with Lesson 76's upright arm under τ = 16(r − φ) − 7ω − Ĝ·sin φ, with the controller's gravity coefficient Ĝ as a Control Studio experiment setting. For a 0.1 rad target, no compensation rests at 0.1332 rad and misses both the 0.12 rad peak and 0.01 rad error requirements. Ĝ = 3.5 N·m rests at 0.1032 rad, and the nominal Ĝ = 4 N·m holds 0.1000 rad with gravity's own −0.399 N·m.
- Lesson 78 (adaptive control) replaces its extruder project with two of Lesson 13's heaters whose heat loss has dropped to 8 kW/°C, each held by the real `Control.PID` (heater B in Manual during the estimation windows). From one-second heat balances the program estimates the heat loss by least squares, Ĥ = Σxy/Σx², over two 30 s windows. It accepts a window only for 7–13 kW/°C, at least 1 °C of temperature span, residuals within 5 kW and agreeing halves, and then sets Ki = Ĥ/(aλ). The steady window is rejected even though it gives 8.0 kW/°C, because it cannot separate the change from an unknown extraction. The step window is accepted and sets Ki to 0.4. On a 3 °C step the revised heater follows the λ = 10 s design (63 % at 9.9 s, no overshoot), while the old gains rise faster and overshoot slightly.
- Lesson 79 (robust control) replaces its flexible-wing project with eighteen of Lesson 13's heaters spanning heat loss 8–12 kW/°C and delivery delay 0.8–1.2 s, each held by its own `Control.PID` in an array of `HeaterCase` blocks. A capacity check shows that holding 38 °C would need 108 % at 12 kW/°C, so the requirements use a 35 → 36 °C step and a 10 kW extraction. The heat loss varies gain and time constant together, and a small-gain bound, W·|T0| ≤ 0.254, proves stability for every heater in the range with Kp = 2.5 and Ki = 0.5. In Control Studio that candidate passes all eighteen cases; Kp = 30 and Ki = 6, tuned for the nominal heater, settles faster there but leaves the 8 and 10 kW/°C heaters with 1.2 s delay oscillating (7 of 9 step cases).
- Lesson 80 (networked control) replaces its pumping-header project with six of Lesson 13's heaters, each under a remote `Control.PID` whose commands travel as packets over a simulated network, with a local `Control.PID` as fallback. The reusable `CommandReceiver` accepts a packet only if it belongs to the current connection, has a newer sequence number and is 0–200 ms old; it holds the last command for at most 300 ms and then hands over to the local PI, which has tracked the applied command. Remote control returns only after an acknowledgement, three fresh packets and a remote command within 0.1 point of the applied one. In Control Studio a dropped, a reordered, a late and an old-session packet are each held or rejected by their own check, an outage switches to local control at 5.4 s, and the acknowledged heater returns to remote control at 8.2 s; the transfers change the command by at most 0.026 points, and all six heaters meet the step requirement.
- Lesson 81 (fault-aware control) replaces its pumping project with six of Lesson 13's heaters, each measured by two temperature sensors and held by a `Control.PID`. The reusable `SensorSelector` accepts a reading only if it is 0–80 °C and at most 0.5 s old, uses sensor A when valid and B otherwise, latches zero heat when two fresh readings differ by more than 0.4 °C for 0.2 s or no reading is valid, and restarts only after three fresh agreeing readings and a new Auto request; each change of sensor re-initializes `Control.PID` with the applied command so the command does not jump. In Control Studio a stale sensor is used until its reading is older than 0.5 s, an out-of-range sensor is replaced at once, a sustained disagreement latches zero heat at 5.2 s while two short ones do not, losing both sensors latches, and the restored heater restarts at 12.0 s from zero with only the integral step.
- Lesson 82 (independent design) replaces its compressed-air project with a matched comparison on twelve of Lesson 13's heaters: PI against PI with feedforward of a measured 10 kW draw-off, each at heat losses of 8, 10 and 12 kW/°C with a setpoint step and a draw-off, against requirements and a decision rule declared before the test. A feasibility check shows that every requirement needs at most 96 %. Both candidates meet the setpoint requirement; only the feedforward candidate keeps the draw-off within 0.3 °C (about 0.18 °C, the fall during the 1 s delivery delay, at every heat loss), so the declared rule recommends it.
- Arrays of function block and class instances work as IEC 61131-3 Table 11 and the truST data-type specification describe: `Timers : ARRAY[1..2] OF TON;` creates one independent instance per element, including each element's nested instances, in programs, function blocks, methods, named array types and globals. `Timers[i](IN := x)` calls an element, `Timers[i].Q` reads it, and restart re-creates every element. The runtime previously refused such declarations with "init failed ...: type mismatch" and could not compile a call through a subscript.
- Instance arrays accept element initializers, `[(PT := T#100ms), (PT := T#300ms)]`, and repetitions written `50((PT := T#100ms))` or `50(PT := T#100ms)`; an array type declared with an initializer gives it to every variable of the type, and a cold restart re-applies it.
- A function-block input may be assigned through its instance before the call, `Delay.IN := Start; Delay();`, as IEC 61131-3 Figure 13 and the statement specification allow; the checker previously rejected it with E301. A function block still may not assign its own inputs.
- A warm restart no longer restores the instance identity of a `RETAIN` global function block, class or instance array: the instances are re-created, as the runtime specification requires, instead of pointing at unrelated instances after the restart.
- The bytecode VM no longer resolves a debug source location at every executed statement when no debugger is attached; with debug control attached it resolves each statement location once instead of rescanning the source file each time. In a release build, Lesson 79's eighteen `Control.PID` loops now take 1.8 ms per scan instead of 2.8 ms.
- With debug control attached, the runtime copies the variable storage when execution stops and once per completed cycle, instead of before every statement. While running, the debug snapshot now shows the last completed cycle, including its last statement; a pause, step or breakpoint still snapshots the exact storage at the stop. With debug control attached, as in VS Code and Control Studio, the same program takes 1.8 ms per scan instead of 27.4 ms, and a function-block call 2.2 µs instead of 7.9 µs.
- The bytecode VM builds an error value only when an operation fails, finds the declared type of a global or instance reference from an index of its storage slot instead of scanning every reference, stores a value that already has the declared elementary type without converting it, computes same-type `BOOL`, `INT`, `DINT`, `LINT`, `REAL` and `LREAL` operations directly, and looks up a function-block field in the instance itself instead of through a string-keyed cache. Results and runtime errors are unchanged. In a release build, Lesson 79 now takes 0.79 ms per scan instead of 1.8 ms, a loop iteration of `X := X + 1` 0.44 µs instead of 0.6 µs, and a function-block call 1.2 µs instead of 1.6 µs, or 1.8 µs instead of 2.2 µs with debug control attached.
- Lesson 14 (identification experiments) uses that recording: its record ends just after the step returns, for any permitted hold. The lesson explains the steady start, step size and hold through the drift, uncertainty and first-order response calculations, with the recorded results.
- Keep chart tick labels distinct when small temperature changes occur around a nonzero operating point.

- Keep rejected independent model predictions inspectable after save/reopen without allowing acceptance. Preserve the model overlay when restoring hidden experiment signal selections.
- Keep Lesson18 on the same native heater used for identification, and use the permitted40s estimation hold for its independent validation handoff. Preserve the lesson's fixed error and timing requirements.


### Changed

- Control Studio's recorded-fit comparison now exposes the existing model-error and uncertainty evidence in collapsed details, retains the recommended structure with one primary action, and restores its model family and measurement settings when the editor reopens. Lessons16–17 use the three-page controls without changing their teaching objectives.
- Control Studio retains project-experiment recordings and signal selections on
  reopen, shows selected live setpoints and diagnostics with stable colours, and
  frames constant measurements without exaggerating numerical roundoff.
- Keep experiment review text compact and use shared numeric controls for declared
  settings. Save native recordings through the response context menu or command palette.
- Allow the identification-experiment lesson's complete 40-second hold after its
  five-second baseline; retain the actual applied-input record for subsequent identification.
- Adapt control-course experiment instructions to the existing three-page
  Control Studio while preserving the taught concepts and declared requirements.
- Tune automatically fills missing test settings from the selected plant model,
  recording and operating limits. Keep declared/manual requirements and pinned
  controller settings; label generated criteria in the optional editor and
  validation results and retain them across reopening.
- Control Studio Identify now performs one live experiment and retains its
  provisional model and measured response. Tune stops with the candidate;
  explicit Validate acquires independent data, checks the frozen model and
  executes the controller campaign before enabling Apply.
- Control Studio Identify records and excites the connected running plant in real time,
  using native experiment restoration and per-scan recording. Reset and reopening
  resume live selected signals. Independent live records retain their own provenance.
- Control Studio: replace the old multi-mode workspace with the agreed Identify,
  Tune and Validate pages, sharing the existing workflow/runtime services.
  Keep exact controller bindings, recorded-data identification, manual/pinned
  settings, channel limits and requirements, and guarded Apply/Restore in that flow.
  Fit explicitly selected recording channels and retain them after reopening;
  identify every declared PID-group member and require a supported complete
  parameter/state transaction before group tuning or application.
  Restore contextual project experiments, manual recorded-data fitting, supplied
  model import, analysis tools and scenario editing within those same pages.
  Retain measured frequency/MIMO results, exact Routh regions, initial-state
  responses and finite-horizon Riccati stages; preserve declared state and
  switching-controller pins during retuning.

### Fixed

- Recognize a rate-limited step's rising increments as part of the same step,
  preserving the existing fitter's estimation of gain, dynamics and delay.

- Identify plant executes a fresh experiment for the chosen method instead of
  silently refitting a previous method's recording. Reset clears the working
  model, chart and dependent results, including the saved draft, while keeping
  configured signals/limits and the running controller unchanged.

- Keep large identified model equations scrollable above the experiment plot;
  explain clipped or rate-limited relay records without reporting missing cycles.

- Show actual identification inputs alongside measured outputs and model
  predictions on aligned time axes. Display returned experiment records before
  fitting completes and retain the matching input trace when reopening results.

- Keep identification on its model and response for review. Identify plant keeps
  its label; Reset chart clears the display without starting identification or
  deleting the model. Use the Tune tab to navigate; ask for missing requirements
  only when tuning is requested.

- Prepare isolated Control Studio bytecode before starting PLC scan deadlines,
  so compilation cannot consume the first scan budget on slower hosts.

- Show selected plant signals and their current readings in Identify before a
  model is acquired. Signal dropdowns apply checkbox choices immediately and
  close on outside click, without Done/Cancel buttons.

- Preserve scenario values when adding or renaming signals.

- Run the model-validation heater/probe through native plant simulation and
  preserve its five-second holdout baseline. Model-based PI tuning now executes
  the library `Control.PID`; source-selected trials run the selected command/load.
- Make the identifiability lesson reuse the saved model comparison and explain
  practical versus structural ambiguity with runnable ST calculations. Its
  heater project reads the noisy probe from native simulation through PLC input.
- Make the model-structure lesson follow the existing Control Studio import,
  comparison and Save/Open controls. Include runnable ST calculations for its
  two-state update and worked prediction, with an explained update-order exercise.
- Import the original first-order estimation run and its settings when a saved
  study also contains later validation data. Expose the additional-lag search
  grid and model-error limits; preserve the original reference
  and unfinished field values across Save/Open.
- Keep identification fitting and saved-study calculations responsive in the
  editor. Cancel retains the recording and confirms the outcome; edits and
  newer Open requests discard obsolete calculations. Preserve current field
  errors when an older settings acknowledgement arrives.
- Fit an additional physical lag on an explicit held-step parameter grid while
  preserving the first-order reference and recording. Keep model-adequacy
  limits separate from measurement-consistent uncertainty, retain ambiguous
  time-constant assignments, and reopen the provisional comparison through the
  existing identification session. The model-structure lesson now reads water
  and probe temperatures from the native plant through ordinary PLC inputs.
- Recognize `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` as test-runner entry
  points in unused-POU analysis, preserving warnings for unused production
  declarations and unused variables inside tests.
- Retain identification previews, measurement settings and both original
  estimation/validation recordings across editor restarts. Recheck accepted
  models on restore. Changing identification inputs clears the selected model
  and dependent tuning state, including the current saved draft. Preserve
  unfinished field edits and imported recordings in the current working draft
  without adding saved history revisions. Rejected saved results leave the
  current result identity and exported contents unchanged.
- Add measured held-step analysis in Identify with qualified baseline/final
  windows, explicit FOPDT search, measurement-consistent parameter ranges and
  grid refinement. Preserve estimation parameters when checking a distinct
  validation step; reject bad, clipped, saturated and inconsistent-unit data.
  Detect sustained onset in either response direction, explain missing input
  steps and require complete baseline and held-step window coverage.
  The lesson gain helper rejects zero input change through ENO without a
  division-by-zero fault; its adaptation and abort instructions match the project.
  Identify names and focuses invalid settings, explains rejected windows and
  points opened recordings to Validate. Short and zoomed editor layouts keep
  settings reachable; narrow plots retain readable reference labels. Zoom and
  Fit clear the previous inspection so its values cannot describe another window.
- Preserve native acquisition identities through recording and saved results,
  so an imported estimation run can use the next native Test for validation.
  Check the selected recording rather than an older Test with the same role.
  Include the five-second baseline in the step-identification lesson's Test.

- Distinguish the last Control Studio Test outcome from the current runtime and
  a newly reviewed plan, while retaining recordings and explicit recovery needs.
  Model and tuning updates preserve the Test-owned status and its accessible label.

- Put the selected course lesson's name and project action before expandable
  technical details. Preserve full teaching text, remove identical repeated
  introductions and retain the PID acronym in module titles.
  Copied lessons also adapt descriptive next-lesson links to the current
  catalogue instead of leaving links to sibling folders that were not copied.
  The step-identification lesson explains EN/ENO without a reference link to
  repository files absent from the copied project.

- Render authored lesson instructions, tables and equations in the example
  gallery. Open referenced bundled files as read-only source previews while
  keeping the selected lesson available in the same editor group.

- Keep native ST assertions out of Simulator source selection in the remaining
  control-course projects while preserving their test bodies and discovery.
  Filtered `trust-dev test` runs now register all test declarations for
  compilation and execute only matching cases, including configured projects.

- Import saved Control Studio recordings for identification while retaining their
  original acquisition identity through Save/Open. Keep source controller and
  design state out of the import, and exclude the step-identification lesson's
  native assertions from Simulator using the existing runtime-ignore marker.
  Show the recording before fitting, keep one primary next action and one
  historical source badge, and return explicit Monitor navigation to live data.

- Accept the documented manual PI Tune-to-Test lesson route when building the
  course catalog, while still rejecting unknown and duplicate stages. Align
  the Routh–Hurwitz lesson title and next-lesson link with the approved course.

- Keep identification experiment settings in Test, lock them during review/run,
  and record the declared baseline before enabling the existing project.
  Preserve newer I/O observations per debug session when delayed events arrive.

- Label paired process signals with their actual names on Control Studio axes,
  including model/measurement and heater/loss comparisons in Energy balance.
  Keep unselected diagnostics in Signals, and allow recorded channel selection
  directly in Validate without leaving the saved or retained response.

- Show applied input and measured output consistently for open-loop experiments,
  without a fictitious setpoint or undeclared controller mode. Validate displays
  the retained native recording before Save/Open while Monitor keeps live data.
  Keep padded temperature-axis endpoints from repeating rounded tick labels.

- Refresh Control Studio observations after a confirmed simulator restart and
  start a fresh live response window while preserving saved recordings. Share
  the coherent signal selection with recording and identification. Reject new
  project-owned Tests while primary signals have forcing or bad quality.

- Keep aborted Test recordings available for save/open/export while identification
  remains disabled for unqualified data. Preserve project-owned activation, stop
  and restoration details, including Boolean applied values, in saved results.

- Give manual controller parameters labelled numeric fields, bounded change
  reviews and observed rollback values. Keep model/search tools and raw JSON
  expandable, and show measured response while tuning without a fitted model.

- Keep recording-only Control Studio validation focused on the measured response,
  save/open/export actions and review readiness. Opening saved revisions replaces
  previous plot evidence and validation status without changing the controller.
- Name isolated diagnostic axes after their selected signal and draw an explicit
  explanation when a saved controller candidate has no stored comparison.
- Keep arithmetic roundoff in constant recorded signals from appearing as a
  large oscillation when Control Studio fits its axes; preserve raw samples.

- Run the heater setpoint/load lesson through the existing Control.PID and one
  native temperature sensor, with separate heater/load delays and applied-output
  feedback. Preserve both experiments and their original response requirements.
- Bind manual PI tuning to the actual Control.PID parameter record and native
  heater inputs. Preserve one-gain changes, accepted values and independent
  target/load trials across explicit restart and reapplication.
- Reuse that PID and native heater in Loop Diagnosis, with separate measured
  and reference inputs, a native sensor-bias experiment and aligned request/
  applied-output diagnostics.
- Omit a repeated controller type from Control Studio setup choices when it
  duplicates the instance label; preserve compiled identities and custom choices.

- Align live Control Studio event markers with actual time and plot boundaries,
  including zoom and resize; show source, time and meaning on hover. Use
  Ready to apply / Ready to restore for the corresponding open change review.

- Add Back and four named phases to native Control Studio setup. Preserve
  compatible answers, revalidate changed choices, and prevent stale asynchronous
  validation from accepting an edited value.

- Hide Control Studio figure tools on Signals and Events. Expose the Response
  sample table with selected channels, engineering units and individual quality.
- Keep live time-axis labels separated near the latest observation without
  trimming the recorded response.

- Keep the existing Abort test action visible across Control Studio modes,
  Signals and plot expansion until the native experiment stops.
- Show the active Test recording and progress consistently; prevent manual
  recording controls and late notices from taking over its acquisition.
- Simplify lesson-owned Test review to duration and a compact exact plan;
  omit fixed controls, unused excitation seeds and internal Boolean units.
  Remove unsupported Automatic Tune actions and collapse empty header space.

- Keep the Signals list on the opened recording's values, units and quality;
  preserve offline readings and return to current values when live observation resumes.

- Draw selected controller diagnostics in live and reopened Response plots,
  preserve their samples and quality in exports, and separate incompatible
  units into aligned lanes. Restore the visible signal checkboxes with saved
  selection and show requests above the physical actuator limit without clipping.
- Keep multi-unit chart labels readable in compact Test views, contain long
  review values, and draw theme-derived measurement strokes at full opacity.
- Label controller-state axes by their actual role and keep diagnostic trace
  colours/styles distinct from primary SP/PV/MV when selecting signals.

- Run the heater P and PI lessons through the actual Control.PID block and keep
  the library quick-start example's history between scans. Add a read-only
  state snapshot for diagnostics without changing the controller equations.
- Use Test's automatic recording in the heater lesson instructions and explain
  how to save and reopen the measured response.
- Keep the manual Control Studio response, compact live values and Test form
  from overlapping; show a saved recording without requiring a tuned candidate.
- Give reviewed tests one confirmation action and a return to editing, keep
  pending starts locked, and restore controls for the next experiment.
- Preserve imported-run identity labels when reopening an evaluation and keep
  optional run-import fields behind an expandable control.
- Expose independent state-space input delays in the plant editor, preserving
  channel values and enforcing the native history bound before Save.
- Keep identified-model predictions bound to their model and output when
  guided setup assigns explicit channel names.
- Reject applied-actuator feedback mappings that overlap coupling or physics
  outputs, and run the windup lessons through the real PID and actuator readback.
- Keep independent model-fit evidence attached to its selected model, retain
  completed Tune/Verify results on screen, and use consistent response colors
  and compact parameter comparisons throughout Control Studio.

- Map the actual `Control.PID` seven-state initialization through the shared Control Studio verification and commissioning workflow.

- Show builtin tuning controls only for adapters that use them and keep hidden
  gain fields from blocking project-owned tuning.
- Disable empty Monitor plot actions until samples arrive while preserving
  inspection of retained samples after disconnection.
- Hide controller removal when no controller is attached and clear its previous
  file selection on removal.
- Let engineers close an unapplied controller review with Close or Escape
  without applying or discarding the candidate.
- Honor a specific proportional gain when P is the only available adapter,
  including its declared parameter bounds, under automatic family selection.
- Preserve a newly selected Control Studio stage when a delayed opening-state
  response arrives.
- Keep empty advanced-workspace charts inside their allocated space so model
  import and validation controls remain visible.
- Keep supplied-model analysis disabled after a rejected import when no valid
  model has been selected.
- Bind declared-project controller tuning to the runtime's project identity,
  avoiding a false program-change rejection on the first tuning attempt.
- Keep unconfirmed Manual transactions on the recovery route, lock dependent
  inputs, and unlock a new study after reviewed runtime replacement.
- Navigate directly to the earliest missing workflow prerequisite.
- Keep supported controller predictions visible beyond the initial frequency
  range, use the task's physical coordinates and sampling period, and remove
  the extra prediction delay and invented current-controller baseline.
- Show one verification retry action when a failed run has no response samples.
- Connect pinned controller comparisons to their cached prediction plots, retain
  unavailable pins across context changes, and support keyboard design history.
- Preserve commissioning recording channel roles so SP/PV/MV curves render with
  their original units even when the recorded requirements only contain output bounds.
- Keep automatic model previews separate from saved analysis studies and format
  displayed search scores without meaningless floating-point digits.
- Control Studio replaces stale model plots on import, keeps unvalidated models
  inspectable without enabling Tune, and uses the actual engineering units in
  selected-sample announcements.
- Guided tuning distinguishes Automatic rule selection from explicit SIMC/AMIGO
  choices and hides response preferences for rules that do not use them.
- Manual/imported PID inputs report validation errors before submission; plot
  exports use the native Save dialog and report the actual saved filename or
  cancellation. Reviewed numerical settings use shared engineering precision.

- Validate the final part of an identified model's independent response as well
  as its average error, preventing an integrating approximation from hiding
  a first-order process's final-response mismatch.

- Reject blank required Control Studio tuning goals and gains, and show manual
  tuning errors instead of leaving the candidate in a generating state.

- Remove duplicate headings and introductory copy from Control Studio; keep manual tools and scenario editing inside Settings.


### Fixed

- Classical tuning accepts the exact held-input equivalent of supported scalar
  sampled models while actual-ST verification retains the original discrete plant.
- Relay qualification excludes recording startup and incomplete initial periods;
  switching between loops keeps each loop's task and draft in its own session.
- Updated the locked `js-yaml` dependency to 4.3.2 to address its dependency audit finding.

- Control Studio keeps supplied-model imports in the main workflow, shows task
  requirements before tuning, and preserves one primary next action after edits
  and reopen. High-contrast primary buttons retain a visible outline.
- Recorded response plots resolve SP/PV/MV by requirement channel identity, keeping
  labels, units and output limits correct when observations are reordered.
- Auto commissioning retains native recordings and completed checkpoints on
  close. Matching saved receipts restore their plots without restoring Apply
  approval; lost acknowledgements reconcile the original parameter operation.


- Control Studio shares relay transitions and transfer-function realization
  between existing and new paths, and centralizes P/PID adapter operations.
  Explicit state, ON/OFF and project-owned controller mappings now use shared
  tuning, initialization and commissioning paths. Binary identification preserves
  actuator output levels and minimum dwell; switching commissioning evaluates
  observed levels and completed dwell intervals. Native tuning retains its runs
  and stops if the loaded program changes. Guided ON/OFF setup maps the existing
  mode, parameters and retained states from compiled symbols. Isolated operations
  save their identity before Start and restore it on reopen; Manual application
  recovery compares the exact reviewed record and revision history.
  The connected Main workflow now retains editable drafts automatically,
  restores recovery ownership before permitting new operations, and includes
  declared noise, load, interaction, switching and state-recovery scenarios.
  Model-derived settings check coordinate units; verification respects model
  validity ranges as well as equipment limits.


- Added exact delayed-integrator simulation for selected-model actual-ST verification, with bounded input history and reset support.


- Control Studio connects integrating models to the shared P/PI design paths and considers PD candidates using their calculated steady-error limits.


- Control Studio rejects nonrepeatable relay amplitudes and periods before offering tuning settings.
- Control Studio supports explicitly choosing P on a typed PID controller, zeroing I/D gains through verification and commissioning.

### Added

- Control Studio now checks supplied-model validation evidence and bound channel
  units before controller recommendation, selects PI when P cannot meet the
  declared gain bounds, rejects initial commands beyond the actuator range, and
  returns failures through the existing Find model, Tune, controller-choice or
  requirements controls. It also preserves declared pulse identification and
  uses a held-out multisine or PRBS frequency fit when requested. Explicit relay selection uses the feedback-relay acquisition route.

- Control Studio can commission a declared first-order PI controller in Auto
  through a typed retained-state adapter. It verifies the actual ST baseline,
  candidate and first-output continuity case, aligns integral and derivative
  history at the runtime task boundary, observes the applied response, and
  returns to Manual with the prior parameter record on rollback.

- Control Studio's automatic recommendation now lets a selected proportional
  controller continue through its own P tracking and Auto commissioning path
  when the declared loop has no PID retained-state adapter. PI and PID Auto
  commissioning still require their declared state-initialization adapter.

- Control Studio Test now offers automatic bounded excitation, derives a safe
  reviewed step from the live loop mode and command headroom, and carries
  independent Test-record identities through model validation. Manual-output
  tests can write a declared command address/type while retaining the actual
  MV/PV response for fitting.

- Control Studio Relay Test now qualifies the runtime's measured feedback
  cycles, then offers one explicit calculation of an unapplied PI proposal.
  It never substitutes a timed square wave, accepts gains from the webview, or
  applies a relay result automatically.

- Runtime controller transfers apply mode and state coherently at the owning
  task boundary, track fresh P-controller inputs, and retain observed adoption,
  cancellation and failure outcomes. Control Studio connects this to reviewed
  local-simulator Auto commissioning and guarded Manual rollback. Recorded
  responses, manual transfer tolerance and fresh recovery reviews survive saved
  studies; completed unconfirmed recovery is retained when its editor closes.

- Control Studio records declared symbolic controller modes with program identity
  and per-channel quality, protects active verification from model replacement,
  and clears incompatible manual gain pins when identification changes the model.

- Isolated Control Studio verification retains received sample prefixes after
  cancellation or wall-budget termination, with bounded capture and explicit
  failure status; cancelled records remain available to inspect, save and reopen.

- Connected Control Studio P workflows show current model, tuning and verification
  stage status, clear historical qualification on reopening, and use one Tune action.

- Control Studio carries declared and measured operating points into selected-model
  simulation, proportional tuning and matched actual-ST verification without
  adding controller bias. Automatic simulation identification reads the actual
  Manual operating command and checks its retained pre-excitation baseline.

- Control Studio reads a declared symbolic controller mode together with process
  signals from one runtime scan, and clears unavailable mode values.
- Opening a saved study during active work is rejected before replacing state;
  fresh verification resumes live readings and shows the unapplied candidate.

- Guided setup binds an existing proportional ST controller without invented
  plant coefficients, for model identification followed by tuning and verification.

- Control Studio retains completed identification trials when qualification or a
  later trial fails, and preserves earlier campaign evidence across retries.
- Failed Control Studio identification studies can be saved and reopened with
  their raw trial evidence; reopening does not authorize controller application.

- Control Studio uses the selected compatible supplied model for Routh analysis,
  and controller comparison prefers simpler sufficient designs over additional performance.

- Control Studio now retains declared unlimited-request and integral-state signals
  beside applied output for a completed saturation-return Test. It reports the
  actual clamp, integral change and release delay, then recommends anti-windup
  without resetting controller state or claiming additional actuator capacity.

- After a qualified saturation-return record, Control Studio can review one
  typed back-calculation candidate for the declared simulation loop, stage its
  complete parameter record, and restore the exact untracked baseline. Matched
  tracked-versus-untracked Test comparison remains in progress.

- Control Studio Routh–Hurwitz analysis displays declared characteristic
  polynomials, table calculations and analytical intervals for supported
  first- and second-order models, with explicit special-case limitations.

- Control Studio configures existing projects from compiled signal and controller
  metadata, discovers multiple loops, and rejects duplicate loop identities.
- Project-owned experiments can open every Control Studio workflow stage.
- Provided transfer-function models support offline named-parameter root-locus
  calculation, candidate selection and saved-result restoration, with explicit
  model provenance and approximation labels. New results use schema v4 while
  legacy v3 records remain readable.

### Fixed

- Closing Control Studio during a standard Manual parameter apply or rollback
  now preserves only the native transaction identity and rechecks the matching
  runtime status after reopening. It never replays the write or retains a
  closed review token or client.

- Control Studio manual recording now retains the original recording owner when
  Start acknowledgement is lost, retries Stop on that owner, preserves returned
  runtime samples, and labels unconfirmed captures as unqualified evidence.


- Start Lesson 1's door disturbance with its experiment, so reading the
  instructions while the simulator runs no longer consumes the scenario.

- Begin targeted repairs to the 82-lesson control curriculum, preserving the
  P-to-PI progression and separating prose, executable-project and rendered
  workflow verification. Individual lesson acceptance remains in progress.
- Control Studio can now start and safely restore project-scripted curriculum
  experiments through one typed Boolean lesson authority without injecting a
  duplicate process excitation.
- Add lesson-specific Control Studio instructions and process-image mapping
  repairs. Their individual execution and rendered acceptance are tracked
  separately; this is not a claim that the entire course is verified.

Target release: `v0.24.65`

### Added

- Show full objectives, expected observations and lesson-specific previews in
  the example gallery, and bundle first-action guidance with all six non-control
  starters. Preserve created projects when VS Code cannot switch folders and
  offer an explicit retry without copying or generating their files again.
- Replace the example gallery's filter-chip wall with compact hardware/category
  selectors and a result count. Isolate damaged lesson metadata and show its
  reason without offering a misleading Create action.
- Preserve learner-selected actuator and sensor layouts in the controllability
  and observability lessons, so their alternate comparison cases can actually
  run. Keep safe zero-flow restoration independent of those selections.
- Retain the state-feedback and observer lesson comparison controls instead of
  forcing the preferred controller path every scan; document separate fair runs
  and preserve existing safe current/torque restoration.
- Split optional plant settings into named actuator, signal conversion, realism
  and scenario tasks, with visible summaries that include disabled configured
  effects. Opening one task no longer exposes every engineering field at once.
- Show scenario event times in seconds with exact millisecond persistence;
  retain invalid edits and block Save instead of rounding or treating blanks
  as zero. Distinguish runnable simulator defaults from optional physical
  hardware on the six starter example cards.
- Return focus to example search after clearing an empty result, so the removed
  reset button does not leave keyboard focus lost.
- Reopen saved plant models that omit default starting values. Show their
  model-sized zero state without substituting a teaching template, and preserve
  the original omission when only the name changes.
- Show effective zero defaults for optional robot-joint friction and gravity
  parameters while preserving omitted values on save.
- Retain created lesson-project locations and offer Open existing project,
  without treating project creation as lesson completion.
- Label editor-polled Control Studio recordings as incomplete, retain observed
  scan gaps and dropped samples, and block identification from these partial
  recordings. Do not report editor refresh intervals as actual PLC cycle timing.

- Add an explicit optional temperature teaching example after selecting the
  first-order-with-delay plant model, with percent/Celsius assumptions and
  reversible physical parameter wording. Keep generic imported models generic.

- Add deterministic simulation plant models for controller verification,
  including static, first-order, FOPDT, integrator, gravity-tank, robot-joint,
  second-order, state-space, and piecewise-linear processes; scalar and MIMO
  typed process-image bindings; enforced local-model validity bounds;
  actuator nonidealities; seeded uncertainty and dropout faults; scheduled
  plant events; bounded status/trace capture; atomic feedback publication;
  failed-substep rollback; physical-I/O activation protection; and a native
  closed-loop `Control.PID` performance oracle. Saved simulated-plant bricks
  now use the canvas' standard confirmed Remove interaction and atomically
  remove only the selected plant and its owned events from `simulation.toml`.
  The VS Code workflow now has documentation-ready Plant Simulation and
  Control Studio journeys covering configuration, monitoring, bounded tests,
  identification, tuning, validation, Automatic Tune, recovery, and explicit
  physical-plant approval boundaries.
- Add a runtime-bound heater step-identification lesson that teaches the real
  Control Studio Monitor, Test, Identify, Tune, and Validate workflow with a
  bounded manual-output experiment, independent validation data, explicit
  restoration, and an unapplied controller candidate. Control Studio now keeps
  signal choices across workflow navigation, shows retained recording events
  after reopening a result, frames identification fits in engineering units,
  and opens Bode plots on a useful dynamics and crossover range.
- Replace the Control Studio plot-tools disclosure with one direct, responsive
  analysis workspace for time response, identification, Bode, root-locus,
  Nyquist, Nichols, sensitivity, requirements, and typed MIMO evidence. Add
  unit-safe signal layouts, linked graphical candidate editing, bounded
  undo/redo and pinning, and evidence-bound chart-session restoration.
- Add the reusable `Control` Structured Text library with the specified
  combined-form `PID` controller, independent first-/second-order `LowPass`
  filter, normative specification, native equation and seven-scenario
  conformance tests, and an executed closed-loop performance benchmark for
  tracking, robustness, sampling, disturbances, limits, noise, and stability
  margins across nominal, integrating, underdamped, delay-dominant, and
  nonlinear-actuator processes, plus open-loop instability, scheduled gain
  variation, severe deterministic scan jitter, and a coupled two-loop process;
  add a control-system engineering report and staged training curriculum that
  explains the design choices, results, tuning workflow, limitations, and
  improvement program.
- Extend the existing Control Studio workflow with reproducible saved-run
  revisions whose raw samples distinguish requested from applied manipulated
  output without inventing unavailable requests, eighty-four executable
  control-engineering lessons, and production-style lesson source organization
  in which `Main` owns I/O, safety, and composition while substantial stateful
  algorithms use dedicated, pedagogically commented function blocks and typed
  records; independently
  qualified SIMC and AMIGO PI formulas, bounded P/PI/PD/PID candidate
  comparison, common performance and actuator metrics, deterministic
  rejection reasons, and an explicit unapplied recommendation and review flow.
  Relay experiments now switch from fresh measured process feedback and a
  retained hysteresis band instead of replaying a timed square wave, while
  preserving the existing bounded abort and restoration path. SIMC tuning now
  also handles eligible integrating models with rate-gain equations and ships
  an executable heater, buffer-tank, and dryer lesson that compares unapplied
  Fast, Balanced, and Robust candidates under gain and delay variation. The
  classical-analysis sequence now begins with an executable overhead-crane
  Routh-Hurwitz lesson that proves `0 < K < 6` and confirms stable, marginal,
  and unstable cases against three equivalent simulated plants and an
  independent numerical-root oracle. The following positioning-servo lesson
  now derives an uncompensated root locus, proves why gain alone misses the
  motion requirements, and validates a full-transfer-function lead design
  against an equivalent executable servo without current saturation. The
  flexible-drive lesson then measures low-, resonant-, and high-frequency
  response on three equivalent executable plants and compares unshaped and
  resonance-shaped Bode margins without silently applying either candidate.
  The delayed-conveyor-oven lesson follows with complete Nyquist contours,
  qualified encirclement counting, and time-domain confirmation of stable and
  unstable loops, including a long-delay case that wrapped phase misclassifies.
  The precision-drive lesson then evaluates loop, sensitivity, complementary-
  sensitivity, load-disturbance, and control-sensitivity paths on one common
  grid and confirms the shaped controller's robustness/noise improvements and
  slower load-recovery cost on equivalent executable motor copies. The sampled
  positioning lesson then proves the exact hold, sampling-zero, aliasing, and
  one-cycle-delay model in the z-plane, safely exposes unchanged gains becoming
  unstable at a coarse interval, and validates a corrected digital design. The
  following tuning-method lesson separates method eligibility from candidate
  feasibility and engineering preference by comparing qualified SIMC and AMIGO
  PI settings on equivalent heater and dryer plants, retaining AMIGO as
  explicitly unavailable for the integrating tank, and exposing time,
  actuator, margin, delay-margin, bandwidth, sensitivity, and Pareto evidence
  without applying or embedding a winner. The next lesson audits the complete
  exact-Simulation Automatic Tune route on a recirculating water heater,
  retaining the Test, Identify, Tune, and Validate decisions, failure and
  recovery boundaries, unchanged running controller, independent validation
  copy, and explicitly unapplied recommendation. The following conveyor-oven
  lesson now derives static and dynamic measured-speed feedforward from
  separate heater and disturbance paths, executes three equivalent ovens,
  qualifies settled versus not-settled results, and proves model-mismatch and
  invalid-signal fallback without removing PI trim or actuator constraints.
  The jacketed-reactor lesson then commissions the valve-facing temperature
  loop before closing the product-temperature loop, compares direct PI with
  fast and deliberately slow cascade structures on equivalent plants, and
  proves time-scale separation, earlier disturbance detection, tracking, and
  bounded valve authority from executable evidence. The following blending-
  skid lesson derives a wild-stream ratio station from material flow, compares
  it with identical composition-feedback trim on separate two-input plants,
  and proves prompt production-rate compensation, meter-bias recovery, and
  tracked low-flow or invalid-quality fallback. The split-range reactor lesson
  then compares explicit dead-zone and overlap failures with a mutually
  exclusive installed-gain-characterized heating/cooling mapper, including
  neutral crossover, utility-throughput, and authority-loss evidence. The
  compressor lesson then compares normal-only, untracked, and externally
  tracked high-selector override on equivalent plants, proving constraint
  priority, inactive-integral consequences, hysteretic takeover/release, and
  explicit invalid-margin fallback without claiming anti-surge protection. The
  gravity-tank lesson then derives and validates three local nonlinear operating
  regions, comparing a conservative fixed PI with atomic interpolation and
  hysteretically selected PIs whose compensated external tracking preserves
  live setpoint history and bumpless region transfer. The following long-
  conveyor lesson qualifies an ordinary PI boundary before applying Smith
  prediction, then proves nominal rise-time benefit, moderate model-mismatch
  degradation and deterministic measured-feedback fallback under severe
  mismatch. The following cooling-water flow-bench lesson characterizes a
  fixed-head equal-percentage valve approximation and then uses requested
  position, measured stem position, process flow, and repeated reversal
  evidence to distinguish healthy motion, command deadband, and mechanical
  stiction before recommending maintenance instead of an unjustified retune.
  The next binary-distillation lesson replaces a premature centralized-control
  summary with two independently restored open-loop interaction tests, derives
  all four signed steady-state gains and the Relative Gain Array from executed
  evidence, and selects the eligible diagonal pairing while leaving controller
  and decoupler design to the following lesson. That next lesson runs the same
  two PI tunings through direct, static-inverse, and proper dynamic cross-filter
  mappings, compares common servo and feed-disturbance records, and deliberately
  rejects the nominal dynamic design under declared cross-path mismatch when
  its small extra cross improvement does not justify its higher error.
  The following robustness lesson then evaluates nominal and stronger-
  interaction column models by singular direction and multivariable
  sensitivity, selects static decoupling over an equivalent centralized
  coordinator in the feasible envelope, and retains a jointly limited
  `+2/+2` request as explicitly infeasible instead of presenting reduced error
  as successful control. The following interconnected-tank lesson checks
  hardware feasibility before controller design: independently calculated
  controllability and observability ranks are matched to executed upstream-
  pump, downstream-bypass, downstream-sensor, and hidden-state records, while
  simulator truth channels remain explicitly unavailable as installed sensors.
  The next state-space lesson then combines a magnetic-bearing pole-placement
  bench with a loaded-conveyor-motor observer: executable evidence distinguishes
  a nonsettling position-only baseline from measured-state feedback, removes
  nominal zero-load speed bias, and deliberately shows unmodelled damping being
  absorbed into an incorrect physical-load estimate despite accurate speed.
  The weigh-belt lesson that follows implements an uncertainty-weighted scalar
  Kalman estimator beside a fixed filter, qualifies covariance and innovation
  calculations against executed noisy feeder records, isolates a paired
  load-cell-bias fault, and rejects invalid covariance through a finite fallback
  without exposing Simulation truth to the estimator. The active-suspension
  lesson then derives four independently qualified LQR designs from explicit
  state/effort costs, compares them with pole placement on common physical
  measures, and combines the balanced design with a position-only Kalman
  estimate. Executed nominal, doubled-noise, and model-mismatch records retain
  the force-energy, motion, estimator-error, and increased actuator-movement
  tradeoffs instead of presenting LQG as universally optimal. The two-zone
  building lesson then executes a three-step, 729-candidate finite-grid MPC on
  every interval, qualifies its first move and objective against an independent
  oracle, compares it with bounded PI on a common weather/occupancy/tariff
  schedule, and distinguishes completed infeasibility, invalid configuration,
  bounded fallback, and recovery without presenting a precomputed policy as an
  optimizer. The nonlinear inverted-pendulum lesson that follows replaces its
  former linear trace with the shipped sine-gravity plant, derives a bounded
  quadratic Lyapunov sublevel set, verifies eight certified trajectories
  against an independent Runge–Kutta oracle, and keeps recovered outside and
  saturated trajectories explicitly separate from certificate evidence. The
  gravity-loaded robot-joint lesson then compares fixed PID, gain-scheduled PID
  and feedback linearisation over common nominal and heavy-payload trajectories,
  qualifies local interpolation and nonlinear cancellation independently,
  preserves motor-command margin, exposes the predicted model-mismatch offset,
  and falls back deterministically when the cancellation model is ineligible.
  The polymer-extruder lesson then compares fixed, recipe-scheduled, and
  supervised adaptive PI on common drifting-process records, qualifies a
  bounded two-parameter recursive-least-squares estimator against an
  independent scan oracle, separates persistent excitation from explicit
  experiment authorization, freezes unsupported learning, applies scheduled
  fallback, and proves projection, corrupt-data safe output, and bounded
  recovery without hiding actuator or nonsettling costs. The flexible-wing
  ground-test lesson then rejects an unstable aggressive nominal controller
  and qualifies a sampled filtered-PD candidate over 27 structured uncertainty
  cases and a declared multiplicative residual envelope, retaining the
  sensitivity, robustness, actuator-effort, time-response, data-quality, and
  bounded-claim evidence instead of extrapolating it to flight certification.
  The remote pumping lesson then calculates delay margin and a conservative
  latency budget, compares arrival-order command retention with locally timed
  bounded ownership on paired header plants, and proves stale, reordered,
  poor-quality, unsynchronized, excessive-uncertainty and future-dated packet
  rejection with tracked local-PI fallback and explicitly approved recovery.
  The fault-aware pumping lesson then coordinates two fast pressure
  transmitters, slow reservoir level and independent drive feedback; qualifies
  model-assisted sensor isolation, justified single-sensor degradation,
  critical safe states, actuator mismatch, acknowledgement, healthy-time
  qualification and rate-limited approved restoration against one deterministic
  multirate fault record. The blinded steam-distribution casebook that follows
  keeps diagnosis outside the controller and distinguishes a qualified healthy
  baseline, poor tuning, valve stiction, transmitter drift, unavailable
  capacity, upstream disturbance, loop interaction, and model/schedule
  mismatch from synchronized requested-demand, measured-stem, reference-meter,
  disturbance, operating-region, and timing evidence before any retuning.
  The first integrated capstone then asks the learner to commission a walk-in
  cold room without a prescribed winner, comparing single-threshold,
  hysteresis-only, and hysteresis-plus-dwell candidates on the same noisy door-
  load record and validating measured compressor state, equipment-owned
  restart readiness, invalid-quality latching, manual recovery, and discrete
  output without hiding a continuous controller. The recirculating-water-heater
  capstone then joins physical modelling, independent FOPDT identification,
  minimum controller-structure selection, Robust and Balanced PI comparison,
  uncertainty and abnormal validation, and rollback planning into one
  executable commissioning decision while leaving the recommended revision
  explicitly unapplied. The flexible-indexing-table capstone then combines
  two-inertia mechanics, P/PD/PID structure selection, derivative filtering,
  root-locus and Bode/sensitivity qualification, exact sampled execution,
  encoder noise, torque limits and model variation; it retains the qualified
  `10 ms` filtered PID while rejecting unchanged delayed `100 ms` execution and
  leaves revision `78` unapplied. The jacketed-reactor capstone then commissions
  its utility-facing loop before production, combines cascade, qualified
  measured-load feedforward, mutually exclusive steam/cooling split range,
  tracked high-temperature override, modes and deterministic degraded handling,
  and compares the integrated architecture against an equivalent feedback-only
  reactor while leaving revision `79` unapplied.
  The gravity-tank capstone then derives and independently qualifies three local
  square-root-outflow models, measures the regional limitation of a conservative
  fixed PI, validates one atomically interpolated retained PI across the full
  level envelope, and proves schedule boundaries, requested/applied pump flow,
  tracking, degraded authority and rollback while leaving revision `80`
  unapplied. The conveyor-oven capstone then combines independently qualified
  FOPDT, Bode, complete-contour Nyquist and delay-margin evidence with measured-
  load feedforward, Smith prediction, model-mismatch fallback, modes and heater
  authority; executable evidence rejects the aggressive ordinary candidate,
  selects a `6 s` Smith design, and leaves revision `81` unapplied. The binary-
  distillation capstone then joins restored four-path identification, unused
  simultaneous validation, RGA pairing, static decoupling, singular-direction
  robustness, sensitivity, constraint feasibility, invalid-mapping fallback
  and operating modes into one architecture decision, selecting static
  decoupling for the normal envelope while leaving revision `82` unapplied. The
  two-zone building capstone then qualifies an additive thermal-load estimator,
  rejects cascade without a measured fast inner variable, and compares strong
  incremental PI with Balanced and actuator-conservative finite-grid MPC under
  the same forecasts and constraints. Executable evidence retains PI as the
  application recommendation because both predictive candidates exceed the
  declared actuator-movement ratio, while revision `83` remains unapplied. The
  independent capstone then provides an executable compressed-air reference
  without prescribing its winning family: it derives and validates receiver
  and compressor dynamics, screens simple and advanced architectures, proves
  PI stability and robustness, and selects PI plus qualified measured-demand
  feedforward from common normal, uncertainty, actuator and abnormal evidence
  while leaving revision `84` unapplied.
- Add composable Smith prediction, PID gain scheduling, and static two-by-two
  decoupling to the Control library with native conformance tests and tuned
  heated-air-duct, gravity-tank, and climate-chamber projects. Organize all 35
  control projects as a cumulative real-equipment curriculum. The added
  plant-modelling strand teaches response classification, conservation-law
  models, bounded identification, model-order selection, excitation and data
  quality, independent validation and residuals, local linearisation,
  frequency response, and MIMO gain/RGA analysis before P-to-PI offset removal,
  feedforward, cascade, bumpless handover, commissioning, ratio, and constraint
  override, while preserving stable gallery identities.

### Changed

- Refine Control Studio into one readable direct analysis surface with
  coordinated Bode magnitude and phase navigation, exact graphical gain entry
  and live margins, full-width plots below compact live values, visible wrapped
  controls at 200 percent zoom, restrained VS Code-themed chart chrome, export
  of the currently visible plot, and pixel-verified MIMO rendering evidence.
- Expose Control Studio as a permanent truST sidebar destination immediately
  below Devices & Connections. The destination is visibly disabled with a
  reason until a project is open, while configured simulated plants retain
  their direct context-preserving Control Studio handoff. Runnable projects
  can provide an explicit `control-loop.json`; the water-heater proportional
  lesson now uses it so the sidebar opens its live SP, PV, and MV instead of an
  unbound workspace.
- Define the Control Studio chart workspace as a large, discoverable
  engineering surface with visible plot and signal selection, unit-safe
  multi-channel lanes, explicit advanced-plot prerequisites, MIMO matrix/RGA/
  singular-value views, professional time/frequency/stability plots, expanded
  mode, direct graphical controller tuning, linked candidate analysis,
  graphical requirements, pinned design comparison, evidence-bound session
  restoration, uncertainty envelopes, undo/redo, and precise responsive and
  accessibility acceptance criteria.
- Render lesson equations with VS Code and GitHub's native Markdown math,
  including conventional fractions, indices, vectors, and matrices, while
  preserving the curriculum's words-before-symbols and accessible prose rules.
- Replace the draft control-engineering curriculum specification and its
  premature implementation checklist with an 84-lesson causal learning program
  and specification-derived implementation checklist. The program teaches
  required theory before each real-equipment practical, carries one observed
  limitation into the next technique, and progresses through production PID,
  modelling, identification, classical analysis, industrial structures, MIMO,
  state-space, predictive, nonlinear, adaptive, robust, networked, diagnostic,
  and capstone work. The reviewed water-heater proportional-control lesson now
  defines the reusable course-wide conformance pattern: words and physical
  meaning before notation, executable predictions, actual Structured Text,
  precisely defined and validity-qualified measures, retained evidence, and a
  causal handoff from one lesson's observed limitation to the next.
- Make `examples/` the single maintained source for bundled projects, generate
  the VS Code example package from its canonical catalog, and replace the flat
  control list with 84 ordered lessons plus topic links that make the rest of
  the tutorial and example catalog discoverable. The control examples README
  now provides the complete linked course map, stating the real equipment,
  theory, Control Studio stages, and practical skill for every lesson. Every
  canonical lesson directory now begins with its zero-padded course number and
  lives in one of 14 consecutive learner modules, so filesystem browsing
  preserves the same learning order as Control Studio. Separate README indexes
  let engineers find lessons by technique, plant behavior, and workflow without
  imposing a second technical taxonomy on the folder tree.
  Canonical bundled projects now use an explicit non-secret local example token
  so every shipped `runtime.toml` passes the runtime configuration validator.
- Specify runtime clock dispatch name/error identity and deterministic LSP
  diagnostic-override collision precedence, including canonical-key priority
  and alias-only lexical ordering.
- The parser example-corpus test now fails when the checked-in example tree is
  missing and parses every checked-in example instead of silently skipping the
  suite.
- Specify the nine shipped tutorial files as executable compile contracts and
  lock the blinker, traffic-light, and motor-starter examples to their reviewed
  clean-cycle timing and I/O sequences.
- Specify the native browser-analysis and WebAssembly JSON adapter boundary for
  native-result parity, opaque virtual document identity, full-set replacement,
  and explicit JSON success and failure projections.
- Specify the runtime variable-storage contract for instance-field resolution,
  declared layouts, reference-helper equivalence, cache invalidation,
  copy-on-write nested updates, and checked extreme array bounds.
- Specify fail-closed host lvalue writes, bounded helper capabilities,
  host-stage POU defaults and write-through behavior, IEC array repetition,
  representative integer-base exponentiation and Boolean comparison results.
- Specify retain codec compatibility, filesystem transaction visibility,
  retry semantics, atomic migration, and typed migration/orphan events.
- Specify mesh envelope metadata, subscription grammar, typed numeric decoding,
  bounded malformed-input handling, readiness, liveliness, QoS, and snapshot
  failure contracts without overstating TLS or query/reply proof.
- Specify the ADS AMS/TCP prefix, frame-length cap, header/payload correlation,
  canonical serialization, and response endpoint/correlation contract.
- Specify per-cycle watchdog deadline arming/restoration and bounded automatic
  restart backoff without treating host-runtime policy as an IEC deviation.
- Align runtime scheduling documentation on priority, due-time, stable-index,
  registered-high SINGLE, and background-program execution order.
- Specify ADS notification cadence, on-change coalescing, watch-length source
  sampling, invalidation, and representable FILETIME conversion.
- Specify ADS service-port routing, router/system compatibility responses,
  notification receiver binding, and bind-conflict startup failure.
- Specify deterministic ADS server symbol assignment, duplicate rejection,
  flag preservation, serialization identity, and local version progression.
- Specify deterministic simulation-model activation, coupling and physics scan
  boundaries, conflict rejection, replay, scripted faults, and scaled-clock
  watchdog isolation.
- Specify the `trust-st-complete-v1` PLCopen profile, ST/data-type interchange,
  fail-closed import diagnostics, migration and CODESYS metadata, reviewed
  vendor shims, and Allen-Bradley/Siemens adapter artifact contracts.

### Fixed

- Replace raw starting-state arrays with labelled controls for first-order,
  integrator, gravity-tank, second-order and robot-joint models, retaining
  invalid-input protection and exact values.
- Render every robot-joint and state-space plant connection with its own named
  picker; preserve neighboring channel bindings and conversions when one changes.
- Route starter and lesson opening through one project-scoped pending destination,
  suppress superseded instructions, and offer recovery or an explicit retry when
  the selected source file or lesson cannot open.
- Show field-specific plant actuator defaults instead of labelling every empty
  value “Unbounded”; distinguish a zero slew rate from no slew limit.
- Order control lesson modules by their declared lesson sequence and show
  noncontiguous lesson ranges without implying missing lessons are included.

- Default omitted FOPDT delay history to its input operating point as specified,
  preserving explicitly configured zero history and deliberate non-equilibrium starts.

- Preserve simulated-plant validity limits when editing names or other exposed
  settings, carrying unedited configuration through the canvas draft and save
  candidate without resurrecting explicitly cleared editable effects.

- Apply plant command uncertainty independently to the held PLC command on
  every simulation substep, avoiding compounded offsets or gains within one
  PLC scan while preserving raw-command trace identity and deterministic replay.

- Resolve the configured project loop when Open Control Studio is invoked
  without arguments or from the sidebar, offer a labelled choice for multiple
  projects, and provide repair/example actions for invalid or missing loop
  configuration while preserving explicitly scoped plant/loop handoffs.

- Correct rejected Control Studio bindings in 24 control lessons and their
  bundled copies. Project-owned controllers declare their actual design owner;
  open-loop comparisons retain reference signals without inventing a setpoint
  or weakening unit and address validation.

- Keep the real Control Studio response chart visible at a usable height, use
  the declared manipulated-variable engineering range, disable plot and
  workflow actions until their evidence exists, prevent limit labels from
  covering traces, and treat an empty saved-result history as a neutral state.

- Resolve typed enum literals through the active namespace and `USING` scope,
  allowing imported declarations such as `PIDMode#Manual` to type-check across
  files without allowing a same-named variable to hide the literal's type.

- Write the correlated runtime control acknowledgement before applying an
  accepted shutdown stop signal, preventing managed fleet stop from losing its
  response while the runtime process exits under load.
- Update the VS Code dependency lock to patched `brace-expansion` and `js-yaml`
  releases after their high-severity denial-of-service advisories.
- Keep malformed numeric candidates such as a real literal with a trailing dot
  as one fail-closed lexer error instead of publishing a valid numeric prefix.
- Preserve the stable `E106` invalid-identifier diagnostic when malformed
  identifier candidates are rejected directly by the lexer, instead of
  degrading the editor diagnostic to a generic parser error.
- Keep function-block parameter defaults in instance storage while encoding
  recursive function/method call-local `IN` and `OUT` defaults, including
  fixed and wildcard arrays, structs, unions, nested strings, and NULL
  references. Constant validation now shares one bounded recursion policy and
  rejects malformed aggregate frames and non-NULL reference identities, while
  supplied interface and object-instance formals no longer fail bytecode
  publication merely because no portable implicit constant exists.
- Publish uniquely resolved cross-file enum values into their enclosing scope
  so later program initializers can use them, while preserving ambiguity for
  colliding variants from distinct enum types.
- Preserve enum identity for unqualified `CASE` labels in bytecode lowering so
  both VM execution paths select the matching branch without weakening
  same-named constant shadowing.
- Keep same-named fleet runtimes, endpoints, configured mesh peers, and links
  distinct across hosts and containers when Devices & Connections combines
  legacy schema-v2/v3/v4 snapshots, while failing closed only for ambiguous
  references and preserving unrelated topology.
- Clamp LSP telemetry duration milliseconds at `u64::MAX` instead of wrapping
  an overflowing duration into a small, misleading latency value.
- Document the external-diagnostic JSON input contract, including trimmed,
  case-insensitive string severities and numeric LSP severities.
- Reject mutable BOOL, REAL, STRING, and aggregate-member dependencies in
  explicit variable initializers instead of applying the constant-expression
  rule only to direct integer initializers.
- Preserve the IEC reference-declaration exception so `REF_TO` variables may
  initialize from `REF(...)` of eligible existing storage while retaining the
  canonical REF lvalue, type, visibility, and lifetime diagnostics.
- Publish stable `C001`-`C005` LSP configuration diagnostics for unreadable or
  malformed files, unknown standard-library profiles and workspace visibility,
  invalid positive bounds, and ignored invalid severity overrides; E303/E304
  explainers now link to their actual IEC array/range clauses.
- Keep project source discovery inside the supported `.st`/`.pou` set after
  resolving contained symbolic links, while retaining deterministic canonical
  identity for valid contained aliases.
- Preserve runtime link health detail in Devices & Connections and render
  degraded/error links with distinct warning/error tones plus an accessible
  hover/focus explanation.
- Release tag and version-evidence guards now fail closed unless an annotated
  tag resolves to the exact release commit, CI and Release runs match the exact
  event, branch, and SHA with usable URLs, API collections are well formed,
  successful API responses are top-level objects with stable endpoint/status
  diagnostics instead of tracebacks, and published asset names are unique.
- HIR temporal arithmetic now implements the complete documented IEC 61131-3
  Table 35 operand/result matrix and rejects swapped, cross-family, and other
  unlisted temporal combinations at the operator boundary.
- Invalid user-defined type formations now remain internal diagnostic-recovery
  records instead of becoming publicly discoverable types, while valid sibling
  declarations continue to publish normally.
- `CURRENT_DT()` now has an explicit UTC host-clock contract and preserves the
  full millisecond `DT` tick range instead of overflowing when the intermediate
  Unix timestamp exceeds signed nanoseconds.
- OpenOT runtime conformance tests now carry the complete pinned authoring and
  value-sampling Structured Text fixture closure, so those tests execute
  independently of a sibling `open-ot-ref` checkout instead of failing at the
  frozen-fixture boundary.
- `trust-dev test` now compiles projects that contain no test POU before
  reporting an empty run, so compile-negative fixtures cannot become false
  successes merely by using ordinary `PROGRAM` declarations.
- HIR and runtime timer registration now support the IEC 61131-3 Table 46
  `TP_TIME`, `TON_TIME`, and `TOF_TIME` names with fixed TIME `PT`/`ET`
  signatures and the same scan-step behavior as their overloaded base forms.
- Bytecode `FOR` loops now derive their internal zero sentinel from the
  evaluated step value, so valid unsigned control variables and steps no
  longer fail strict numeric comparison with an unrelated `LINT` sentinel.
- `trust-runtime play` now creates a project only for ordinary project
  absence; current-directory and system-I/O configuration failures remain
  visible instead of being treated as a first-run setup request.
- ADS onboarding sum-up reads now reject unknown handles and incomplete,
  duplicate, unknown, non-good, valueless, or descriptor-incompatible results
  before projecting any wire value.
- HIR now rejects an explicit integer variable initializer that depends on an
  ordinary mutable variable, while preserving literal and declared-constant
  integer expressions required by IEC 61131-3 Section 6.5.1.3.
- File-backed retain snapshots now persist `PROGRAM` `VAR RETAIN` values under
  collision-free qualified identities, restore the saved value on warm
  restart, and remain unloaded on cold restart; the reliability test now
  distinguishes a real store reload from unchanged live memory.
- Debug breakpoints now emit at most one stop per breakpoint identity within a
  runtime scan, preventing overlapping debug-map entries from repeatedly
  stopping the same scan while preserving breakpoint re-hits on later scans.
- Runtime bytecode documentation now matches the executable STBC 1.1 opcode
  set and specifies source-to-section, resource/process-image, variable
  metadata, and validation contracts used by the existing encoder and runtime.
- trust-debug now rejects `stackTrace` requests for thread IDs that are absent
  from the current DAP thread projection instead of returning a synthetic frame
  for another thread; debug evaluation documentation now matches the shipped
  side-effect-free standard-function whitelist.
- trust-runtime-core and trust-hir now reject explicitly typed mixed numeric
  operations when the operands have no accuracy-preserving common type instead
  of silently widening values such as `ULINT` to `REAL`; representable untyped
  numeric literals retain their valid contextual operand or overloaded-argument
  type.
- OSCAT: decimal rounding, random helpers, array shuffle, random generators,
  and dew-point relative-humidity conversion now preserve their declared
  `INT`/`REAL` types instead of failing at runtime when generic `LIMIT`
  expressions widened untyped bounds; `IS_SORTED` now reports unsorted
  nonempty arrays instead of overwriting its detected failure. OSCAT
  bit-transfer documentation and tests now also honor truST's existing
  rejection of non-finite `REAL` values.
- VS Code: managed runtimes whose status command fails or returns an
  unrecognized state are now shown as unavailable with lifecycle actions
  disabled instead of being misreported as stopped and offered a misleading
  Start action.
- trust-syntax: `ACTION ... END_ACTION` declarations nested in a `PROGRAM` are
  now parsed as action nodes instead of terminating the program statement list
  early and cascading top-level parse diagnostics.
- PLCopen Motion: `MC_Stop` no longer reports `Busy` and `Done`
  simultaneously after reaching zero velocity, and public parameter writes to
  standard read-only Part 1 values are now limited to the documented disabled,
  idle initialization phase; the vendor BOOL extension is published as
  `PN_TRUST_VendorBool0`. Buffered commands flushed by `ErrorStop` now report
  `Error` with the retained axis fault code instead of `CommandAborted`, even
  when `MC_Reset` runs before the buffered FB observes the flush.
- trust-runtime: removing the final project I/O driver now retains `io.toml`
  with an explicit no-driver posture, preserving safe-state policy and project
  comments outside rewritten driver/safe-state items without implicitly
  activating machine-wide system I/O fallback.
- trust-runtime-core: indexed string replacement now applies the destination
  `CHAR`/`WCHAR` width to one-scalar `STRING` and `WSTRING` values instead of
  bypassing overflow validation.
- trust-hir: unambiguous integer-valued enumerated constants are accepted as
  array bounds, while nonconstant integer indexes are no longer rejected
  solely because their declared subrange extends beyond those bounds; their
  actual computed value remains bounds-checked at runtime.
- VS Code: Devices & Connections now keeps established degraded and failed
  links solid while using semantic health styling, and renders configured-only
  links dashed so line style consistently represents topology proof.
- trust-runtime: registry publish help now describes standard bundle detection
  instead of claiming a current-directory fallback.
- trust-runtime: `hmi init`, `hmi update`, and `hmi reset` now preserve standard
  bundle-detection failures instead of silently treating an undetectable
  current directory as the selected project.
- trust-runtime: versioned deployment now rejects invalid pointer slots before
  installation, copies enabled ADS and OPC UA client sidecars into the
  installed bundle, and rejects symbolic links in the copied bundle closure.
- trust-runtime: standalone `ide serve` now rejects a non-directory project
  path before constructing control state or attempting to bind the web server.
- trust-runtime: generated runtime templates and shipped examples no longer
  serialize an empty mesh authentication token that current configuration
  validation rejects; OSCAT runtime examples also include the required log,
  retain, watchdog, and fault sections, restoring loadable example projects and
  offline communication topology.
- trust-runtime: Modbus worker input and output deadline results now preserve
  the configured `fault`, `warn`, or `ignore` policy; faulted stale input no
  longer copies a cached snapshot, and timed-out output remains queued.
- trust-runtime: direct-address parsing now rejects sized, suffixed, spaced,
  and otherwise malformed IEC wildcard forms, plus signed or non-decimal
  address components such as `%IW+1`, instead of accepting partial or
  ambiguous locations.
- trust-runtime: Modbus `f32` point scaling now rejects finite internal `f64`
  results that would narrow to infinity in the process image or wire payload.
- trust-runtime: MQTT raw reads now select the newest payload drained for a
  scan, while the default fault policy preserves stale-input, output-deadline,
  and typed-output preflight failures instead of degrading them to success.
- trust-runtime: OPC UA client configuration now rejects duplicate connection
  names, empty endpoint authorities, anonymous credential fields, and
  conflicting access aliases; client PKI operations ignore or reject symbolic
  links detected at roots, entries, and promotion targets, and one-shot
  read/write paths now invoke an exact service-result cardinality guard.
- trust-runtime: OPC UA client workers now reject mismatched and stale-session
  subscription events, preserve newer queued output generations across older
  completions, reject non-finite scalar outputs before transport, and wake
  promptly while revoking readable authority during shutdown.
- trust-runtime: ADS server publication now selects bounded symbol sets in
  canonical-name order, client policy compares parsed exact IP identities and
  requires explicit opt-in for unpinned entries, and security-first write
  rejection preserves precise ADS errors without queueing PLC mutations.
- trust-runtime: ADS client workers now reject malformed or miscorrelated
  handle, poll, subscription, and notification responses before publishing
  authority; rebuild symbol-derived state only from stable token-bracketed
  candidates; preserve newer generation-tagged output intent across older
  completions and error projection; terminally retire only the correlated
  rejected generation; reject non-finite output values before the shared queue;
  and revoke cached input authority during prompt shutdown.
- trust-runtime: active ADS device lookup now requires an IP-literal route's
  host, AMS port, and supplied target AMS identity to identify the same Doctor
  target instead of accepting a partial endpoint match.
- trust-runtime: the ADS server Doctor now derives health from the live server
  symbol table, rejects blank external proof and zero handles, requires complete
  notification samples, and rejects response-direction or endpoint identity
  mismatches before trusting loopback payloads.
- trust-runtime: ADS client configuration parsing now rejects duplicate
  connection names, noncanonical target AMS Net IDs, and blank or noncanonical
  explicit local AMS Net IDs before constructing activation routes.
- trust-runtime: ADS client configuration parsing now rejects zero or
  non-STRING capacities, overflowing array byte lengths, and index-address
  sizes that disagree with the declared scalar or array type.
- trust-runtime: ADS client connections now reject noncanonical target or local
  AMS Net IDs and target AMS port zero before invoking the connector or opening
  TCP, while preserving cached symbol-handle and subscription state when
  validation fails.
- trust-runtime: the ADS Doctor now rejects malformed manual endpoints before
  transport activity, rejects blank or absent explicit symbols at the upload
  boundary, refuses active-device snapshots for another ADS endpoint, and no
  longer reports empty, timestamp-less, or degraded live point sets as a
  passing batch read.
- trust-runtime: ADS diagnostics now fail closed for empty reports and
  non-blocking failed steps, reject incomplete or role-mismatched TwinCAT
  production evidence, and require the declared age, clock, deployed-config,
  and live-status evidence to remain current before reporting readiness.
- trust-runtime: live ADS onboarding discovery now retains distinct runtime
  endpoints that share one host IP, and guarded write probes attempt and verify
  restoration after probe-write or read-back failure while preserving any
  restore failure in the returned diagnostic.
- trust-runtime: Runtime Cloud preflight now rejects empty, blank, or duplicate
  target lists instead of allowing an empty dispatch or repeating a target;
  cross-site writes in `dev` and `plant` profiles no longer incorrectly require
  a WAN allowlist, while `wan` remains explicit-allow only.
- trust-runtime: Runtime Cloud control and I/O proxy planners now reject blank
  routing/correlation fields and incompatible I/O proxy API versions before
  preflight, returning a `400 contract_violation` instead of manufacturing an
  invalid action request.
- trust-runtime: Runtime Cloud configuration reconciliation now records only
  the immutable desired snapshot actually applied; a newer desired revision
  accepted during an in-flight success or failure remains pending instead of
  being falsely reported in sync or stranded.
- trust-runtime: Runtime Cloud forward-additive schema validation now rejects
  blank or duplicate field names, zero-sized or overflowing ranges, overlaps,
  and fields inserted before the prior layout end instead of reporting an
  ambiguous memory layout as compatible.
- trust-runtime: Runtime Cloud topology no longer reports invented latency or
  packet-loss values from discovery-only presence, and loopback discovery for
  one peer no longer classifies unrelated peer runtimes as same-host T0
  candidates.
- trust-runtime: Runtime Cloud dual-host HA now detects split-brain candidates
  across the complete supplied HA group even when an action targets only one
  candidate, and rejects a duplicate request while its first dispatch remains
  in flight instead of starting the protected command twice.
- trust-runtime: ADS onboarding now rejects malformed runtime-host and target
  IP addresses and blank or malformed local AMS Net ID overrides before
  constructing route authority, and canonicalizes valid six-octet overrides.
- trust-runtime: ADS discovery now preserves distinct PLC runtime endpoints
  that share one host IP while still deduplicating repeated observations of
  the same AMS Net ID and AMS port.
- trust-runtime: ADS route planning now encodes generated XML as data before
  embedding it in PowerShell, preventing route-name here-string breakout, and
  rejects blank one-shot credentials before calling the route wire.
- trust-runtime: ADS server-control symbol import now confines its three
  outputs to distinct project-relative non-symlink paths, keeps snapshots in
  the canonical import cache, and permits follow-up regeneration only when the
  existing generated ST still matches the current ADS configuration and cached
  snapshots. Acknowledged Write-only symbols now remain Write-only instead of
  acquiring a remote Read capability during import.
- trust-runtime: ADS symbol import no longer treats an empty include pattern as
  a wildcard, correctly backtracks `*` matches to anchored suffixes, and uses
  the canonical ST lexer to prevent keywords from becoming generated variables.
- trust-ads-server: malformed sum-up write and read/write frames are now fully
  validated before any runtime write is queued, and notification-handle wrap
  finds a free non-zero handle instead of replacing a live registration.
- benchmarks: mesh query/reply and runtime-cloud dispatch now exercise the
  exact `--payload-bytes` requested instead of silently capping at 256 bytes;
  T0, mesh, and dispatch reports expose the effective payload length.
- ADS CLI: `ads server ... --json` now rejects control responses with
  `ok = false` or a missing Boolean `ok` instead of printing them and exiting
  successfully; JSON and human output now share the control-envelope check.
- trust-harness: an explicitly empty authoritative `sources` list now reports
  `invalid_argument`; failed load/reload attempts preserve the live session,
  and focused protocol tests lock source precedence, time-alias precedence,
  zero-cycle `run_until`, and stable structural/runtime/boundary error kinds.
- docs: the IEC deviations log now contains only concrete normative conflicts
  or omissions. IEC-silent runtime/tool/vendor behavior remains in product
  specifications, IEC-permitted non-finite `REAL` choices are recorded in the
  decisions log, and the PLCopen LD interchange subset is routed to the
  PLCopen deviations log.
- trust-runtime: rejected online changes and restarts now preserve the live
  variable/instance image, scan counter, fault state, telemetry scan state, and
  queued debugger mutations when retained-value migration or restart instance
  construction fails; warm-restart storage and retained values are staged
  before the bytecode commit.
- trust-runtime: newly encoded bytecode now preserves the declared IEC resource
  identity, and named bytecode application rejects an unknown resource instead
  of silently falling back to a different primary resource while retaining the
  single-resource legacy-placeholder compatibility path and normalizing it to
  the requested active resource identity.
- trust-runtime: guided project setup now rebuilds runtime configuration,
  generated ST configuration, and bytecode coherently; wizard source discovery
  treats paths literally and includes project dependencies; generated resource
  names are valid non-reserved ST identifiers; dangling project I/O links are
  removed when system I/O is selected; and remote setup fails closed on secure
  randomness or token-expiry overflow. Partial browser apply requests now share
  the advertised safe defaults and cannot write system-wide I/O unless
  `write_system_io` is explicitly true; interactive browser ports above 65535
  are rejected instead of wrapping to another port; and CLI setup rejects
  browser-only access, bind, port, and token-TTL options instead of silently
  ignoring them, including explicitly supplied default values.
- trust-runtime: runtime and I/O configuration now reject millisecond values
  that cannot fit the signed-nanosecond duration, typed safe-state integers
  outside their addressed BYTE/WORD/DWORD width, mixed legacy and multi-driver
  I/O forms, and blank explicit paths, credentials, scalar selectors, or
  collection/map entries instead of panicking, wrapping, silently ignoring
  fields, substituting defaults, or downgrading authentication/source pins.
- trust-runtime: `play` now validates restart and simulation-scale options
  before auto-creating a selected project; runtime configuration rejects
  unknown log levels instead of silently treating them as `info`; legacy
  runtime-root source discovery treats paths literally and recursively; and
  validation/startup summaries no longer present disabled I/O drivers as
  active.
- trust-runtime: registry publish without `--project` now preserves standard
  bundle-detection failure instead of silently substituting a non-project
  current directory and reporting a later invented-bundle error.
- trust-runtime: managed-runtime status no longer reports `stopped` when local
  auth config is invalid or a connected endpoint rejects/mangles the response;
  lifecycle control now checks response IDs and bounds response lines, read-only
  status/log calls do not create state, log tailing is memory-bounded by the
  requested line count, and early child/PID persistence failures are cleaned up.
- trust-runtime: HMI scaffolding now discovers compiler project sources and
  local dependency sources recursively with literal filesystem paths, so glob
  metacharacters in project names no longer produce a false empty project;
  wizard Git setup now rejects malformed or dangling `.git` file markers.
- trust-runtime: managed-fleet manifests now reject unsafe, whitespace-padded,
  or duplicate runtime names, malformed control endpoints, zero ports, and
  cross-role TCP port collisions before lifecycle mutation; fleet control-token
  generation now fails closed when operating-system secure randomness is
  unavailable.
- trust-runtime: deprecated workbench-command aliases no longer let a directory
  or non-executable sibling `trust-dev` shadow a usable PATH command, and Unix
  signal termination now retains the conventional `128 + signal` exit status.
- trust-runtime: deploy labels can no longer escape the bundle store, automatic
  labels remain unique within the same second, relative deployment roots and
  dangling pointers are handled correctly, ordinary files are not overwritten
  as pointers, external pointers are rejected, and pruning preserves the
  actual immediate rollback bundle. Deployment summaries now identify the new
  bundle, detect every runtime and I/O configuration-file change, compare safe
  states structurally, traverse source paths without glob interpretation, and
  fail instead of silently discarding source-read errors.
- trust-runtime: `ctl` now keeps the selected project's control token when an
  explicit endpoint overrides only the endpoint, and exits non-zero for
  rejected, malformed, oversized, timed-out, or mismatched-ID control responses
  instead of reporting script success.
- trust-runtime: the conformance runner now rejects malformed case IDs,
  unknown category directories, case directories without manifests, source
  paths that escape their case, and `compile_error` cases that compile
  successfully instead of silently omitting or blessing invalid corpus entries.
- trust-runtime: standalone `ide serve` now prioritizes an explicitly selected
  root runtime, reports malformed primary runtime configuration before binding
  the web server, and loads the selected workspace runtime's recursive,
  deterministic build-source set instead of looking only in `<workspace>/src`.
- trust-runtime: benchmark jitter summaries now report zero observations when
  fewer than two latency samples exist instead of inventing a zero-valued
  jitter measurement.
- trust-runtime: mistyped top-level commands now receive the correct suggestion
  when `--verbose` precedes the command, and the suggestion vocabulary includes
  the shipped `check` and `agent` command families.
- trust-runtime: pending debug writes and active forces are now cleared at
  runtime fault, stop, and restart boundaries so stale debugger mutations
  cannot cross a lifecycle transition.
- trust-runtime: pre-commit watchdog and output-handoff faults now restore the
  last committed output image before applying a partial safe-state map, so
  unconfigured points cannot receive pending values from the failed scan.
- trust-runtime: explicit conversions to `REAL` or `LREAL` now return the
  stable overflow fault when narrowing, parsing, or IEC bit transfer would
  synthesize NaN or infinity inside PLC state.
- trust-runtime: deterministic simulation-time advancement now saturates at the
  signed nanosecond bounds instead of panicking or wrapping on overflow.
- trust-runtime: OPC UA server variables backed by periodic PLC snapshots now
  advertise read-only access and reject client writes instead of acknowledging
  values that the runtime never applies to PLC storage.

- verification: the runtime communications fuzz gate now selects the live WAN
  allowlist smoke module instead of exiting successfully after running zero
  tests; bounded campaigns also reject any zero-test Cargo filter.
- trust-ads-server: restored the command-dispatch cargo-fuzz target after the
  symbol snapshot interface moved to shared `Arc` ownership, so all registered
  ADS fuzz targets compile and execute under the bounded campaign.
- trust-dev: project-scoped commits now abort before mutation when the Git index
  already contains an in-scope path, while repository-root commits reject any
  pre-staged path and dry runs leave both the worktree and index unchanged.
  Project paths must resolve canonically inside the repository, and the index is
  checked again after interactive prompts so concurrently staged in-scope paths
  are not absorbed.
- vscode: connector state and health projections now accept only the shared
  canonical wire vocabulary, so unknown backend values fail visibly instead of
  flowing into the Devices & Connections UI as an invented status.
- vscode: updated build and test dependencies and pinned patched transitive
  versions so the committed extension lockfile passes `npm audit` with zero
  known advisories; CI and release packaging now enforce the same audit.
- trust-lsp: closing an unsaved file-backed document now cancels in-flight
  semantic work, restores durable disk contents, and evicts semantic-token and
  pull-diagnostic caches; late results are generation-checked under the cache
  lock so discarded buffer state cannot survive `didClose`.
- trust-lsp: invalid incremental edit lines now preserve the last valid buffer
  and show a full-resynchronization error, while range/on-type formatting uses
  the shared LF, CRLF, and bare-CR line model instead of an LF-only splitter.
- trust-runtime: MQTT discovery now reports authentication and authorization
  CONNACK rejections as `likely` protocol evidence instead of overclaiming an
  accepted, confirmed broker session; clean-session DISCONNECT behavior is
  preserved.
- trust-runtime: control requests denied by the reviewed role matrix now return
  the stable `insufficient_role` error code before dispatch; reviewed operation
  names, fixed minimum roles, and debug-surface classification share one
  internal registry while unclassified operations remain Admin-only.
- trust-runtime: bytecode decoding, validation, VM traps, and runtime failures
  now expose stable lower-snake-case machine identifiers without parsing
  diagnostic text; HMI type, bounded-string, subrange, and non-finite write
  rejections return the matching identifier in `error_code` before queueing.
- trust-runtime: STBC decoding, validation, executable materialization, and VM
  execution now enforce the documented fixed container, instruction,
  reference, local, parameter, native-call, operand-stack, call-depth, and
  per-invocation instruction limits; stack, register, and tier-1 execution
  charge the same original bytecode instructions across nested calls.
- trust-hir/trust-runtime: implicit numeric assignment now permits only
  accuracy-preserving widenings, ordinary subrange initializers are checked at
  both bounds, contextual subrange literals retain their declared runtime tag,
  binary expressions, function returns, FOR bounds, and call arguments preserve
  their HIR-resolved or declared numeric type, and crafted primitive-tag
  mismatches fail before storage.
- trust-runtime: the embedded web server now isolates read-only traffic from a
  fixed, bounded body/mutation lane, so incomplete request bodies cannot block
  HMI and control reads; saturated lanes fail promptly with HTTP 503 and the
  stable `server_busy` denial code.
- trust-runtime: TON, TOF, and TP elapsed-time accumulation now clamps safely
  at `PT` instead of panicking in debug builds or wrapping negative in release
  builds near the signed TIME/LTIME duration boundary.
- trust-runtime: STBC decoding now rejects duplicate standardized section IDs
  instead of silently selecting the first conflicting section during later
  validation and execution.
- trust-runtime: bytecode decoding now rejects top-level and nested collection
  counts that cannot fit within their section payload before reserving the
  declared collection capacity, preventing tiny malformed containers from
  requesting disproportionate allocations.
- trust-syntax: malformed `CASE`, `ELSIF`, `FOR`, `WHILE`, and `REPEAT`
  statements now diagnose missing required delimiters instead of accepting
  partial syntax as a valid control-flow construct.
- trust-lsp: cancelled workspace diagnostic requests now return
  `ContentModified` instead of a successful empty or partial report that could
  hide diagnostics collected after a newer semantic request superseded them.
- trust-lsp: UTF-16 positions now recognize bare-CR lines and keep every CRLF
  terminator byte at the preceding line end, matching the LSP line-boundary
  contract for incoming requests and outgoing ranges.
- trust-runtime: debugger statement-boundary pauses no longer count operator
  dwell time against the active cycle watchdog or output-commit deadline;
  genuine execution time before and after resume remains bounded.
- trust-runtime: enabling the debug control surface now requires Admin instead
  of Engineer, and unclassified future control operations fail safe at Admin
  rather than inheriting Viewer authority.
- trust-runtime: online change now reads and validates the retained snapshot
  before replacing the live executable or resetting runtime state, preventing a
  retain-store failure from leaving a partially applied reload.
- trust-runtime: safe-state application now requires confirmed healthy driver
  handoff, reports queued/reconnecting worker delivery as unconfirmed, attempts
  every configured driver, and prevents a deliberate stop from reporting
  `Stopped` when physical safe-state delivery was not confirmed.
- trust-runtime: retained snapshots are now fully validated before any restored
  global is changed, preventing a later incompatible value from leaving earlier
  values partially applied after a failed warm load.
- trust-runtime: MQTT typed input-point batches now commit their mapped
  process-image snapshot only after every payload validates, preventing values
  from a rejected non-finite batch from leaking into a later successful scan.
- trust-runtime-core: integer operand normalization now has an explicit
  fail-closed contract: every integer runtime tag preserves its mathematical
  value when representable as `i64`, oversized `ULINT` values return overflow,
  and non-integer tags return a type mismatch.
- trust-runtime-core: unsigned integer result materialization now preserves
  every representable `USINT`, `UINT`, `UDINT`, and `ULINT` value exactly,
  rejects destination overflow, and rejects non-unsigned destination tags.
- trust-runtime-core: retained snapshot insertion now has an explicit
  deterministic contract: new names append, reinserting the same resolved
  name replaces its value in place, and stored runtime values are preserved
  exactly until the owning persistence or reload boundary validates them.
- trust-runtime-core: task readiness now has an explicit deterministic
  contract for SINGLE edge sampling, periodic suppression, backward and
  forward logical-clock jumps, missed intervals, and saturating overrun
  accounting.
- trust-runtime-core: portable fault-state clearing now has an explicit
  lifecycle contract: it atomically removes the fault flag and stored error,
  preserves the configured fault policy, remains idempotent while healthy, and
  does not itself claim resource recovery or safe-state handling.
- trust-runtime-core: portable fault-policy decisions now have an explicit
  projection contract for halt, safe-halt, and restart, including the exact
  safe-state flag and the boundary between returning a decision and executing
  recovery actions.
- trust-runtime-core: fault recording and fault-policy replacement now have an
  explicit state-preservation contract, including latest-error replacement and
  changing policy while faulted without erasing the current fault.
- trust-runtime-core: watchdog policy installation now has an explicit
  normalization contract: enabled non-positive timeouts floor to one
  millisecond, valid and disabled timeouts retain their values, and installing
  policy does not itself execute a watchdog action.

### Added

- The VS Code extension test harness now supports opt-in
  `TRUST_VSCODE_TEST_GREP` filtering and optional per-test JSON output while
  preserving the unfiltered `npm test` default.

- verification: added direct cargo-fuzz targets for HIR lowering, PLCopen XML,
  bytecode containers, runtime configuration, LSP incremental edits, and HMI
  payloads; added direct runtime-anomaly tests; and resolved six obsolete test
  skips, including retirement of a capture for a runtime command that is
  intentionally hidden from the command palette.
- release: added digest-bound artifact provenance and result-derived
  conformance status assets, plus release checks binding the workspace version,
  tag, successful workflow, required assets, and GitHub Latest pointer.
- trust-runtime: documented the complete STBC validator-before-apply contract,
  added product-path malformed-bytecode case execution, and promoted the
  existing owner, reference, call, jump, stack, schema, checksum, and version
  rejection checks from quarantine into the regular test suite.
- trust-runtime: added focused automatic-restart storage tests alongside the
  existing runtime panic-containment and bounded Modbus slow-device tests.
- trust-runtime: added a Viewer-gated `connectors.status` control surface that
  projects process-image I/O driver health and ADS client/server status into
  the shared connector status schema without changing the existing
  `status.io_drivers`, `ads.status`, or `ads.server.status` JSON contracts.
- trust-runtime/vscode: Browser HMI and Devices & Connections now consume the
  shared connector status and discovery-confidence contract while presenting it
  with user-facing labels: HMI summarizes connections in one compact header
  chip, and Network Canvas/Discover use Connection, Health, Verification, and
  Signals wording instead of backend connector/proof terminology. Devices &
  Connections now rejects unknown state, health, or confidence values while
  retaining the affected peer topology and showing the validation error instead
  of silently dropping all peers.
- vscode: Generated truST projects and bundled examples now hide VS Code's
  native debug-status selector so the truST sidebar remains the single visible
  run/debug control surface while keeping the native simulator launch
  configuration available.
- vscode/trust-debug: Devices & Connections can add browsed ADS tags to a
  stopped project through the same deterministic `ads import-symbols` pipeline,
  and simulator/debug launches now load the project's ADS configuration so the
  generated `_quality` globals prove the live TwinCAT read path in Live Values.
- trust-runtime: Modbus TCP I/O can now explicitly select FC01/FC02/FC03 input
  reads and FC05/FC06/FC15/FC16 output writes through `input_function` and
  `output_function`, and can use optional per-point maps with bool/u16/i16/u32/
  i32/f32 types, scaling, byte order, and word order while preserving the
  existing FC04 input-register and FC16 multiple-register default profile.
- trust-runtime: MQTT I/O can now use optional typed per-point topic maps with
  bool/u16/i16/u32/i32/f32 values, text/JSON/binary payload formats, and linear
  scaling while preserving the existing raw `topic_in`/`topic_out` byte bridge.
- trust-runtime: MQTT typed output points can opt into a bounded Sparkplug B
  outbound node profile (`spBv1.0`, Sparkplug 3.0.0) that publishes NBIRTH,
  configures NDEATH as the MQTT last will, and emits NDATA metric payloads with
  Tahu-compatible scalar protobuf encoding.
- trust-runtime: added an ignored protocol device-in-the-loop gate for lab
  EtherCAT, ADS, Modbus, and MQTT targets, including explicit skip artifacts,
  hardware-required failure mode, nightly/manual workflow wiring, and JSON
  evidence for topology, discovery, doctor, and broker interop runs.
- trust-runtime: expanded the deterministic conformance suite to v2 with
  strings, arrays, structs, enums, nested values, OOP dispatch, `REF_TO`,
  retain/reset matrix, scheduler, and simulated connector-status determinism
  categories; CI now validates both summary schemas and uploads JSON plus
  human-readable conformance reports as artifacts.
- vscode: added the Libraries surface under Project for reusable ST libraries,
  including packaged OSCAT and PLCopen Motion curated choices, vendored
  per-project dependencies, local-folder and Git add flows, expandable
  function block/function/type browsing, remove/update actions, and shared
  truST webview theme coverage. The bundled examples now include a portable
  PLCopen Motion single-axis starter with a vendored motion library and mapped
  Live Values proof points.
- trust-runtime: added the staged Beckhoff ADS client import workflow with
  deterministic `ads.toml` + cached symbol snapshot validation, generated ST
  value/`_quality` globals, scan-safe background worker/cache handoff,
  `[runtime.ads]` bundle loading and runtime scan-cycle wiring, on-change/cyclic
  ADS notification caching, first-class HMI/debug/historian/OPC UA surface
  coverage, explicit plain-ADS security warnings and route-write guardrails,
  CLI failure-mode coverage, the runtime-host `/setup/ads` onboarding wizard,
  VS Code ADS commands that reuse the selected Runtime pane context, route
  artifact generation/removal, remote plain-TCP route-add through the local CLI
  without forwarding TwinCAT credentials to the runtime, ADS Doctor/status
  surfaces, multi-target UDP broadcast discovery, live `ads browse` symbol
  listing and `ads validate --live` symbol validation, guarded Doctor write
  probes with read-back/restore verification,
  non-Windows ADS-client
  StaticRoutes.xml flags, Usermode Runtime route-file discovery,
  compatible-symbol filtering during live uploads, preview-before-write symbol
  import, and an offline communication example that compiles without a PLC.
- trust-runtime: added Beckhoff ADS server support for exposing selected
  runtime globals as ADS symbols through `[runtime.ads_server]`, including a
  standalone `trust-ads-server` protocol crate, fail-closed source-pinned client
  allowlists, loopback Doctor/status/symbol control surfaces, route artifacts
  for external clients, audited write-back through a dedicated ADS server write
  port, online-change symbol-table refresh with `SYM_VERSION` bumps, pyads smoke
  proof tooling, stale snapshot quality reporting, and an ADS server
  communication example;
  production readiness still requires a real TwinCAT external-client lab gate.
- trust-runtime/vscode: added the native `Structured Text: Communication`
  surface with typed `comm.capabilities`, `comm.schema`, `comm.apply`, and
  `comm.test` runtime control responses, one palette-visible Communication
  command, hidden-but-compatible ADS command entry points, capability-backed
  protocol cards, runtime-owned I/O driver setup forms, restart-required apply
  feedback, explicit multi-instance add/update controls, public-docs actions,
  rendered card/status/intent UX polish, bounded Modbus/MQTT connection tests,
  and secret-channel blocking for unsafe remote plain-TCP setup.
- vscode: added the `Structured Text: Network Canvas` first-run topology
  surface with a zero-config simulator path, intent chooser, guided simulated
  device setup, strict-CSP rendering, and real runtime-backed first-green
  states: the canvas now starts the local simulator through the shared runtime
  lifecycle service, keeps runtime/device nodes pending until status and I/O
  values are proven, and renders explicit retry/port/log fallbacks for startup
  failures. The canvas add flow now renders schema-driven setup for the I/O
  driver family (Modbus TCP, MQTT, EtherCAT, GPIO, simulated, and loopback)
  through the runtime `comm.schema`/`comm.apply`/`comm.test` contract, supports
  multi-instance add/update/remove controls without overwriting existing
  drivers, and adds searchable palette/quick-add/template paths, click-to-pin
  focus, keyboard navigation, fault navigation, terminal overflow handling, and
  per-edge communication roles. The canvas renderer is split into stage, graph,
  presentation, and style modules, and the runtime now exposes a Viewer-gated
  `fleet.topology` control surface so VS Code can render Host -> Runtime ->
  Endpoint fleet topology, degraded endpoint faults, and per-edge mesh/runtime
  links from runtime evidence instead of fabricated canvas state. The fleet
  surface now uses a dedicated responsive layout with full-width host/runtime
  cards, wrapped endpoint labels, a non-overlapping status/footer bar, and
  search dimming that never changes raw endpoint health or fault rollups; its
  wide and narrow webview states are now guarded by a headless geometry overlap
  check wired into the VS Code test script. The `fleet.topology` schema now
  reports real discovery-backed peer hosts, service-bound liveness evidence,
  per-endpoint live I/O snapshots, MQTT shared broker nodes, host/container
  facts, and an additive `discovered[]` feed without serializing configured
  secrets. The fleet contract now emits schema version 3 with explicit link
  IDs and per-link roles, an ADS client endpoint for `[runtime.ads]`, and
  configured counterpart nodes/links for ADS targets, Modbus TCP devices, and
  EtherCAT segments so the canvas no longer needs to infer or fabricate those
  graph relationships.
- trust-runtime: added offline `trust-runtime comm schema`, `comm topology`,
  and `comm apply` CLI commands so VS Code can inspect and edit communication
  settings from project files without a running runtime; offline topology now
  includes redacted endpoint params for setup prefill while keeping live status
  non-green until runtime evidence exists.
- trust-runtime: advanced the Communication/Network Canvas setup contract to
  `comm.schema` version 4 with device-first protocol categories, config-home
  metadata, ADS/ADS-server schemas, and universal
  file-writing `comm.apply` behavior for `io.toml`, `runtime.toml`, and ADS
  runtime setup instead of detached paste snippets.
- trust-runtime: made OPC UA wire/server support part of the default runtime
  build and added `comm.schema` availability metadata plus a schema-vs-capability
  regression gate so user-visible default communication protocols cannot be
  advertised without being compiled into the runtime for the current platform.
- trust-runtime: added offline `trust-runtime fleet runtime add` and
  `trust-runtime fleet list` commands for Network Canvas host-level runtime
  setup; the new `fleet.toml` manifest registers sibling runtime projects,
  creates loopback-only control/web endpoints with per-project control tokens,
  and keeps newly-created runtimes offline/configured until they are actually
  started.
- trust-runtime: added `comm.discover` and `comm.browse_symbols` control/CLI
  surfaces for the Network Canvas Browse/Connect flow; the first slice can
  discover ADS targets, truST runtimes, connect-only Modbus TCP endpoints, and
  targeted MQTT brokers, returns an honest server-only warning for OPC UA
  instead of orphaned client candidates, and can browse ADS symbols as a generic
  tree while reusing the existing ADS import/generate-ST candidate payload.
- trust-runtime: completed the Network Canvas Browse/Connect backend by adding
  offline local-global symbol browsing for OPC UA server, ADS server, and
  OpenOT expose pickers; exact selected-tag `ads.import_symbols.apply` writes
  `ads.toml`, cached symbol snapshots, and generated ST through the existing
  ADS import pipeline; EtherCAT endpoint `children[]`/channel browsing now
  come from configured modules; and runtime-origin EtherCAT/GPIO discovery now
  reports real observed bus/modules or GPIO lines without write probes.
- trust-runtime: made ADS client wire support and the ADS server part of the
  default runtime build so normal release binaries include the ADS functionality
  validated with TwinCAT, without requiring custom feature flags.
- trust-runtime: added `trust-runtime check --json/--ci` for VS Code's real
  Check program backend; it validates project config and compiles sources to
  bytecode in memory without writing `program.stbc`, returning structured
  issues for editor rendering.
- trust-runtime: added managed local-runtime lifecycle commands under
  `trust-runtime fleet runtime start|stop|status|logs`; they build registered
  sibling projects before launch, run the local runtime as a background process
  with captured logs, report reachability through the control endpoint, and stop
  through the existing runtime shutdown verb.
- trust-runtime: added the OPC UA client backend for Network Canvas peer links,
  including the `opcua_client` schema, `opcua_client.toml` file writer, targeted
  discovery/test/browse flows, reversible client certificate trust-store CLI,
  offline non-green fleet topology, and scan-cycle read/write polling that only
  reports live status after real OPC UA reads.

- trust-runtime: OpenOT telemetry can now publish real encoded records from a
  configured ST OpenOT producer FB via `[runtime.openot] source = "st-fb"` and
  `producer_instance`, while keeping the existing heartbeat publisher as the
  default.
- trust-runtime: OpenOT ST-FB telemetry now supports multi-PROGRAM authoring by
  draining a configured `producer_instances = [...]` list into one shared ring,
  with deterministic per-program source ids and collision diagnostics.
- trust-runtime: ST OpenOT producer telemetry now hands off multi-record
  transition bursts through the producer FB `ScanRecords` descriptor outputs,
  while fail-closing scans whose published-record delta does not match the
  descriptor count.
- trust-runtime: OpenOT declaration attributes (`{attribute 'oot' := ...}`) now
  lower to the hidden ST-FB producer path for value, state, message, and alarm
  records, including enum-typed state variables whose HIR enum values populate
  StateTransition payloads and definition-file enum sets, with a reactor example
  that emits a UTC-rendered, runtime-clocked typed audit log, definition file,
  and forced-overflow reconciliation artifact.
- trust-runtime/trust-lsp: OpenOT authoring now supports the simple
  operator/regulated edge events `operator-action`, `operator-login`,
  `operator-logout`, and `security-failure`, including authResult symbol
  lowering, bounded `contextRef` bindings, runtime ST-FB emission, renderer
  support, completions, validation, and inlay hints.
- trust-runtime/trust-lsp: OpenOT audited value declarations now emit
  `ParameterChange` records via `audit := 'true'`, with compile-time string
  width and record-size validation, producer baseline rollback on fail-closed
  drops, resolved audit rendering, completions, diagnostics, and inlay hints.
- docs: added a public OpenOT attribute-authoring guide covering the truST
  compiler diagnostics, language-server assistance, generated producer path,
  definition-file generation, value sampling policies, and `[runtime.openot]`
  `source = "st-fb"` runtime configuration.
- examples/docs: added an OpenOT multi-PROGRAM project showing two
  attribute-generated producers drained through `producer_instances` into one
  shared ring.
- Added fail-closed runtime boundary watch envelopes for `trust-harness` protocol
  v2 and `trust-dev` agent harness snapshots. Unresolved watch paths now carry
  structured `status: "error"` entries instead of silently becoming `NULL`.
- Added a runtime boundary fail-closed architecture gate wired into
  `architecture-doctor --full-map` and CI.
- Added a `trust-dev` developer/workbench CLI binary. The first migrated
  command is `trust-dev commit`, and release runtime archives now ship
  `trust-dev` beside `trust-runtime` and `trust-bundle-gen`.
- `trust-dev agent serve` now owns the external agent JSON-RPC server. The
  legacy `trust-runtime agent serve` entrypoint remains as a deprecated
  forwarding alias during the product/workbench CLI split.
- `trust-dev docs` now owns ST API documentation generation, and
  `trust-runtime docs` remains as a deprecated forwarding alias during the
  product/workbench CLI split.
- `trust-dev test` now owns ST test discovery/execution, and `trust-runtime
  test` remains as a deprecated forwarding alias during the product/workbench
  CLI split.

### Removed

- trust-runtime: removed the abandoned digital-twin/world proof stack, including
  the Scena/URDF robot-arm dependencies, `RobotP3MinimalArm` built-in function
  block, world-smoke fixtures, and transform-handoff lint script. The release
  keeps the normal simulator/runtime surfaces and no longer ships a
  digital-twin or trust-twin runtime path.

### Changed

- vscode/docs: documented the shipped product contract for the truST sidebar,
  Live Values, Devices & Connections, examples, libraries, HMI, and visual
  editors; public guides now use the current Compile, Live Values, and direct
  `+ Add` workflows instead of retired Runtime Panel, Check, and Edit-mode
  wording.
- branding: replaced font-dependent truST wordmarks and product icons with
  self-contained path-based assets across the public documentation, runtime Web
  IDE, and packaged VS Code extension.
- vscode: normalized simulator copy across the sidebar, Devices & Connections,
  Live Values, new-project template, and bundled examples so the local runtime
  appears as `Simulator` and its local endpoints appear as `Simulated I/O` /
  `Loopback I/O` instead of mixing `truST runtime`, `local simulator`, and bare
  protocol names.
- vscode: normalized online-change wording across the sidebar command, command
  palette, language-model tool, toasts, and runtime feedback so the user-facing
  verb is consistently `Update` instead of mixing update and hot-reload terms.
- vscode: clarified Examples Gallery badges and filters so hardware-required
  starters use the shared warning role while category chips display readable
  labels such as `ADS`, `Raspberry Pi`, and `EtherCAT`.
- vscode: clarified Libraries row actions and counts by renaming read-only
  library access to `View source`, showing versioned update buttons such as
  `Update to 0.1.0`, and using singular/plural symbol counts consistently.
- vscode: aligned Devices & Connections runtime setup panes with the shared
  inspector breadcrumb so `Set up runtime` and `Connect existing runtime` read
  as part of the same right-pane shell.
- vscode: changed Devices & Connections endpoint edit breadcrumbs from role
  badge wording such as `CLIENT edit` to task wording such as `Edit Modbus TCP`.
- vscode: styled the open project name in the truST sidebar as a project
  identity row with a folder icon so lowercase project names no longer read as
  placeholder copy.
- vscode/runtime: removed the unfinished 3D twin prototype, typed LM tool,
  packaged WASM/assets, scene-page HMI contract, and related activation/build
  and test wiring from the shipped product surface so the PLC IDE focuses on the
  accepted Run, Devices & Connections, Live Values, HMI, and visual-editor
  workflows.
- `trust-harness` now defaults to JSON protocol version 2 and accepts
  `--protocol-version 1` or `TRUST_HARNESS_PROTOCOL_VERSION=1` for the legacy
  watch-value map shape during migration.
- `trust-runtime commit` is now a deprecated compatibility alias that forwards
  to `trust-dev commit`, preserving the existing command while the product
  runtime CLI is split from workbench/dev commands.
- Public docs and terminal capture scripts now point agent automation workflows
  at `trust-dev agent serve`, with `trust-runtime agent serve` documented as a
  compatibility alias.
- Public docs, CI templates, flake probes, and terminal capture scripts now
  point ST test and ST documentation workflows at `trust-dev test` and
  `trust-dev docs`.
- `trust-runtime test`, `trust-runtime docs`, `trust-runtime commit`, and
  `trust-runtime agent serve` warnings now include the `trust-dev` canonical
  command and a removal-not-before date of 2026-10-05; actual alias removal
  requires a separate behavior-change release note.
- Local Rust test recipes now use the `mold` linker on Linux when it is
  installed, `just test-all` and CI no longer run `complete_program` twice, and
  `just test-hir-fast` provides a focused HIR refactor loop before the final
  full workspace gate.
- VS Code extension tests now build and resolve the required local
  `trust-debug` adapter and reuse the Linux fast-link Rust wrapper for local
  Rust build steps.
- OSCAT OOP now uses the settled `OscatOop` dependency alias,
  concrete types in simple examples, `PidGains` plus `Configure(...)` for PID
  setup, clearer PID/calendar/astronomy names, and example coverage for
  `Snapshot()` and configuration error handling.
- MQTT I/O now supports explicit TLS/mTLS configuration instead of carrying
  unused default TLS dependencies: `io.params.tls = true` requires
  `tls_ca_path`, optional `tls_client_cert_path`/`tls_client_key_path` enable
  client authentication, `mqtts://` and `ssl://` broker schemes imply TLS,
  Linux release builds use vendored OpenSSL for the selected native TLS backend,
  and remote plaintext brokers still require `allow_insecure_remote = true`.
- GPIO I/O now defaults new configurations to the Linux GPIO character-device
  backend (`libgpiod`/`chardev`) while keeping legacy `sysfs` selectable, and
  setup surfaces expose the GPIO chip path alongside the existing line mapping
  help.
- The default OSCAT OOP example catalog test now keeps the all-example folder,
  README, source-layout, and pattern checks in the normal Rust suite while
  moving the expensive 98-project `trust-runtime test --project` sweep behind
  an explicit ignored gate with per-project progress, child PID/elapsed
  reporting, and timeout diagnostics.
- The runtime VM benchmark gate now records per-fixture metrics, confidence
  spread, optional baseline compare deltas, and `quick-low-noise` /
  `full-low-noise` repeated-run profiles in its artifacts.
- The runtime VM malformed-bytecode fuzz smoke gate now records progress-visible
  logs and summary artifacts for deterministic bytecode mutation coverage.
- The runtime VM default-switch readiness ledger now records production-guard,
  differential, malformed-bytecode fuzz, benchmark, residual-risk, and rollback
  evidence without pretending blocked performance thresholds are ready.
- VM function and method local initialization now populates VM frame slots
  directly for initialized locals, static locals, and function-block local
  member overrides instead of creating a temporary runtime storage frame on the
  native-call path.
- trust-runtime: documented the stable signed-integer materialization policy:
  representable `SINT`/`INT`/`DINT`/`LINT` results retain their exact value and
  destination tag, while range overflow fails before storage instead of
  wrapping, saturating, truncating, or substituting a value.

### Fixed

- trust-runtime: typed process-image `REAL` and `LREAL` output bindings now
  reject non-finite values transactionally before changing `%Q`/`%M` bytes or
  committing output to a physical driver.
- trust-runtime: finite `REAL` `EXP` and `EXPT` calls now fault before
  assignment when their basic-single result would be NaN or infinity instead
  of storing a non-finite process value.
- trust-runtime: finite `REAL` binary arithmetic now faults before assignment
  when its basic-single result would be NaN or infinity instead of storing a
  non-finite process value.
- trust-runtime: `simulation.toml` now rejects NaN and infinite coupling
  thresholds before activating the configuration instead of silently selecting
  a coupling branch.
- trust-debug: managed Live Values writes and forces now parse declared
  `REAL`/`LREAL` values semantically and reject NaN, infinities, width overflow,
  and raw non-finite bit encodings before changing process-image or force state.
- trust-hir: cross-file POU imports now preserve `STRING[n]` and `WSTRING[n]`
  capacities derived from project constants, so equal-capacity `VAR_IN_OUT`
  bindings no longer fail as bounded-to-unbounded mismatches.
- trust-runtime: file-backed retain snapshots now reject non-finite
  `REAL`/`LREAL` values recursively on save and load, preserving the last good
  durable image and preventing partial application of an invalid snapshot.
- trust-runtime: mesh subscriptions now reject integer values outside the
  subscribed IEC type's range and reject floating-point conversions that would
  produce a non-finite PLC value instead of silently wrapping or storing
  infinity.
- trust-runtime: OPC UA client `Float`/`Double` input samples now reject `NaN`
  and positive or negative infinity before cache acceptance, mark the affected
  point faulted, and leave the PLC target unchanged.
- trust-ads-core/trust-runtime: ADS `REAL`/`LREAL` client reads, notifications,
  arrays, and server writes now reject `NaN` and both infinities before cache
  acceptance, server write queuing, or PLC storage mutation.
- trust-hir/trust-runtime: compile-time bounded `STRING[n]` and `WSTRING[n]`
  literal checks now count Unicode scalar values; call binding rejects
  width-changing `VAR_IN_OUT`, and function/function-block output copy-back
  honors the receiving declaration's capacity. Metadata-free legacy v1.1
  bytecode now fails closed when a local string output target's declared type
  cannot be recovered; compiler and runtime must be upgraded together for this
  boundary.

- trust-runtime: in-process warm and cold restarts now preserve monotonic
  runtime time while reinitializing timer and task baselines at the preserved
  value, preventing restart from rewinding simulation time or charging
  pre-restart elapsed time to recreated timer instances.
- trust-runtime: bounded MQTT and Modbus input workers now preserve the typed
  error from a completed protocol read instead of replacing MQTT connection
  context and Modbus exception responses with a generic unavailable-snapshot
  transport error; MQTT reads without a fresh snapshot report `IoFreshness`.
- trust-runtime: watchdog faults before physical output commit no longer
  re-send the pending process-image output when no safe-state outputs are
  configured; the resource faults visibly without issuing a driver write.
- trust-runtime: TOF and TOF_LTIME now hold `ET = PT` after the off-delay
  expires while `IN` remains false, matching IEC 61131-3 Ed.3 Figure 15(c),
  until a later true-input scan rearms the timer.
- ci: release tests now tolerate loaded matrix runners for MQTT/Modbus
  scan-bound timing, runtime fail-closed state propagation, Windows index-cache
  mtime fixture updates, and Windows ADS diagnostics fixture line endings.
- ci: Modbus write-failure policy tests now use a deterministic local Modbus
  exception response instead of assuming a fixed localhost port is closed on
  every hosted runner.
- trust-runtime: Web IDE workspace path validation now rejects Windows rooted
  paths with the same absolute-path denial used on Unix, keeping release tests
  and browser-facing file-scope errors consistent across platforms.
- release: Windows packaging now installs Strawberry Perl before building
  vendored OpenSSL so release artifacts use the same prerequisites as CI tests.
- ci: Ubuntu release test jobs now serialize all-target Rust linking to avoid
  hosted-runner `rust-lld` bus errors before the test suite starts.
- trust-runtime: Windows `trust-runtime.exe` now links with a larger stack so
  CLI invocations such as ADS route and validation commands do not overflow the
  default Windows executable stack during release tests.
- ci: Windows test jobs now install a full Perl toolchain before building
  vendored OpenSSL so cross-platform release gates do not fail on missing Perl
  modules.
- ci: macOS release tests now tolerate `/private/var` temp-path aliases and
  filesystems that reject invalid non-UTF8 fixture names.
- ci: the regression-test-first guard now recognizes Rust module files named
  `tests.rs` as focused regression-test evidence.
- docs: public docs release gates now include the checked-in VS Code screenshot
  inventory entries, current regenerated PlantUML SVGs, and search-regression
  scoring for the intended VS Code, runtime-panel, and observability pages.
- build/source: `trust-runtime` now uses pinned public Git dependencies for the
  OpenOT Rust crates, so fresh clones and GitHub source archives can build
  without a pre-existing sibling `../open-ot-ref` checkout; the source-build
  guide now documents that the checkout script is only needed for OpenOT IEC
  examples and tests that read the ST source package.
- vscode: Ladder visual editor right pane now matches the other visual editors'
  empty-property treatment and no longer repeats passive Rungs/Pan guidance that
  is already shown in the status bar.
- vscode: Statechart visual editor now keeps added states and transition labels
  inside the visible canvas at normal sidebar widths and keeps animation CSS out
  of node text.
- examples: the OpenOT multi-program example now includes a simulated `io.toml`
  so the normal project Compile/Run workflow can validate and launch it.
- vscode: native Settings now contributes product-language `trust.*` setting
  keys with legacy `trust-lsp.*` fallback, so first-user Settings labels no
  longer render under the internal `Trust-lsp` heading.
- vscode: Compile now distinguishes missing `trust-runtime` binaries from
  incompatible runtime check-report versions, with short product-language
  recovery text for Runtime path/update actions.
- vscode: the truST sidebar now treats unreadable project manifests as a
  repair/no-project state instead of showing Target/Compile/Start controls for a
  project that cannot load.
- trust-plcopen: PLCopen import now accepts Structured Text POUs that include
  benign body-level metadata such as `addData`, while still rejecting graphical
  or unknown non-ST bodies with named diagnostics.
- trust-syntax: ST exponentiation now follows IEC 61131-3 Ed.3 Table 71
  left-to-right equal-precedence evaluation, and unclosed function-call
  argument lists now report the missing `)` diagnostic.
- trust-dev: `trust-dev test` now discovers `.st` and `.pou` files with
  mixed-case extensions under literal project paths containing glob
  metacharacters, and `trust-dev commit` now uses OS-safe scoped git pathspecs
  so unrelated pre-staged files outside the PLC project are not committed.
- vscode: the MQTT Add form now exposes the runtime `On error` policy so users
  can choose scan-tolerant `warn` handling for local broker startup instead of
  editing `io.toml` by hand for the successful MQTT use-in-ST path.
- trust-ide/trust-lsp: rename now exposes structured refusal reasons and rejects
  unsafe renames that would capture references through local shadows or create
  imported-symbol origin conflicts instead of returning behavior-changing edit
  sets or silent no-op protocol responses.
- trust-lsp: references and workspace-symbol requests that stream partial
  results now return an empty final response instead of duplicating the
  streamed locations or symbols in the final payload.
- trust-lsp: cancelled diagnostics requests now fail closed at the protocol
  boundary: pull diagnostics return `ContentModified`, and push diagnostics
  skip publication instead of transiently clearing real errors with an empty
  successful diagnostics set.
- trust-lsp: inline runtime values now bound runtime-control connect/read/write
  waits and return an empty inline-value result on timeout or transport failure
  instead of parking an LSP worker on a silent endpoint.
- trust-lsp: LSP position handling now advertises UTF-16 and maps hover ranges,
  incremental edits, diagnostics, refactor edits, and semantic-token
  start/length fields through UTF-16 code units so supplementary-plane
  characters no longer desynchronize editor buffers.
- trust-lsp: EOF positions on documents that end with a newline now round-trip
  through the line index without panicking, fixing inline-value requests whose
  range ends at the final empty line.
- trust-lsp: HMI TOML pull diagnostics no longer compile a runtime from
  disk-only ST sources, so open buffers are not contradicted by stale
  `unknown binding path` warnings; the pull path now keeps cheap TOML parse and
  local widget-property diagnostics only.
- trust-ide/trust-lsp: semantic tokens, document highlight, references,
  struct-field go-to-definition, and call hierarchy now avoid repeated
  syntax-tree and whole-workspace scans on interactive paths, keeping
  large-workspace editor requests bounded by the new perf locks.
- trust-runtime: VM process-image stores and function/function-block parameter
  copy-in now materialize HIR-legal numeric and bit-string widening into the
  declared target tag, so assigning `INT` values into `REAL`/`DINT` storage no
  longer leaves narrow runtime tags that compute integer division or overflow at
  the wrong width; bytecode VM function-block `STRING[n]` input and in-out
  parameter copy-in now also honors the declared bound instead of storing
  over-length strings in the FB instance.
- trust-runtime: runtime writes to declared subrange storage now fail visibly
  instead of committing out-of-range values across bytecode VM assignment,
  function-block/function parameter copy-in, dynamic `REF` writes, HMI/control
  writes, and retain reload.
- trust-runtime: bytecode encoding now rejects manually-mutated `VAR_ACCESS`
  bindings that shadow globals, matching the source-level HIR diagnostics
  instead of silently rebinding bare global names to access-map targets.
- trust-runtime/trust-hir: OpenOT authoring now reports explicit `sourceid`
  ownership collisions consistently in HIR and runtime validation, and
  `CompileSession` fails closed on OpenOT instrumentation validation errors
  instead of building bytecode from uninstrumented source with zero telemetry.
- trust-lsp: closing an unsaved file-backed dependency now restores disk truth
  for cross-file analysis, memory-budget eviction no longer removes closed
  files from the semantic project, and the persistent index cache verifies disk
  content before accepting same-size/same-second metadata hits.
- ci/build: Unix runtime signal-shutdown helpers and their unit-test fixtures
  are now cfg-gated so Windows warning-deny and pre-push `trust-lsp` test
  compile checks do not fail on unused Unix-only signal code.
- trust-hir/runtime: `REF()` now rejects function and method return variables
  before lowering, preventing source programs from persisting frame-local return
  slot references into longer-lived storage.
- trust-runtime: bytecode validation now rejects crafted modules that persist
  frame-local reference values into longer-lived storage or mix multiple baked
  instance owners in one POU body, and catches operand-stack underflow,
  non-empty POU exit stacks, known BOOL operands to arithmetic opcodes, and
  incompatible constant stores to typed `STORE_REF` targets, invalid parameter
  direction metadata, and expression arguments bound to target-only `OUT`/
  `IN_OUT` parameters, legacy `CALL` opcode bodies, and inconsistent operand
  stack depths at control-flow joins before malformed bytecode can be loaded or
  executed.
- trust-runtime: bytecode lowering now fails closed for unsupported statement
  paths instead of silently emitting `NOP`; label-only statements remain the
  explicit no-op path.
- vscode: visual ST companions and runtime wrappers now map raw visual-editor
  direct I/O operands through generated `AT` globals instead of emitting raw
  `%I`/`%Q`/`%M` references inside function-block bodies, so the bundled visual
  examples keep compiling under the fail-closed bytecode lowering contract.
- trust-runtime: hardened scheduler and process-image failure handling so
  resource-cycle panics become visible faults instead of silently killing the
  scan thread, enabled watchdog policies arm VM execution deadlines and reject
  zero-timeout scan bricking, Modbus/EtherCAT/MQTT non-fault I/O policies
  degrade health without faulting the resource, requested stops apply safe
  outputs and surface retain-save failures, and direct process-image writes are
  capped at a 16 MiB per-area limit.
- trust-runtime: moved Modbus TCP and MQTT wire/broker waits behind
  protocol-owned workers so scan-cycle `read_inputs` and `write_outputs` use
  bounded snapshot/handoff state instead of blocking on delayed protocol
  responses, reconnect backoff, or missing fresh broker payloads.
- trust-runtime: bounded EtherCAT hardware construction failures after the
  ethercrab `PduStorage` split boundary so a missing/unavailable adapter reports
  a terminal driver-rebuild-required state instead of repeatedly allocating
  storage during deferred retry windows.
- trust-runtime: tightened runtime correctness boundaries by preserving TP
  elapsed time at pulse expiry, enforcing bounded `STRING[n]` ingress for
  assignments, retain reload, HMI writes, and process-image writes, rejecting
  non-finite REAL/LREAL ingress through HMI, string conversion, raw typed I/O,
  Modbus maps, and MQTT maps, releasing ADS notifications across symbol-version
  refresh/drop paths, treating rejected OPC UA point writes as point faults
  without reconnect churn, and validating online-change task metadata before
  resizing process images.
- ci: restored OpenOT sibling checkout in the Nightly Reliability and HMI Long
  Soak workflows so path dependencies resolve in long-running reliability
  gates again.
- ci/release: pinned OpenOT checkout, added release tag/CI preflight,
  changelog-backed release bodies, SHA256 checksums, cargo-deny/cargo-audit
  supply-chain gates, and conformance/architecture-safety coverage in the
  release gate report.
- docs/tooling: made `AGENTS.md`, `CLAUDE.md`, and repo skills the shared
  versioned agent rule source, added Claude/Codex permission guardrails, and
  fixed stale HMI and IEC instruction paths.
- vscode: the Libraries symbol browser now searches and pages through the full
  library symbol list, shows declaration detail for the selected symbol, and
  can insert or copy a declaration snippet instead of rendering a capped list
  of inert chips.
- trust-runtime/vscode: GPIO setup fields are now conditional on the selected
  backend, so the default libgpiod form hides legacy Sysfs base settings while
  sysfs mode hides the libgpiod chip field.
- vscode: Devices & Connections server inspectors now show the server endpoint
  and exposed globals after OPC UA/ADS expose flows, preserve ADS pinned-client
  config while reapplying expose changes, and carry ADS live client-count
  evidence through to the inspector when available.
- vscode: Devices & Connections runtime nodes now project the shared selected
  run target back onto the canvas and inspector with a neutral `Run target`
  badge, so `Set as run target` gives persistent visible feedback.
- vscode: visual editors now distinguish `Preview ST` from `Generate ST`, with
  tooltips that state whether the action only previews generated code or writes
  a companion `.st` file.
- vscode: Devices & Connections discovery now treats reachable `Online`
  runtimes as valid EtherCAT/GPIO scan origins instead of leaving hardware scans
  disabled until a debug-attached `running` state appears.
- vscode: visual editors now share keyboard/canvas chrome: Blockly toolbox
  focus uses the truST focus ring, and SFC/Statechart expose the same themed
  canvas zoom controls as Devices & Connections.
- trust-runtime/vscode: stopped/configured Devices & Connections nodes now use
  one consistent "runtime is not running" state phrase across runtime and
  endpoint inspectors instead of mixing project-file and live-health wording.
- vscode: Devices & Connections now preserves the stopped project runtime when
  a managed local runtime starts, so the topology updates state in place instead
  of silently removing a runtime node the user just saw.
- vscode: HMI Process SVG previews now inherit the shared truST theme roles,
  so generated process diagrams no longer render as a white slab inside the
  dark VS Code theme while remaining readable in Light Modern.
- trust-runtime: Browser HMI cards now suppress standalone unit rows for
  widgets whose renderer already shows the unit inline, avoiding duplicate
  `%`/`bar` lines under setpoint and readback values.
- trust-runtime: Browser HMI now separates passive header status chips from
  clickable header actions, including moving the theme toggle into the action
  group and giving actions visible button affordance.
- trust-runtime: Browser HMI alarm banners and alarm tables now use configured
  alarm labels while keeping the raw signal path as tooltip detail, so operator
  alarms do not expose variable-name labels when a human title exists.
- vscode/trust-debug: starter projects and bundled examples now include a
  native `truST Simulator` launch configuration, and Structured Text debug
  stack frames display workspace-relative source names while keeping absolute
  paths for navigation, so the Run and Debug view no longer shows
  `No Configurations` or long local paths during first-use debugging.
- vscode: Devices & Connections now exposes a first-class `+ Add` toolbar
  action that opens the device/connection picker for the selected/default
  runtime, instead of teaching first-time users to discover the primary add
  flow through Edit mode.
- vscode: Devices & Connections discovery now disables EtherCAT/GPIO hardware
  scans with an inline reason until a running runtime is selected, and empty
  scan results explain concrete recovery checks instead of vague runtime
  plumbing.
- vscode: Browse tags/nodes now clears stale selections when browse results
  disappear and keeps Add/write controls neutral-disabled with a visible reason
  when there is nothing valid to add.
- vscode: OPC UA browse authentication warnings now include an inline
  `Edit credentials` recovery action that reopens the connection form with the
  failed endpoint values prefilled.
- vscode: SFC diagrams now use clearer IEC-style initial-step and transition
  markers so the visual editor reads as SFC instead of a generic flowchart.
- vscode: Ladder diagrams now render symbolic names above mapped `%I/%Q/%M`
  addresses and use neutral edit-time strokes, reserving status colour for live
  power-flow.
- vscode: visual editor right panes now share the same Tools/Edit/View section
  order, with Fit View in the pane and Show Code labeled as a generated-ST
  preview instead of a save action.
- vscode: invalid SFC, Statechart, and Blockly visual-editor files now show an
  `Open as text` recovery action so users can fix malformed JSON directly in
  the default VS Code editor.
- vscode: Blockly now uses a truST-themed workspace, toolbox, flyout, and muted
  block palette derived from the shared webview design tokens instead of raw
  default Blockly hues.
- vscode: Advanced integration cards in the Devices & Connections Add picker
  now use intentional badges (`MESH`, `OT`, `RT`, `CLOUD`) instead of truncated
  protocol fragments.
- vscode: Simulated and Loopback endpoint node titles now use matching local I/O
  naming, leaving the `I/O` role label to the node band.
- vscode: Boolean fields in Devices & Connections protocol forms now render as
  native checkboxes with On/Off labels instead of dropdowns.
- vscode/runtime: generated protocol-form copy now preserves acronym casing in
  empty states, avoids schema-defaults wording for saved secrets, and the
  Libraries surface pluralizes symbol counts correctly.
- vscode: Devices & Connections now lands on the saved endpoint after a
  successful add-device Save, keeps the success result visible, and resolves
  the selected node from the refreshed topology when the apply response omits
  an instance id.
- vscode: Devices & Connections draft/advanced mesh fabric nodes now use one
  muted draft treatment, with an opaque label knockout and separate `DRAFT`
  chip so dashed wires never read as live state or run through text.
- vscode: Compile diagnostics now use the user-facing `truST` Problems source
  label instead of the stale `trust check` name, and Compile resolves the
  nearest `trust-lsp.toml` from the active file before falling back to the
  workspace root.
- vscode: Devices & Connections inspectors now render lifecycle details as a
  single Title-Case `State` row, hiding backend `mode`/`status`/`detail`
  duplicates unless `Mode` adds real target context such as Simulator or
  Managed.
- vscode: Blockly editor status now counts all visible Blockly blocks from the
  live workspace instead of reporting only serialized top-level stacks.
- trust-debug/trust-runtime: debug Variables and Watch/Evaluate output now show
  function block/program instance type names instead of leaking raw
  `Instance(id)` runtime handles.
- trust-runtime: ST test assertion failures now format expected and actual
  values as user-facing ST values instead of leaking Rust enum wrappers such as
  `Int(1)` or `Real(1.5)`.
- vscode: refreshing the native Test Explorer now preserves pass/fail state for
  unchanged Structured Text tests instead of clearing the gutter/status evidence
  immediately after a run.
- vscode: Live Values now shows a `Forced (N)` filter chip when overrides are
  active, letting operators list only forced points before releasing or
  inspecting them.
- vscode: Live Values now explains the force ceremony in-panel, distinguishing
  immediate simulator forcing from the managed/remote `Arm force` flow so the
  two target types do not look inconsistent.
- vscode: Devices & Connections and Live Values now reserve red alarm styling
  for real failures: stopped local runtimes render as neutral guidance, local
  socket paths are humanized, and viewer-role permission guidance uses the
  warning treatment instead of the danger treatment.
- vscode: shared webview theme tokens now handle VS Code High Contrast and
  High Contrast Light explicitly, so visual editors, Devices & Connections,
  and other product webviews no longer render as dark/navy or white forced-colors
  islands under high-contrast captures.
- vscode: Devices & Connections now keeps simulator-mode topologies under one
  `Simulator` runtime and refits populated remote/auth-failure graphs, so the
  canvas, summary, and status bar no longer contradict each other during GPIO
  scans or token-error recovery.
- vscode: the extension test runner and development binary resolver now honor
  `CARGO_TARGET_DIR` when locating built `trust-lsp`, `trust-runtime`, and
  `trust-debug` binaries, keeping isolated remote-builder gates aligned with
  warmed target-cache builds.
- vscode: Live Values now renders row Write/Force controls as quiet secondary
  actions, marks active forces with amber warning treatment instead of success
  green, and keeps forced rows readable by removing the stale editable value
  control while showing a disabled Write reason plus Release.
- vscode: Structured Text snippets now prefer identifier-friendly completion
  aliases while preserving the readable hyphenated aliases, making snippet
  completions deterministic in the extension test harness.
- trust-runtime/vscode: Live Values snapshots now come from one final
  post-logic runtime scan with per-snapshot force markers and a visible scan
  number, preventing stale rows from being decorated with newer force state.
- trust-debug/vscode: running-simulation updates now suppress pre-scan reload
  I/O events and the conveyor example no longer overlaps bit and word outputs,
  keeping Live Values update evidence logically consistent.
- vscode: Live Values now auto-expires short success feedback such as released
  forces while leaving standing-state banners, including active forces and
  permission-denied guidance, visible until the state changes.
- trust-runtime/vscode: Live Values rows now carry runtime-owned source
  provenance when available, so mapped Modbus/MQTT rows show the driver
  endpoint or topic directly in the panel instead of relying on ST comments.
- vscode: Live Values now labels local control-socket targets as
  `Local runtime (control socket)` while keeping the full socket path in the
  tooltip, so temporary socket filenames no longer leak into the panel header.
- vscode: Live Values now includes a compact DEC/HEX/BIN display toggle for
  word-like numeric rows, rendering IEC-style `16#` and `2#` literals without
  changing write/force parsing.
- trust-debug/vscode: Live Values now preserves configured I/O value types from
  the runtime snapshot, so REAL, TIME, and fixed-length STRING rows render as
  typed values instead of raw DWORDs/bytes and local writes accept `1.5`,
  `T#250ms`, and text values.
- testing: VS Code UX evidence PNG hygiene now strips color/text/time chunks and
  hashes decoded pixels, so duplicate screenshots cannot bypass review by
  differing only in PNG metadata. The runtime I/O cycle snapshot test now
  asserts the post-output-commit snapshot contract with a bounded timeout
  instead of hanging broad gates when a snapshot is missing.
- vscode: Live Values now explains program-driven outputs and memory rows in
  the panel and on disabled Write controls, pointing users to Force for
  simulation overrides instead of leaving greyed actions unexplained.
- vscode: Live Values now maps disconnected I/O transport failures to
  user-facing reconnect guidance instead of surfacing raw socket or cancellation
  text in the panel.
- vscode: HMI Preview now refreshes through the shared descriptor debounce when
  HMI descriptor or process SVG files are edited in an open workspace, keeping
  the panel current without waiting for a separate manual refresh.
- vscode: Live Values now clears stale connected/value rows as soon as a
  Structured Text debug session stops or disconnects, keeps remote
  disconnected targets labelled `Not connected` instead of fabricating
  `Running`, and retries transient runtime-busy write/force/release requests
  before surfacing a user-visible failure.
- vscode: Devices & Connections now only labels a fleet host as `This computer`
  when its reported identity also matches a local interface, so remote runtimes
  on machines with lab-builder hostnames render as generic `Computer <address>`
  headlines instead of misleading local-machine labels.
- vscode: Add Device forms now clear stale field validation and hide stale
  apply banners as soon as the user edits the rejected field, so corrected
  example values no longer remain marked invalid before the next Save.
- vscode: the `Update running simulation` command now uses the same compile
  and config gate as the sidebar button, returning the disabled-with-reason
  message instead of attempting hot reload from an invalid project.
- vscode: Live Values now keeps long signal names inside a capped name column,
  preserves VALUE/TYPE/STATE/ACTIONS alignment at narrow widths, and exposes the
  full signal/address in the row tooltip instead of letting names overlap values.
- vscode/trust-runtime: Devices & Connections now renders ADS server allowed
  clients as readable pins such as `127.0.0.1.1.100 (from 127.0.0.1)` and no
  longer sends the raw ADS client allowlist objects through the topology
  inspector payload.
- vscode/trust-runtime: Live Values now receives runtime role/capability data
  from the control channel, disables viewer-role Write/Force/Release controls
  before any unsafe attempt, and shows a visible engineer-token recovery reason;
  remote auth recovery now distinguishes missing tokens from rejected tokens,
  managed Stop clears the selected-runtime status consistently, and Update
  running simulation reports success or compile-failure recovery without leaking
  raw source paths.
- vscode: the sidebar Start, Debug, and Update-running-simulation controls now
  stay visible but disable with a plain recovery reason when compile or
  `runtime.toml` errors mean the action cannot succeed.
- trust-runtime: advanced communication setup schemas now use one
  configured-only note for Mesh, Realtime T0, and Runtime cloud so draft links
  cannot read like live runtime proof.
- examples: OPC UA server and ADS server expose examples now update the exposed
  globals from ST each scan so protocol proof can show value changes instead of
  reading only initializer constants.
- vscode: tightened the first-run/sidebar UX by restoring the distinctive
  stacked `tru/ST` activity icon, replacing the Project bucket with one compact
  sidebar control surface, making Start from example the no-project primary
  path, exposing Libraries as a first-class destination, renaming project
  validation to Compile, and adding the Examples Gallery surface for curated
  runnable starters.
- vscode: Examples Gallery filters now combine hardware requirements and
  categories and include a clear no-match reset state, so users can find a
  runnable starter without scrolling a flat list.
- trust-debug/vscode: internal debug-adapter lifecycle trace now uses internal
  DAP events instead of the visible Debug Console, so first-time debugging no
  longer exposes raw `[trust-debug]` backend logs.
- vscode: Devices & Connections runtime inspectors now show a direct
  `Set auth token` recovery action for remote runtime authentication failures,
  reusing the SecretStorage-backed token command instead of leaving users to
  infer the fix from Settings.
- vscode: Run target labels now keep the remote control port, so two runtimes on
  the same host no longer appear as identical `127.0.0.1` entries.
- vscode: shortened bundled starter descriptions in the Start from example
  picker so hardware notes and first-run purpose text remain readable instead
  of clipping in the clean-profile onboarding flow.
- trust-runtime: HMI schema generation now marks only explicitly allowlisted
  `[write]` targets as writable/read-write, so browser HMI slider/toggle
  controls are enabled for safe configured writes without making the whole HMI
  writable.
- trust-runtime/vscode: Devices & Connections endpoint Disable now preserves I/O
  drivers as disabled instead of deleting them, skips disabled drivers at
  runtime/deploy validation, keeps disabled endpoints visible and non-green in
  `fleet.topology`, and exposes a schema-driven Disable action in the endpoint
  inspector with explicit re-enable guidance.
- trust-runtime/vscode: MQTT setup validation now reports missing TLS CA paths,
  mismatched client certificate/key paths, TLS-field misuse, and broker-format
  errors against the relevant setup fields, uses an MQTT port example for
  broker errors, and treats the schema's empty TLS ALPN default as empty so
  Devices & Connections can highlight the exact field instead of showing only a
  generic blocked-save message; MQTT broker Test results now use honest
  broker-port reachable/not-reachable wording, keep unresolved broker errors
  attached to the Broker field, and render saved MQTT broker nodes from fleet
  topology instead of leaving publish/subscribe links hidden. The ADS/OPC UA
  server expose picker now strips topology-only evidence fields before saving,
  so selected globals are persisted instead of being rejected by `comm apply`.
- vscode/trust-runtime: Devices & Connections endpoint summaries now expose the
  existing Communication Test action for testable endpoints, translate
  structured OPC UA client endpoint failures into user-facing recovery text,
  classify rejected OPC UA anonymous logins as authentication-required recovery,
  and summarize configured connections instead of rendering raw JSON blobs.
- vscode: the truST sidebar now distinguishes an open non-truST folder from the
  no-folder first-run state and offers an explicit "Initialize truST here"
  action instead of showing the generic create/open/example welcome.
- trust-lsp/vscode: Move Namespace now asks the LSP for the computed workspace
  edit, applies it through VS Code in a text-edits-before-cleanup order,
  pre-creates missing target files, and removes an empty pre-created target on
  failure, so VS Code no longer reports success after leaving an empty target
  file and unchanged references.
- vscode: the namespace refactor command is now labeled "Move Structured Text
  Namespace" in the Command Palette and editor code action, so the advanced
  refactor no longer appears as the ambiguous bare "Move Namespace" command.
- vscode: stale package contribution and legacy Live Values titles now use
  "Devices & Connections" and "Live Values" instead of the retired "Network
  Canvas", "Runtime Panel", or "Structured Text Runtime" wording.
- vscode: starting the simulator from the truST Run card no longer auto-opens
  VS Code's Debug Console with raw adapter logs, keeping the first-run running
  state focused on product surfaces.
- vscode: attach-mode debug sessions no longer auto-open VS Code's Debug
  Console with raw adapter logs, keeping managed-runtime Start/Connect flows
  focused on Devices & Connections and Live Values.
- vscode: managed-runtime Start/Stop in Devices & Connections now refits when
  endpoint nodes appear or disappear, and managed runtime logs are summarized as
  readable event lines instead of raw JSON records.
- vscode: Devices & Connections now uses the shared truST webview product
  chrome for its Add pane, so visual-editor right panes and the device setup
  pane resolve the same title, panel, input, and button theme treatment across
  Dark+, Light+, and High Contrast themes.
- vscode: Devices & Connections protocol setup forms now use the same shared
  truST form chrome as the rest of the canvas, and starting a new add/edit
  workflow clears stale validation banners so one protocol's failed save cannot
  leak into the next form.
- vscode: Devices & Connections add/edit validation now shows a concise
  field-issue banner that fits the toolbar, and saved Modbus endpoints render
  as "Modbus TCP" in the graph instead of leaking the raw `modbus_tcp` driver
  id.
- vscode: Devices & Connections endpoint edit forms no longer reset in-progress
  field changes when background topology refreshes deliver the same endpoint
  parameters with a new object identity.
- vscode/trust-runtime: Devices & Connections EtherCAT Browse channels now
  saves selected PDO channel paths back to the EtherCAT driver config instead
  of incorrectly routing them through the ADS tag-import flow.
- vscode: Devices & Connections endpoint inspectors now render user-facing
  status labels such as "Configured" instead of raw backend health IDs such as
  `configured_policy`.
- vscode: visual-editor right panes now use surface-specific titles such as
  "SFC editor" and the shared truST section/button chrome, removing the generic
  "Editor tools" title and private Ladder/Blockly panel styling that made those
  editors look like separate products.
- vscode: Blockly toolbox category labels now use the shared normal foreground
  theme token instead of accent-button text, keeping the toolbox readable in
  Light+, Dark+, and High Contrast themes.
- vscode: Live Values now uses the same shared truST product theme roles as
  Devices & Connections and the visual editors instead of a private color-token
  layer, so cross-surface color checks compare equivalent UI roles.
- vscode: Devices & Connections, SFC, and Statechart canvas grids now use the
  same shared truST grid-line role instead of separate raw VS Code color
  variables.
- vscode: Devices & Connections node-summary actions and shared React Flow
  canvas controls now use the shared truST product chrome, closing the remaining
  visual-editor parity gaps in Dark+, Light+, and High Contrast themes.
- vscode: the create-project scaffold now writes a `src/config.st` CONFIGURATION
  that instantiates `Main` (RESOURCE + TASK + `PROGRAM Main WITH MainTask`), so a
  brand-new project opens clean instead of tripping the W009 "unused program"
  lint on first open (F-02).
- trust-debug: the debug/simulator control server now serves HMI schema and
  value requests from the launch project and live debug snapshot, so VS Code's
  HMI Preview can render against a running simulator instead of timing out on
  `hmi.schema.get`.
- trust-debug: simulator launches now load the project `io.toml` driver stack
  and direct debug-runtime builds size process images from I/O bindings, so
  Live Values reflects simulated/loopback I/O updates instead of showing forced
  outputs that never reach mirrored inputs.
- vscode: managed local runtimes now import their generated control token into
  SecretStorage and attach after a successful Start, so Live Values can read,
  write, force, and release values without manual token setup.
- trust-debug/vscode: Live Values Force and Release now use attach-safe
  debug-adapter I/O requests for managed and remote runtimes instead of
  `setExpression`, and managed-runtime token import now accepts both table and
  dotted `runtime.control.auth_token` TOML forms without rewriting the same
  SecretStorage entry on every refresh.
- trust-runtime: OPC UA server startup no longer aborts when the first runtime
  snapshot is not available yet; the server binds immediately and publishes
  exposed nodes after real snapshot evidence arrives. Runtime debug breakpoint
  control now accepts project-relative source paths such as `src/main.st`.
- trust-runtime: OPC UA client node browsing now returns each leaf's raw
  `node_id` plus an apply-ready scalar `data_type`, and OPC UA client
  browse/test failures now include structured reason codes for certificate,
  authentication, reachability, browse-denial, and security-profile errors.
  Explicit OPC UA client certificate trust now promotes a previously rejected
  server certificate into the trusted store before reconnecting, so the
  Devices & Connections `Trust certificate` action can recover from the first
  untrusted browse attempt.
- vscode: release packaging now bundles `trust-runtime` inside the VSIX next
  to `trust-lsp` and `trust-debug`, so fresh installs can use offline
  Devices & Connections configuration without requiring a separate runtime
  binary on `PATH`.
- trust-runtime: Network Canvas communication setup now labels the ADS client
  timing knob as an ADS link update interval instead of the internal "worker
  tick" term, and offline fleet topology now uses the OS hostname before
  falling back to `local-host`.
- trust-runtime: ADS live symbol browsing and exact-tag import now treat
  symbol-upload no-reply/timeouts after a stale route check as structured
  `route.status = "missing"` responses with an `ads.route_plan`, so the
  Network Canvas can show "Create route" instead of a raw ADS timeout when the
  TwinCAT return AMS route is missing.
- trust-runtime: `comm.schema` now advertises the implemented
  Discover/Browse actions for EtherCAT, GPIO, OPC UA expose, and ADS-server
  expose workflows, and `comm.apply` now bootstraps Network Canvas I/O edits
  from single-driver, empty, or comment-only `io.toml` files by writing the
  canonical multi-driver form instead of blocking on setup placeholder files.
- trust-runtime: HMI persistence reload now parses persisted JSONL trend/alarm
  records through a tolerant record reader, avoiding a deterministic reload
  failure observed in the full remote runtime gate while preserving the stored
  wire format.
- trust-debug: attach-mode I/O forcing now forwards VS Code Force/Release
  actions to remote runtimes through `io.force` and `io.unforce`, and runtime
  control `io.read` now marks forced rows so Live Values can show real remote
  force state instead of disabling the workflow.
- trust-debug: debug-launched local simulator control servers now receive the
  launch project root, letting `comm.apply` create or update the project's
  `io.toml` instead of failing Network Canvas easy-setup with "No runtime
  project root is available for io.toml setup."
- trust-runtime: ADS server UDP identify now listens on a wildcard UDP socket
  while advertising the configured concrete runtime identity, so TwinCAT
  Broadcast Search can reach truST instead of requiring directed identify or
  static route-file setup.
- trust-runtime: ADS server UDP discovery now acknowledges TwinCAT Broadcast
  Search Add Route requests while keeping actual ADS access gated by the
  source-pinned `[runtime.ads_server.clients]` allowlist.
- trust-runtime: ADS server value reads now zero-pad shorter scalar payloads to
  the requested read length and accept TwinCAT's zero-payload handle release
  form, matching TwinCAT symbolic FB expectations without changing symbol-upload
  or datatype-table response lengths.
- trust-runtime: ADS server subscriptions now accept Beckhoff ADS.NET 7 compact
  `AddDeviceNotification` registrations, normalize Beckhoff's 100 ns
  notification cycle/max-delay values to server milliseconds, and retain the
  registering ADS port per notification so TwinCAT/ADS.NET router-multiplexed
  clients receive changed-value notifications.
- trust-runtime: ADS server `SYM_UPLOADINFO2` responses now zero-pad to the
  requested read length, matching ADS clients such as `ads-rs` that read the
  24-byte upload-info structure through a larger fixed buffer.
- trust-runtime: ADS server `GET_SYMINFO_BYNAME` now returns the compact
  index-group/index-offset/byte-size tuple expected by ADS clients such as
  `ads-rs`, while `GET_SYMINFO_BYNAMEEX` continues to return the full symbol
  entry used by browsing clients.
- trust-runtime: ADS server now handles `SUMUP_READ_EX` batch reads with
  per-item result-length metadata, matching the batch-read form used by the
  `ads-rs` client backend.
- trust-runtime: ADS client `read_write` bindings now seed their output-change
  baseline from the first good remote read, preventing local cold-start default
  values from being written back before the PLC value has been applied.
- trust-runtime: ADS guarded write probes now wait for scan-boundary write
  application before read-back comparison, avoiding false mismatches against
  scan-applied ADS servers.
- trust-hir/trust-ide: IEC Table 39 validation functions `IS_VALID` and
  `IS_VALID_BCD` now type-check and surface in editor completion, hover, and
  signature help; `IS_VALID_BCD` accepts `BYTE`/`WORD`/`DWORD`/`LWORD` and
  rejects `BOOL`.
- trust-runtime: `IS_VALID_BCD` now rejects `BOOL`, and runtime bytecode now
  supports Table 17 partial read/write through properly mapped structured
  `VAR_IN_OUT` references.
- Runtime bytecode execution now supports IEC Table 17 partial
  bit/byte/word/dword access on `BYTE`, `WORD`, `DWORD`, and `LWORD`
  function-block instance fields, matching the existing program-variable
  behavior.
- `architecture-doctor --changed` now follows the current
  `crates/trust-runtime/src/host/...` runtime host layout for initializer and
  evaluation checks.
- Runtime I/O drivers now fail closed for the covered safety paths: MQTT
  disconnected/stale reads and publish/connect failures return structured
  runtime errors, EtherCAT discovery and image-size mismatches fault under every
  policy, Modbus flush/transport failures and Modbus exceptions remain
  distinguishable, and GPIO read/write failures are reflected in driver health.
- Retain persistence now writes through a temp file, flush/fsync, atomic rename,
  and parent directory sync, and retain snapshots now use a length-delimited
  CRC/trailer-protected codec with legacy v1 read support plus explicit
  retain migration/orphan events.
- Runtime initialization, evaluator assignments, and queued debug writes now
  fail closed for the covered safety paths: default materialization failures
  return `InitFailed` instead of becoming `NULL`, undefined evaluator/debug
  targets are rejected instead of creating globals, and queued debug write
  failures fault the cycle visibly.
- Interface-typed runtime variables now materialize as explicit `NULL`
  references during default initialization, and generic `ANY_INT` counter
  slots defer to the call argument type instead of failing runtime startup.
- Runtime bytecode lowering now materializes HIR-allowed contextual widening at
  assignment boundaries using the target runtime type, with proof coverage for
  function inputs/outputs, assignments, initializers, return values, InOut
  rejection, and narrowing rejection.
- Runtime audit/event, mesh, debug, and runtime-cloud safety paths now fail
  closed for the covered cases: closed audit sinks emit `AuditDropped`, closed
  debug event/log streams fall back to in-memory buffers, mesh snapshot timeouts
  are errors instead of empty maps, corrupt runtime-cloud config state becomes
  an explicit error state, corrupt link/rollout state prevents web startup, and
  disabled debug-control requests return structured `feature_disabled`.
- `FULLMAP-RUNTIMESAFE` now runs as a blocking architecture-doctor/CI gate with
  zero findings and zero allowlist entries.
- Harness boundary inputs now fail closed: typoed `set_input` names and
  `bind_direct` targets return structured boundary errors instead of creating
  hidden globals or undeclared I/O bindings.
- `CONFIGURATION` builds now reject declared top-level `PROGRAM`s that are not
  bound by the configuration, unless a test builder explicitly opts in through
  extra program instances.
- MQTT `keep_alive_s` is now applied to the `rumqttc` session options instead
  of being parsed only at validation time.
- MQTT TLS fixture paths are normalized before being embedded in TOML so the
  TLS config tests run on Windows paths as well as Unix paths.
- Dependency hygiene now has explicit `cargo deny`, `cargo audit`, and
  `cargo machete` policy evidence: unused direct workspace dependencies were
  removed, the workspace MSRV was raised from Rust `1.85` to Rust `1.95`,
  `time` was updated to the patched `0.3.47`, `ratatui` was updated to `0.30`,
  `rumqttc` no longer enables unused default TLS dependencies, `qrcode` no
  longer pulls its unused image backend, the optional `opcua-wire` advisory paths
  are documented with owner/rationale/removal metadata, `Cargo.lock` is tracked
  for reproducible CI dependency resolution, runtime modulo checks now use the
  Rust `1.95` `is_multiple_of` API so Clippy stays clean on the new MSRV,
  `third_party/tiverse-mmap` is an intentional workspace exclude, and the
  full-map doctor reports the dependency hygiene status with failing policy
  fixtures.
- External safety follow-up now prunes the Zenoh default-feature dependency
  surface to remove the transitive `rsa` advisory path, removes direct
  `rustls-pemfile` use from runtime TLS PEM parsing, refreshes owned
  `cargo deny` advisory metadata for the remaining OPC UA, tiny_http TLS, and
  Zenoh `paste` paths, and records the remaining external unsafe-scanner gap in
  the architecture follow-up checklist.
- Parser recovery for malformed aggregate/positional initializers is now
  bounded by shared top-level scan helpers, preserves following declarations at
  declaration boundaries, and is guarded by full-map doctor and focused
  mutation evidence.
- HIR validation now rejects non-repeat call expressions used as array defaults
  and validates direct array repetition defaults against the repeated element
  type, closing focused mutation-testing gaps in default initializer analysis.
- Runtime and constant-expression array repetition initializers now shape arrays
  from the expanded value count, so declarations such as `[3(1, 2)]` materialize
  as six elements instead of failing type validation as a one-element array.
- Bytecode validation now rejects duplicate `POU_INDEX` ids and unsupported
  runtime-only opcodes before VM module construction/dispatch instead of
  silently allowing map overwrites or late runtime traps.
- VM instance-owner inference now scans partial-access opcodes with their
  correct operand width, avoiding a silent owner-context drop in bytecode that
  mixes instance references with bit/byte/word/dword access.
- Bytecode validation now rejects POU bytecode that uses a local reference
  outside that POU's declared local-ref range, so malformed local slots fail
  before VM frame execution, while still accepting owned derived local path
  references such as local array elements and struct fields.
- HIR validation now reports `UndefinedVariable` when a `VAR_ACCESS` access
  path points at a missing target instead of silently accepting the declaration.
- HIR constant evaluation now reports `CannotResolve` for ambiguous unqualified
  enum value names instead of picking the first matching enum member from the
  symbol table.
- HIR alias resolution no longer silently stops after sixteen alias hops, so
  deep but valid alias chains resolve to their base type.
- HIR call checking now reports `UndefinedFunction` when a resolved field or
  non-callable expression is used as a callee instead of silently returning an
  unknown type.
- HIR global symbol lookup now uses the same first-writer collision policy for
  normal collector inserts and raw/import inserts, avoiding order-dependent
  duplicate-name lookup drift.
- Project source registration now rejects explicit `FileId` collisions instead
  of silently reallocating a different file identity.
- HIR type-check context construction now classifies missing POU owners/scopes
  and reports `CannotResolve` instead of silently falling back to global scope.
- HIR OOP diagnostics now use a shared extends-chain walker so mixed
  function-block/class inheritance cycles cannot bypass cycle detection.
- HIR diagnostics now distinguish existing symbols used in the wrong semantic
  role: values used as types, types used as values, and callables used as
  variables report primary wrong-kind diagnostics instead of silent unknowns or
  wrong-reason undefined diagnostics.
- HIR expression checking now applies the same wrong-kind diagnostics to
  namespace-qualified project imports, so imported types used as values no
  longer type-check silently through field-expression resolution.
- HIR/parser name handling now treats `GET` and `SET` as contextual names in
  method/member expression positions, so methods such as `Get()` can be
  collected, type checked, and called without fallback to global scope.
- HIR expression inference now suppresses numeric/bit/unary/index/dereference
  and `__DELETE` follow-on type errors when an operand or indexed base already
  has a primary unresolved-name diagnostic.
- HIR validation now reports unresolved or malformed `VAR_CONFIG` targets
  explicitly instead of letting invalid configuration entries disappear during
  collection/validation.
- Cross-project HIR symbol imports now report duplicate imported global names
  instead of silently keeping the first imported binding.
- Cross-project HIR type imports now report cyclic aliases with
  `CyclicDependency` and avoid degrading the cycle into silent unknown-type
  suppression.
- HIR type checking now reports constant-evaluation failures at array bounds,
  subrange assignments, CASE label duplicate tracking, and `SIZEOF` type
  operands instead of collapsing `ConstEvalError` into `None`.
- HIR assignment compatibility no longer substitutes missing array element or
  pointer target `TypeId`s with `Type::Unknown`, preventing malformed type
  identities from being accepted as compatible.
- HIR typed literals now report `UndefinedType` when their type prefix cannot
  be resolved instead of silently inferring an unknown expression type.
- Project source registration now preserves noncanonical fallback paths when
  OS canonicalization fails, preventing lexical paths from silently colliding
  with existing canonical source keys.
- HIR OOP `EXTENDS`/`IMPLEMENTS` resolution no longer treats a missing owner
  scope as global scope when resolving inherited or implemented types.
- HIR interface conformance now resolves inherited interface members through
  the owner's namespace scope instead of falling back to a global bare name.
- HIR call inference now preserves ambiguous `USING` resolution as the primary
  `CannotResolve` diagnostic instead of adding a wrong `UndefinedFunction`.
- HIR value/type expression inference now preserves ambiguous `USING`
  resolution as the primary `CannotResolve` diagnostic for value reads,
  assignment targets, `SIZEOF`, and `NEW` type operands instead of degrading
  into wrong-reason undefined diagnostics.
- HIR standard function type checking now suppresses wrong-reason argument
  diagnostics when an argument already has a primary unresolved-name diagnostic.
- HIR diagnostics now deduplicate exact duplicate diagnostics by full diagnostic
  identity, preventing repeated semantic probes from reporting the same primary
  error multiple times while preserving distinct diagnostics.
- HIR `SymbolTable` broad lookup helpers are no longer public API; external
  callers now use explicit global, qualified-name, or registered-type lookup
  contracts.
- HIR constant precollection now keys constants by full namespace/POU scope
  identity, so same-named POUs in different namespaces cannot share local
  constant values for array, subrange, or string bounds.
- HIR `VAR_CONFIG` validation now treats duplicate bare program instance names
  across resources as ambiguous and reports `CannotResolve` instead of choosing
  one instance and emitting follow-on type/address diagnostics.
- HIR member and compatibility resolution for raw `EXTENDS`/`IMPLEMENTS`
  references now resolves bare base/interface names from the owning POU scope
  instead of falling back to a global bare-name lookup.
- HIR analysis now exposes a declaration catalog with qualified names, source
  identity, semantic roles, project-import origin, translated type identities,
  and classified OOP reference outcomes; runtime POU registration is driven by
  that catalog and reports catalog/lowering mismatches instead of silently
  skipping accepted declarations.
- VS Code activation no longer writes the default runtime control endpoint into
  workspace folder settings; runtime panels still use the built-in endpoint
  fallback when the setting is empty.
- `Pt1Filter.Reset()` now reinitializes the underlying OSCAT `FT_PT1` state on
  the next update, and `HysteresisSwitch.Reset()` now resets the actual
  hysteresis state instead of only clearing cached wrapper fields.
- `UnitConverter` now delegates Celsius/Kelvin and energy/calorie conversions
  to classic OSCAT functions/function blocks so the component facade cannot
  silently diverge from the parity oracle.
- OSCAT OOP tests now cover reset behavior, multi-scan PT1 parity,
  invalid limit rejection, FIFO ordering, and the OSCAT version through
  `OSCAT_VERSION(IN := FALSE)` instead of a hardcoded value.
- Public docs links for PLCopen/Oscat example READMEs now resolve under MkDocs
  strict mode.
- Runtime compilation now registers namespaced `PROGRAM` declarations and
  resolves sibling namespace types for local OOP `IMPLEMENTS` checks, so
  namespaced runtime programs and same-namespace interfaces are handled
  consistently with functions, function blocks, classes, and type declarations.
- Runtime bytecode generation now preserves POU namespace context for sibling
  function calls, keeps method names owner-local inside namespaced class-like
  POUs, and exposes bare return-variable aliases for namespaced functions.

### Added

- Structured Text initializers now support named aggregate defaults for
  `STRUCT`/`UNION` values at VAR, TYPE, and member-default sites, route those
  defaults through a runtime initializer service, validate unknown/duplicate
  fields with stable diagnostics, allow legal function-block member overrides,
  and expose `trust-runtime bench init` for startup/first-cycle initialization
  benchmark evidence.
- PLCopen Motion now ships a second, object-oriented package at
  `libraries/plcopen_motion/oop`, with ST interfaces and concrete objects for
  `itfAxis`, command objects, and `MC_OopAxis`. The package delegates axis
  behavior to the classic single-axis PLCopen package, includes deterministic
  unsupported command-object returns for unimplemented OOP methods, and is
  covered by Structured Text unit tests plus runtime integration drivers.
- The public docs now include a PLCopen Motion OOP library guide and five
  runnable real-world examples: warehouse shuttle, labeling conveyor,
  pick-and-place lift, indexing table, and feeder axis.
- OSCAT now ships an object-oriented Components companion package at
  `libraries/oscat/oop`, with narrow interfaces for automation context,
  unit conversion, filtering, PI/PID/PWM control, hysteresis switching, signal
  generation, FIFO/stack memory objects, latches/toggles/counters, measuring
  components, calendar/RTC helpers, selected building-control objects, and a
  single-output device driver. The package is covered by Structured Text
  parity tests against classic OSCAT.
- The OSCAT OOP example suite now includes 49 classic/OOP comparison
  pairs under `examples/OSCAT/<example>/{non-oop,oop}`: 27 hand-written
  process-first industrial pattern scenarios, 20 compact component-composition
  showcases, and 2 compact pattern showcases. Each example folder has one
  teaching README explaining the process, the OOP pattern, why it helps, how to
  reuse it, and when classic ST is better. Each project has Structured Text
  application code and Structured Text unit tests; OOP projects with
  communication claims include runtime/IO
  configuration and README integration maps naming `%I/%Q` bindings and exposed
  runtime records. A Rust catalog gate now runs every project and checks that
  the claimed OOP pattern is present in `src/Main.st`, covering Factory,
  Template Method, Strategy, Mediator, Observer, Composite, Iterator-style
  traversal, Decorator, Facade, Chain of Responsibility, State,
  Command/Memento, Adapter, Proxy, polymorphism, and composition. Public docs
  include a library guide plus the new truST Structured Text naming standard.

- The public docs now include a `One Project, Every Surface` concept page that
  positions VS Code, Editor AI tools, Browser IDE, Browser HMI, CLI/CI, Agent
  API, LSP editors, and truST Mesh as live surfaces over the same project, with
  an honest surface capability matrix and AI tooling evidence links. The docs
  homepage, README, workflow chooser, visual-editor docs, and migration path
  now point into the same narrative, and the README/homepage use a new
  one-project surface-tour GIF built from real product captures. Migration
  ecosystem pages now have canonical `migrate/*` URLs, visual-editor pages avoid
  misleading empty captures by using accurate diagrams, and project/support
  pages stay outside the lookup-only Reference route. A full public-doc text
  pass also removed thin wrapper prose, separated HMI authoring from HMI
  operation, routed Learning Paths into Program, clarified Troubleshooting vs
  FAQ, expanded First Project into a real walkthrough, added jargon notes to
  concept pages, tightened the homepage around one route table plus real
  product proof, added richer example/category routing, kept AI Assistance
  grounded in diagram/source evidence, added first-figure proof PNGs for the
  operator HMI overview, daily-check, alarm, and handover pages, and tightened
  README platform/status wording including Linux PREEMPT_RT support. The public docs nav has since
  been collapsed to six user-facing doors (`What Is truST?`, `Install`,
  `Program`, `Run`, `Hardware`, `Reference`), with tutorials/examples,
  communication, I/O, HMI authoring, migration, automation, and AI work routed
  through `Program`; OpenPLC is no longer presented as a standalone truST
  workflow; dense terms blocks and stock "use this guide when" boilerplate were
  removed; and the one-project GIF was regenerated at higher resolution without
  the dark banner overlay.

- `trust-runtime plcopen` now imports official TC6 XML multi-worksheet ST
  bodies in execution order, reconstructs all standard POU interface var
  sections (`VAR`, `VAR_TEMP`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`,
  `VAR_EXTERNAL`, `VAR_GLOBAL`, `VAR_ACCESS`), and round-trips CODESYS
  `addData/method` metadata for `FUNCTION_BLOCK` methods with deterministic
  method object IDs/project-structure placement.

- `trust-runtime` now supports a Linux `runtime.realtime` startup profile for
  `PREEMPT_RT` posture, including config-schema support, bundle-template
  defaults, launcher-side kernel/scheduler/affinity/memory-lock verification,
  richer runtime status/config surfaces, application-level cycle `p50` / `p95`
  / `p99` metrics, a dedicated operator deployment guide, a reference
  `systemd` unit template, and a shipped validation script for collecting
  host/kernel/service evidence alongside release-mode benchmark artifacts.
- The public docs now have a workflow-first entry model instead of a mixed
  "one start flow for everyone" structure. Home and Start now route five real
  user journeys directly: Program in VS Code, Program in Browser IDE, Operate
  in Browser HMI, Automate with CLI / CI / agents, and Maintain an Existing
  Project. The site now also includes public `About`, `FAQ`, `Contribute`,
  `Version History`, `Hardware Compatibility`, `API Lifecycle And Deprecation`,
  runbook/operator pages, target-host lifecycle pages, and enterprise-integration
  routing pages so technicians, maintainers, and GitHub visitors can find the
  right docs path without repo spelunking.
- The docs homepage and repo README now surface the published docs site as the
  primary entry point, use the truST logo directly, and show real browser
  captures for Browser IDE and Browser HMI instead of forcing readers through
  repo file paths or VS Code-only framing.
- `trust-runtime agent serve` now provides an initial external agent surface
  over stdio JSON-RPC with `agent.describe`, `workspace.read`,
  `workspace.write`, `workspace.project_info`, `lsp.diagnostics`,
  `lsp.format`, `runtime.build`,
  `runtime.compile_reload`, `runtime.validate`, `runtime.test`,
  `runtime.reload`, `harness.load`, `harness.reload`, `harness.cycle`,
  `harness.set_input`, `harness.get_output`, `harness.advance_time`, and
  `harness.run_until`, plus stable error codes for unknown methods,
  workspace-path escapes, harness state, and bounded `run_until` timeouts.
  The runtime-facing methods now reuse the existing `build --ci`,
  `validate --ci`, JSON test-output path, in-process Web IDE
  diagnostics/formatting services, and control-server `bytecode.reload`
  contract instead of inventing a second reporting surface.
- `trust-runtime agent serve runtime.compile_reload` now returns one
  machine-readable payload for `diagnose -> build -> reload`, including
  diagnostics counts/issues plus `runtimeStatus` / `runtimeMessage` and the
  optional nested build/reload results needed for iterative repair loops.
- `trust-runtime agent serve` now exposes `lsp.ast_canonicalize` and
  `lsp.ast_similarity` for the canonical-AST 5-gram normalization path used by
  benchmark contamination checks, dedup, and self-improvement diversity gates.
  The existing `lsp.diagnostics` payload now also includes end positions and
  zero-based UTF-8 byte spans, so benchmark/datagen tooling can consume stable
  machine-readable diagnostics without brittle text scraping.
- `trust-runtime agent serve` now also exposes a stateless `harness.execute`
  fixture wrapper that loads project or inline sources, runs deterministic
  harness steps, evaluates output/access/direct-I/O assertions, and returns a
  compact pass/fail report with reduced failure context for benchmark/datagen
  callers that need one-shot POU or system-fixture execution.
- `trust-runtime agent serve workspace.project_info` now returns a stable
  orientation payload for agents, including resolved source roots, source
  files, local dependency roots, runtime/io config presence, parsed runtime
  control/web/mesh/discovery summary fields, and `trust-lsp.toml`
  `vendor_profile` when present.
- `trust-runtime` now lowers ST `PROPERTY` implementations into runtime getter
  and setter calls, so OOP interface references can dispatch property reads and
  assignments across source files and package dependencies.
- `trust-harness` is now a real programmable deterministic executor instead of
  a minimal `load` / `cycle` helper. It now supports `reload`, `set_input`,
  `get_output`, `set_access`, `get_access`, `bind_direct`,
  `set_direct_input`, `get_direct_output`, `advance_time`, `run_until`,
  `restart`, and `snapshot`, all over structured JSON-lines with shared typed
  IEC value encoding and protocol docs in
  `docs/guides/TRUST_HARNESS_PROTOCOL.md`.
- A canonical public docs site now lives under `docs/public/` with question-
  driven navigation (`start`, `develop`, `connect`, `operate`, `reference`,
  `concepts`, `examples`), a MkDocs + Material site config at repo root, a
  Pages workflow that builds and deploys the docs site, and a public-doc link
  checker in `scripts/check_public_docs_links.py`.
- The runnable examples catalog is now aligned to the docs IA instead of
  acting like one flat dump: `examples/README.md` now routes by docs category,
  `docs/internal/testing/checklists/example-catalog-audit.md` records keep /
  tweak / merge / archive decisions, `examples/sfc/README.md` and
  `examples/web_ui_complete_project/README.md` were added, and
  `examples/simulate_process/README.md` now marks that family as archive-
  candidate rather than a first-stop public example. Kept public examples now
  also link back to their owning docs category, and
  `scripts/check_example_catalog_links.py` enforces that the curated docs
  catalog points to real runnable example paths.
- The public docs are now materially self-contained instead of mostly wrapper
  pages: editor selection, protocol matrices, runtime-to-runtime transports,
  visual-editor guidance, runtime control/debug/operator docs, benchmark
  reference, vendor-profile example routing, and key protocol pages now render
  substantive in-site content instead of punting readers to raw GitHub pages.
- Public-doc media is now generated through an explicit pipeline:
  `scripts/capture-public-docs-visual-editors.sh` captures the visual-editor
  screenshots automatically, `scripts/generate_public_docs_media.py` syncs
  them into `docs/public/assets/images/`, and the Pages workflow now rebuilds
  when those source screenshots or media-generation scripts change.
- The docs Pages workflow now has stricter quality gates: local search
  regression checks, post-deploy verification of the published site, and
  automated media synchronization all run as part of the docs build path.
- The public docs now cover more real operator/agent search paths directly in
  CI, including `scan cycle`, `project layout`, `visual editor`, `vendor
  profile`, `watchdog`, `fault policy`, `agent serve`, `hot reload`, and test
  output format queries, so discoverability regressions are caught earlier.
- Public-doc screenshots now have a first automated browser-capture lane:
  Playwright regenerates checked-in `/ide` and `/hmi` assets against live
  truST surfaces, code-server provides a truthful VS Code-compatible proof
  capture for the public docs, `docs/public/assets/capture-inventory.json`
  records the checked-in completeness contract, and a dedicated docs-captures
  workflow can refresh those assets on `main` and nightly without relying on
  `ydotoold`.
- Public-doc terminal captures now have a first automated lane as well:
  VHS tapes regenerate checked-in CLI verification GIFs for installation and
  build/validate/test docs, and the same docs-captures workflow refreshes
  those assets alongside the browser/code-server captures.
- The public docs now include a dedicated diagnostics reference page covering
  current `E...`, `W...`, and `I...` codes, their default severities, meanings,
  and first-fix guidance, and docs-search regression coverage now includes code
  queries such as `E001`, `W003`, `implicit conversion`, and `missing else`.

### Fixed

- Enum values loaded from `VAR` initializers now compare equal to the same enum
  variant literal in VM execution, covering unqualified, qualified, and
  literal-on-left equality checks for init-only state-machine selectors. Runtime
  enum construction now canonicalizes alias-backed enum values and retained enum
  state before comparison, preventing stale or case-variant type names from
  silently turning valid enum equality checks into `FALSE`. Runtime compound
  values now also validate struct field identity and array shape at construction
  and retain-apply boundaries, so corrupted retained structs or arrays fail with
  diagnostics instead of being silently coerced or defaulted.
- Runtime ST test compilation now reuses expression type information from the
  HIR analysis pass, avoids repeated full-project symbol-table clones during
  lowering, and skips shared-global task-hazard scans when a project has no
  configured tasks. This keeps the full OSCAT timeout regression below the
  test harness 60-second warning while preserving the runtime timeout budget.
- Public docs IA now removes the duplicate Install/Verify Install route,
  keeps Reference focused on lookup material by moving specifications higher
  and hiding contributor-only pages, deletes dead interoperability stubs and
  orphan/broken image assets, embeds the real one-project surface tour on the
  docs homepage, and expands the IA checker to block generic filler phrases
  such as "use this page", "after reading", and "you should be able".
- Public docs visual chrome now uses a dark/light palette toggle, top-level
  navigation tabs for the six main doors, a shorter GitHub repo label, a
  distinct accent color, Inter/JetBrains Mono typography, and image overflow
  safeguards so screenshots stay inside the content column. The docs homepage
  also switches to a dark-mode wordmark variant so the brand stays legible on
  the slate palette.
- Public docs navigation now exposes the PREEMPT_RT runbook from the Operate
  sidebar, reuses the interoperability pages under Migrate without moving their
  canonical URLs, groups Operate by engineering, target-administration,
  operator/technician, and fleet-delivery personas, surfaces truST Mesh from
  Connect, links README start paths directly, and avoids confirmed
  snippet/list-rendering issues in included public documentation. Concept,
  Reference, Specifications, and Examples indexes now use clearer task and
  mental-model groupings, the docs maintenance guide codifies nav, snippet,
  visual, verification, and proof rules, the SFC profile reference now uses a
  non-numbered slug to avoid a spec-14 collision, visual-editor and operator
  pages now include public visuals, the Migrate page includes a compatibility
  matrix, and the example catalog audit understands PyMdown line-range snippet
  targets used to skip included page titles. A new docs IA lint now enforces
  section-index/nav coverage, snippet H1 safety, and CommonMark list spacing,
  and the internal docs checklist records analytics, versioning, success-metric,
  accessibility, and mobile-review decisions.
- The shipped `examples/oscat_smoke` consumer now matches the current OSCAT
  named-parameter surface (`DEG`, `DAYS`, and `DATA`), so the example builds
  cleanly again instead of failing on stale `D := ...` calls.
- `trust-lsp` push-diagnostics mode now refreshes dependent open documents
  after `didOpen`, `didChange`, and `didSave` events, so cross-file call
  diagnostics clear and update immediately instead of staying stale until a
  reopen/save cycle.
- Method-call completion and signature help now resolve `FUNCTION_BLOCK`
  methods with `VAR_INPUT` parameters correctly, so formal parameter names are
  offered for `fb.Method(...)` calls instead of falling back to the owning
  instance call shape.

- Linux `PREEMPT_RT` support no longer breaks non-Linux cargo builds and
  warning gates: Linux-only helpers/imports in `trust-runtime` are now
  target-scoped so macOS and Windows builds keep compiling cleanly while the
  RT posture feature remains Linux-only.
- The Linux cyclic runtime no longer rebuilds ready-task and background-program
  scratch vectors every scan cycle; those scratch buffers are now runtime-owned
  and reused so the `PREEMPT_RT` path reduces avoidable hot-path allocation
  churn while preserving existing Linux behavior.
- Linux `PREEMPT_RT` posture verification now reads the scheduler thread's
  user-space realtime priority from `/proc/thread-self/stat` instead of the
  kernel-internal `/proc/.../sched` priority field, so correctly-configured
  `SCHED_FIFO` / `SCHED_RR` deployments no longer report false priority
  mismatches. The shipped validation script now also exposes whether a target
  threshold was declared, records the approximate soak window, warns when a
  `PREEMPT_RT` run falls back to the cycle budget, and fails gated RT evidence
  runs when `cyclictest` or an explicit `TRUST_RT_CYCLE_P95_MAX_US` threshold
  is missing.
- The public docs and repo landing surfaces now lead with the strongest truST
  story instead of burying it behind browser-first proof shots: the homepage,
  `Program In VS Code`, `Debugging And Runtime Panel`, and `README.md` now show
  real desktop VS Code screenshots for the runtime panel, IEC-aware diagnostics,
  paused debugging, and cross-file rename, while browser IDE/HMI surfaces are
  clearly presented as supporting views of the same project.
- The docs site now ships the truST wordmark, a dedicated favicon, and a teal
  Material theme instead of the default MkDocs branding, and the public pages
  were rewritten to remove boilerplate "use this page when..." framing and
  similar filler so the rendered docs read as product documentation instead of
  internal notes.
- The public docs landing/start/reference/operator pages now describe truST in
  direct product language, ship real social metadata for sharing, and replace
  thin operator checklists with concrete tables, screenshots, and escalation
  placeholders instead of stub prose.
- The checked-in Browser IDE and code-server public-doc captures now prove a
  successful truST state more clearly: `/ide` waits for a real build to finish
  before capture, and the VS Code-compatible proof opens the Structured Text
  Runtime panel instead of showing only a generic workspace shell.
- The docs search regression gate now routes `install mac`, `first project`,
  `ladder`, and `retain` queries back to the intended public docs pages after
  the docs IA/spec split, including a restored `Start -> First Project` page
  and stronger canonical ranking for `runtime.toml` retain/watchdog/fault
  queries.
- The docs-capture code-server launcher now keeps mutable user-data and
  extension state inside container-local writable directories, avoiding the
  GitHub Actions permission failure that previously blocked the code-server
  capture lane on repo-mounted `.cache` paths.
- The public docs Pages pipeline no longer hard-fails on runners without
  `ffmpeg` when all committed screenshot assets are already present, and the
  cross-platform `trust-runtime agent serve` contract tests now normalize
  OS-specific path spellings (`/private` aliases, `\\?\\` prefixes, native
  separators) instead of failing on macOS/Windows-only path formatting
  differences.
- Warm restart and live `bytecode.reload` now preserve compiled instance-backed
  runtime references across the restart boundary, so instance-backed I/O
  bindings and the new `trust-runtime agent serve` `runtime.reload` flow no
  longer trip `null reference dereference` faults immediately after a reload.

- VS Code language-model tool discovery is now consistent again: the extension
  registers the same linked-editing, on-type-formatting, call-hierarchy, and
  type-hierarchy tool names that `editors/vscode/package.json` declares and
  activates, and the extension test suite now fails if manifest declarations,
  activation events, and `lm.registerTool(...)` registrations drift apart.
- The public docs workflow now rebuilds on guide/spec/conformance/media-source
  edits instead of only direct `docs/public/**` changes, closing the stale-site
  gap for `--8<--`-included content and generated public assets.
- The code-server screenshot lane now waits for real editor content instead of
  only a tab label, dismisses first-run/sign-in chrome, closes the unrelated
  chat side panel before capture, and renames the Browser IDE screenshot to the
  truthful "tutorial loaded" state instead of implying a welcome screen that
  was not what the image actually showed. The installation docs also no longer
  tell users to run the nonexistent `cargo build -p trust-harness` package
  command; they now document the real package/build surface.
- The automated public-doc capture lanes now produce stable dark-mode browser
  and code-server screenshots from real app/theme controls, wait for the HMI
  schema/export surface before capturing `/hmi`, and install/load the truST
  VS Code extension through one consistent code-server profile so the
  command-palette capture stops regressing on fresh CI boots.
- Unqualified enum variant names now also resolve in three contexts outside
  `CASE` labels: `VAR` initializers (`state : Phase := IDLE`), the right-hand
  side of assignments (`state := RUNNING`), and operands of binary
  comparisons (`IF state = RUNNING THEN ...`). Before this change the first
  form failed PROGRAM init with `undefined variable 'IDLE'`, and the other
  two compiled but silently lowered the bare variant name as an unresolved
  `Expr::Name`, so the target either kept its previous value or the
  comparison never matched at runtime. The fix introduces a small
  harness-lowering helper next to the existing `enum_literal_value` used by
  `CASE` labels and applies it at the VAR-declaration, assignment, and
  binary-expression call sites, mirroring the pattern introduced by the
  unqualified-CASE-label fix. HIR type-check was already accepting all
  three forms, so this is a lowering-layer alignment only; bare names still
  follow normal symbol resolution, which means local variables/constants
  continue to shadow same-named enum members, and qualified `Phase#X` forms
  plus `CASE` labels behave as before.
- The ST compiler/runtime build path now closes the remaining open regression
  cases around control-flow and declaration lowering: unqualified enum members
  now work as `CASE` labels in end-to-end runtime/bytecode builds, aggregate
  array declaration initializers now parse and execute for explicit, partial,
  and repetition-count forms, struct-field access through indexed arrays stays
  buildable on the CLI path, and bare `RETURN;` now works after assigning the
  implicit function/method result on that path because the HIR checker tracks
  definite assignment and the bytecode VM lowers `RETURN` instead of rejecting
  it as a legacy C5 edge case.
- The OSCAT runtime/compiler path now handles the full shipped OSCAT library
  more reliably: bytecode lowering resolves local instance names before global
  callable symbols, normalizes OSCAT-style identifier lookup consistently
  across locals/reference lowering, and fixes remaining Chapter 19/20/23/24
  regressions such as `AOUT`/`AOUT1` scaling, `MULTI_IN` mode `0`,
  `ACTUATOR_COIL`, `CTRL_IN`, `HYST*`, `PARSET`/`PARSET2`, and first-scan
  `TEMP_EXT` sampling. The full OSCAT core fixture now runs green again.
- Runtime VM string access, native-call dispatch, and hot-path execution are
  now more consistent and predictable: signed unary `NEG` on minimum signed
  integers now reports overflow instead of panicking/wrapping, pure-`DINT`
  `MOD 0` now matches the generic `ModuloByZero` contract, array offset math
  and runtime date construction reject invalid edge cases deterministically,
  runtime `STRING` / `WSTRING` indexing and string stdlib helpers now follow a
  single documented element contract for non-ASCII text, `CALL_NATIVE` now
  caches statically resolvable function targets instead of uppercasing and
  hashing every call, and the VM hot path now reuses register frame buffers,
  drops redundant register-file zeroing, batches stack deadline checks, and
  avoids cloning the debug hook on each statement. The release benchmark path
  also restores the earlier VM latency envelope by caching per-POU local-init
  plans and trimming dynamic-ref hot-path overhead instead of rebuilding that
  work every scan.
- The OSCAT BASIC `CALENDAR` port keeps truST strict about IEC reserved
  keywords instead of relaxing the parser, so the shipped Chapter 3 data type
  exposes compliant local-time field names `LOCAL_DT`, `LOCAL_DATE`, and
  `LOCAL_TOD` in place of upstream `LDT`, `LDATE`, and `LTOD`.
- `trust-hir` now resolves cross-file user-defined types for root `VAR_GLOBAL`
  declarations during project-aware symbol collection instead of depending on a
  late repair pass, so split projects like OSCAT BASIC can keep shared carrier
  types in one file/chapter and their globals in another while diagnostics,
  analysis, runtime builds, and VM execution still resolve `MATH` / `PHYS` /
  `LANGUAGE` correctly. The same project-type catalog path now also preserves
  imported function block/class/interface identity, honors `USING` + namespaced
  simple type references, and attaches every declaration inside multi-entry
  `TYPE ... END_TYPE` blocks so cross-file POU member access surfaces like
  `FB_Accumulator`, `ST_PumpCommand`, and `ST_PumpStatus` analyze correctly.
- Typed conversion calls no longer misclassify an outer positional argument as a named/formal argument when the nested inner call uses `IN := ...`, so expressions like `UDINT_TO_REAL(DWORD_TO_UDINT(IN := x))` type-check correctly again.
- `SIZEOF(...)` now resolves the static type of an explicit type name or
  storage operand (`var`, field/index access, dereference, `THIS.field`) during
  analysis/lowering instead of evaluating a runtime value, so `SIZEOF(var)` now
  works in normal expressions and constant contexts like
  `ARRAY[0..SIZEOF(packet)-1]`, bare-name lookup follows CODESYS-style
  variable-before-type shadowing even when a top-level type shares the same
  bare name, pointer/reference operands now report and const-fold to platform
  pointer word size instead of truST's internal handle layout, and invalid
  forms such as `SIZEOF(call)`, arithmetic expressions, open arrays, unsized strings, and whole
  FB/class/interface instances fail deterministically during build instead of
  lowering to the legacy `SIZEOF_VALUE` path.

### Added

- `libraries/oscat` now ships the full manual-aligned OSCAT chapter set
  (`03_data_types` through `26_list_processing`) as the primary OSCAT package,
  including the OSCAT_BUILDING Chapter 23 control/environmental surface, a
  full ST conformance project under `fixtures/oscat/core` that now passes
  `126` tests with `0` failures and `0` errors, plus a narrowed
  `negative_public_surface` sentinel that exists only to pin the deliberate
  upstream `OVERRIDE` -> truST `OVERRIDE_3` name deviation.
- The OSCAT user documentation now matches the library’s current shipped
  surface: `docs/guides/OSCAT_LIBRARY_GUIDE.md`, `libraries/oscat/README.md`,
  `docs/README.md`, and `examples/oscat_smoke/README.md` now describe the full
  package layout, validation evidence, public carrier/data-type surface, and
  the major function-block families instead of the earlier smaller OSCAT BASIC
  slice.
- `libraries/oscat` now also ships the full Chapter 6 `Arrays` surface:
  `_ARRAY_ABS`, `_ARRAY_ADD`, `_ARRAY_INIT`, `_ARRAY_MEDIAN`, `_ARRAY_MUL`,
  `_ARRAY_SHUFFLE`, `_ARRAY_SORT`, `ARRAY_AVG`, `ARRAY_GAV`, `ARRAY_HAV`,
  `ARRAY_MAX`, `ARRAY_MIN`, `ARRAY_SDV`, `ARRAY_SPR`, `ARRAY_SUM`,
  `ARRAY_TREND`, `ARRAY_VAR`, and `IS_SORTED`.
- `libraries/oscat` now also ships the full Chapter 4 `Other Functions`
  slice: `STATUS_TO_ESR`, `OSCAT_VERSION`, `ESR_COLLECT`, `ESR_MON_B8`,
  `ESR_MON_R4`, and `ESR_MON_X8`.
- `libraries/oscat` now also ships the full Chapter 5 `Mathematics`
  surface, including the remaining error/distribution helpers (`ERF`, `ERFC`,
  `GAUSS`, `GAUSSCD`), integer/sequence/fraction helpers (`EXPN`, `FACT`,
  `FIB`, `GCD`, `REAL_TO_FRAC`), transcendental helpers (`GDF`, `GOLD`,
  `LAMBERT_W`, `LANGEVIN`, `SIGMOID`, `SINC`, `SQRTN`, `TANC`), rounding and
  window helpers (`RND`, `ROUND`, `WINDOW`, `WINDOW2`), and the OSCAT random
  helpers (`RDM`, `RDM2`, `RDMDW`).
- `libraries/oscat` now also ships the first Chapter 3 data-type slice:
  `CALENDAR`, `COMPLEX`, `CONSTANTS_LOCATION`, `CONSTANTS_SETUP`, `ESR_DATA`,
  `FRACTION`, `HOLIDAY_DATA`, `REAL2`, `SDT`, `TIMER_EVENT`, and `VECTOR_3`,
  plus the preloaded `MATH.FACTS[...]` factorial table used by later OSCAT
  helpers.
- `libraries/oscat` now also ships the stateful helper function blocks `DELAY` and `FT_AVG`.
- `libraries/oscat` now also ships the first linear/polynomial/ramp math helpers: `F_LIN`, `F_LIN2`, `F_POLY`, `F_POWER`, `F_QUAD`, and `FRMP_B`.
- Added the first shipped `libraries/oscat` package with the initial OSCAT compatibility slice (shared math/physics constants plus core conversion helpers), alongside `examples/oscat_smoke` as the first consumer project wired through `[dependencies]`.
- Added OSCAT compatibility extensions `TIME_TO_DWORD`, `DWORD_TO_TIME`, `T_PLC_MS`, and `T_PLC_US`, plus restored `DIR_TO_DEG`, `F_TO_PT`, `PT_TO_F`, and the live `LANGUAGE.DIRS[...]` surface in `libraries/oscat`.
- `libraries/oscat` now also ships a larger date/time slice including `DAY_TO_TIME`, `HOUR_TO_TIME`, `MINUTE_TO_TIME`, `SECOND_TO_TIME`, `DAY_OF_DATE`, `DAYS_DELTA`, `DAYS_IN_MONTH`, `DAYS_IN_YEAR`, `DATE_ADD`, `EASTER`, `WORK_WEEK`, `HOUR_OF_DT`, `MINUTE_OF_DT`, `SECOND_OF_DT`, `MONTH_BEGIN`, `MONTH_END`, `YEAR_BEGIN`, `YEAR_END`, and related leap/year helpers.
- `libraries/oscat` now also ships the first larger string and logic slices, including `MONTH_TO_STRING`, `WEEKDAY_TO_STRING`, `DT_TO_STRF`, `TO_LOWER`, `TO_UPPER`, `LOWERCASE`, `UPPERCASE`, the `ISC_*` / `IS_*` predicate helpers, the `FIND_CHAR` / `FIND_CTRL` / `FIND_NONUM` / `FIND_NUM` / `FINDB*` / `FINDP` search helpers, plus `CAPITALIZE`, `CLEAN`, `COUNT_CHAR`, `COUNT_SUBSTRING`, `CODE`, `DEL_CHARS`, `TO_UML`, `DEC_TO_BYTE`, `DEC_TO_DWORD`, `DEC_TO_INT`, `BYTE_TO_STRB`, `BYTE_TO_STRH`, `DWORD_TO_STRB`, `DWORD_TO_STRH`, `BIN_TO_BYTE`, `BIN_TO_DWORD`, `HEX_TO_BYTE`, `HEX_TO_DWORD`, `OCT_TO_BYTE`, `OCT_TO_DWORD`, `MIRROR`, `REPLACE_ALL`, `REPLACE_CHARS`, `REPLACE_UML`, `CHARCODE`, `CHARNAME`, `TICKER`, `LTCH`, `LTCH_4`, `STORE_8`, `COUNT_BR`, `COUNT_DR`, `TOGGLE`, `FF_D2E`, `FF_D4E`, and `FF_DRE`.
- `libraries/oscat` now also ships the rest of the current logic surface: `FF_JKE`, `FF_RSE`, `SELECT_8`, `SHR_4E`, `SHR_4UDE`, `SHR_8PLE`, `SHR_8UDE`, the full current gate-logic helper slice (`DEC_*`, `MUX_*`, `BIT_*`, `BYTE_*`, `WORD_*`, `DWORD_*`, `SHL1`, `SHR1`, `SWAP_*`, `REAL_TO_DW`, `DW_TO_REAL`, `CHK_REAL`, `REFLECT`, `REVERSE`), plus generator trigger FBs `A_TRIG`, `B_TRIG`, and `D_TRIG`.
- `libraries/oscat` now also ships the full current `/Logic/generators` and `/Logic/memory` slices: `CLICK_CNT`, `CLICK_DEC`, `CLK_DIV`, `CLK_N`, `CLK_PULSE`, `CYCLE_4`, `GEN_BIT`, `GEN_SQ`, `SCHEDULER`, `SCHEDULER_2`, `SEQUENCE_4`, `SEQUENCE_8`, `TONOF`, `TP_X`, `FIFO_16`, `FIFO_32`, `STACK_16`, and `STACK_32`.
- `libraries/oscat` now also ships the `/Logic/Others`, `/Buffer Management`, `/List Processing`, `/Mathematical/Geometry`, and `/Mathematical/Double Precision` slices: `CRC_GEN`, `MATRIX`, `PIN_CODE`, `_BUFFER_*`, `BUFFER_*`, `LIST_*`, `REAL2`, `R2_*`, `CIRCLE_*`, `CONE_V`, `ELLIPSE_*`, `SPHERE_V`, and `TRIANGLE_A`.
- Added `docs/guides/OSCAT_LIBRARY_GUIDE.md` plus an expanded `examples/oscat_smoke` walkthrough so the shipped OSCAT package now has a user-facing reference and a concrete consumer guide comparable to the motion-library docs.
- Runtime benchmark scripts now share a documented host-codegen policy via `TRUST_RUNTIME_HOST_CODEGEN=auto|generic|native`, defaulting to host-native builds only on Raspberry Pi benchmark hosts while keeping portable/generic builds explicit for shared comparisons and release artifacts.
- User-facing build docs now also cover the full host-native and PGO runtime build workflow, including the required `llvm-tools-preview` setup, corpus-training commands, and current Raspberry Pi 5 benchmark evidence (`full_demo` about `433.501 us -> 404.353 us` with `native`, then about `292.298 us` with `native+PGO`).
- Added `docs/diagrams/architecture/runtime-bytecode-vm-execution.puml` and corrected the high-level runtime architecture notes so the official diagrams now distinguish production bytecode-VM execution from residual `EvalContext` / `legacy-interpreter` helper paths.
- `trust-runtime bench project` now benchmarks a real project folder with watched-global capture, cycle-budget reporting, per-cycle latency summaries, and budget-overrun counts, so reusable Structured Text libraries can be validated through user-facing example projects instead of only synthetic fixtures.
- Added `examples/plcopen_motion_single_axis_demo` as the reference consumer for `libraries/plcopen_motion/single_axis_core`, plus `scripts/runtime_motion_example_bench_gate.sh` for focused end-to-end semantic + cycle-budget validation of the shipped PLCopen motion surface.
- Added `examples/plcopen_motion_single_axis_benchmarks` plus `scripts/runtime_motion_benchmark_breakdown.sh` to isolate runtime floor, `MC_Constants()`, status/readback, inactive-command, single-command, and constants-once motion costs on the same release benchmark path.
- `scripts/runtime_motion_benchmark_breakdown.sh` now forces a fresh `cargo build --release -p trust-runtime --bin trust-runtime` before benchmarking so release measurements cannot accidentally reuse a stale `target/release/trust-runtime` binary.
- `trust-runtime bench project` now emits VM register-profile details for VM-backed workloads, including hot blocks, fallback reasons, and lowering-cache counters, so project benchmarks can explain where scan time is going instead of only reporting latency totals.
- `trust-runtime bench project` now emits `vm_profile.ref_ops` and `vm_profile.call_ops` in JSON/table output, and `scripts/runtime_motion_benchmark_breakdown.sh` now summarizes those counters per workload so VM performance investigation runs show ref-path and FB/frame activity directly instead of hiding it in raw runtime state.
- `trust-runtime bench project` now also emits `vm_profile.value_ops` in JSON/table output, and `scripts/runtime_motion_benchmark_breakdown.sh` now summarizes value-clone/move attribution (const loads, register reads, read-side value clones, binding expr clones, output copy-back clones) so runtime speed work can distinguish read-path churn from register-file churn.
- `trust-runtime bench project` now also emits `vm_profile.tier1_specialized_executor` in JSON/table output, and the experimental tier-1 compiler now supports `RefField`, `RefIndex`, `LoadDynamic`, and `StoreDynamic` so ref-heavy FUNCTION_BLOCK blocks show up in benchmark compile/deopt/execution evidence instead of staying invisible behind `null` tier-1 stats.
- `VariableStorage` now caches instance-field resolution per instance, with invalidation on new instance-field insert, so repeated FUNCTION_BLOCK self-field access and parameter binding can skip repeated field-name lookup without assuming that all same-type instances always share the exact same runtime layout.
- VM FUNCTION_BLOCK argument binding now uses a direct declared-parameter offset fast path with recursive fallback for inherited or unusual fields, trimming repeated binding overhead on ref-heavy release benchmarks without changing call semantics.
- VM output copy-back and empty-path storage access now use direct-slot `VmWriteTarget` / `VariableStorage` fast paths for locals, globals, and instances, reducing generic `ValueRef` resolution on the hot benchmark path without changing runtime semantics.
- VM `IN` arguments that arrive as empty-path target references now reuse the same direct-slot `VmWriteTarget` read path for locals, globals, and instances, trimming generic reference reads on ref-heavy hot paths without changing call semantics.
- VM dynamic reference shape inspection now borrows storage/local values for `REF_FIELD` and `REF_INDEX` traversal instead of cloning them first, which materially improves ref-heavy release motion workloads without changing runtime semantics.
- VM hot paths now also borrow output target type inspection for `write_output_int()` and use borrowed DINT fast paths for fused ref+const/register guards in `register_ir`, trimming unnecessary clone/materialization work on ref-heavy micro-benchmarks without changing runtime semantics.
- VM FUNCTION_BLOCK binders now skip field resolution when `OUT`/`IN_OUT` parameters are omitted, use an ordered named-argument fast path for the common declaration-order case, and decode native call payloads in pop order instead of reversing/removing a payload vector on every call.
- Runtime `Value::Struct` now uses shared-on-clone copy-on-write storage, which removes the remaining deep-clone hot path for PLCopen-style `AXIS_REF` `VAR_IN_OUT` transfers; on the rebuilt motion demo gate this cuts `read_value_clones` from `6467` to `227`, eliminates `output_value_clones`, and improves release latency to about `p50 424 us` / `p95 649 us` with clean semantics.
- `trust-runtime bench project` now keeps VM profiling enabled by default while leaving the experimental tier-1 specialized executor opt-in via `--tier1`, so normal speed baselines stay on the primary VM path and diagnostic tier-1 cache/compile/deopt evidence is still available when explicitly requested.
- `trust-runtime bench project` and `scripts/runtime_motion_benchmark_breakdown.sh` now also surface tier-1 compile-failure reasons alongside deopt reasons, so remaining specialization misses can be attributed to exact unsupported instruction/op buckets instead of only a generic `compile_failures` count.
- The experimental tier-1 VM path now also compiles `LoadRefAddr` and `CallNative`, and widens binary specialization to the generic non-DINT fallback path (including `AND`/`OR`), so the motion benchmark matrix now reaches `compile_failures = 0` / `deopts = 0` in tier-1 profiling runs while preserving clean motion semantics.
- VM ref-binary execution now reuses the first borrowed ref/const read on non-DINT fallback in both the interpreted register executor and the tier-1 executor, removing duplicate storage reads and bogus clone accounting on BOOL/REAL/UINT/WORD-style ref binaries; the rebuilt motion breakdown improves over the tier-1-complete state (`dynamic_refs 34.834 us`, `full_demo 478.836 us`) while preserving clean motion semantics.
- The register VM and experimental tier-1 executor now lower and execute `LOAD_SUPER (0x24)`, so method receiver / `SUPER` call paths no longer fall back through `unsupported_opcode_0x24` on the current-code benchmark surface.

### Changed

- `trust-runtime test` now prepares one runtime per project and reuses it across discovered cases instead of recompiling the whole ST project for every case, which removes false timeout growth on larger OSCAT fixtures.
- truST now accepts direct `CHAR_TO_BYTE` / `WCHAR_TO_WORD` conversion-style helpers, and the shipped OSCAT `CODE()` helper now uses that fast path instead of a brute-force byte scan; the OSCAT core fixture is back to passing at the default `--timeout 5`.
- `trust-runtime` is now VM-only in production: the `legacy-interpreter` feature, interpreter backend dispatch seam, and interpreter runtime paths are gone; CLI/config startup selection still accepts `vm` and now rejects `interpreter` explicitly.
- Removed `trust-runtime bench execution-backend` and the interpreter differential gate flow; VM-only syntax-corpus, determinism/reliability, and production-backend guard scripts now carry the runtime evidence path.
- Hardened `scripts/runtime_vm_syntax_corpus.sh` so the default corpus skips stale missing fixture folders instead of aborting halfway through a benchmark run.
- Runtime docs/diagrams/specs now describe `program_model` plus bounded `helper_eval` helpers as the surviving support surface around the VM, and executor internals under `src/eval/**` are confined to test-only coverage.
- Rewrote the user-facing PLCopen motion documentation so the single-axis demo README now explains the example itself and the main library guide now serves as a real reference manual with package wiring, datatype purpose tables, and per-function-block input/output coverage.

- Structured Text `FUNCTION_BLOCK` calls now reuse the previously stored `VAR_INPUT` value when an input argument is omitted, with first-call fallback still using the declared initializer or IEC default; this closes the motion-library command-update semantics gap without inventing separate runtime rules.
- Structured Text indexing now supports nested `field[index]` / `index.field` chains and character indexing on `STRING` / `WSTRING`, so OSCAT-style `LANGUAGE.DIRS[ly, i]` access and IEC string element forms compile and execute without fallback workarounds.
- Web IDE build/deploy behavior now matches CLI semantics in unified shell flows: build tasks use project-root `--project` resolution, deploy normalizes `src/`-prefixed source paths to avoid nested `src/src` writes, and online connection defaults now seed same-origin host/port for faster standalone startup.
- Web IDE header controls now use a compact primary toolbar (`Open`, `Save`, `Build`, `Deploy`) with overflow menu actions for lower visual clutter while preserving the full command set.
- VS Code HMI widget-navigation source scanning fallback was merged on top of current mainline traversal logic so declaration resolution stays deterministic for `.st`/`.pou` projects after main-branch merge.
- MP-060 production backend policy now enforces VM-only startup for `trust-runtime run`/`play`: `--execution-backend` accepts `vm` only, `runtime.execution_backend='interpreter'` is rejected by runtime config validation, omitted backend config defaults to `vm`, and run/play startup now fails fast when a legacy interpreter backend is present in runtime bundle settings.
- MP-060 setup/wizard runtime templates now emit `runtime.execution_backend = "vm"` by default so newly generated `runtime.toml` files validate and start under VM-only production policy.
- MP-060 no-bundle and source-compiled runtime startup now always applies compiled bytecode before execution backend selection so VM-only startup has loaded bytecode metadata in both project and IDE-shell startup flows.
- MP-060 interpreter internals are now explicitly feature-gated behind `legacy-interpreter` for parity-only workflows (differential/benchmark/debug parity tests), while default production builds remain VM-only.
- MP-060 runtime/harness defaults now execute through VM paths with automatic bytecode materialization from runtime metadata when VM bytecode has not been preloaded, preventing implicit interpreter execution in default runtime/test flows.
- MP-060 CI now enforces a production backend leak guard (`scripts/runtime_vm_production_backend_guard.sh`) to fail if run/config/control/template/runtime-default surfaces regress to interpreter references.
- MP-060 migration/docs/checklists updated for restart execution: added detailed recovery board `docs/internal/testing/checklists/mp-060-vm-restart-recovery-checklist.md` and updated backend migration/spec/master-plan text to reflect interpreter test-oracle-only policy.
- MP-060 post-C1 recovery pass implemented for register VM hot path (`P0 -> P2 -> P1`): boxed large runtime value variants (`Array`/`Struct`/`Enum` with unboxed `Reference`), added extended register-op fusion (`BinaryRefToRef`, `BinaryRefConstToRef`, `BinaryConstRefToRef`, `CmpRefConstJumpIf`) with tier-1 specialized-executor support, and added consume-aware per-block register read paths; refreshed locked `mp-060-corpus-v3` 3-run benchmark evidence and comparison artifacts in `target/gate-artifacts/runtime-vm-bench-v3-post-p0-p2-p1-run1/`.
- MP-060 post-hotpath correction pass: `execution-backend` benchmark corpus upgraded to `mp-060-corpus-v4` with per-cycle loop-state reset in `loop-arith`, VM profile guardrails now assert loop-body execution during measured cycles, benchmark comparison now uses 3-run median-of-runs decision metrics (`scripts/runtime_vm_bench_compare.sh`) with aggregate median derived from per-fixture medians (instead of pooled cross-fixture sample p50), and register-IR `CALL_NATIVE` now reuses a program-level pooled operand stack (removing per-call stack allocation churn) alongside cached per-program read metadata and direct block-id indexing in the register executor.
- Runtime specs/docs synchronized to VM-default production backend policy: `docs/specs/10-runtime.md` and `docs/specs/README.md` now describe bytecode VM execution as the primary runtime path, with interpreter execution documented as legacy `legacy-interpreter` parity/test-oracle flow only.
- `trust-runtime plcopen import` now defaults to native CODESYS/TwinCAT-style global-list materialization: file-scope GVLs stay as `VAR_GLOBAL` files, `qualified_only` lists import as namespaced GVLs, and mandatory `VAR_EXTERNAL` injection is no longer the default import shape. A strict adapter mode remains available for wrapper + injected-`VAR_EXTERNAL` reshaping when external consumers need it.
- The canonical PLCopen motion demo now initializes `MC_Constants()` once during startup instead of republishing its outputs every scan, so the reference benchmark reflects the cheaper steady-state usage pattern already documented by the library guide.
- Register-IR hotpath coverage now includes `LOAD_SELF (0x23)`, complex local-ref execution, and CASE block-entry stack modeling, project-bench VM hot-block reports resolve readable POU names, and lowering-cache fallbacks now preserve the failing POU name plus the original lowering error message. On the PLCopen motion demo this removes the old `unsupported_opcode_0x23`, `complex_local_ref_path`, and `Main` lowering-error buckets and keeps the release gate comfortably under the 10 ms cycle budget with `0` VM fallbacks.

### Fixed

- Runtime and VM execution now handle nested field/index lvalue chains, string and WSTRING dynamic indexing, split-local/initializer parity, and debugger force/release on direct instance fields consistently across harness, VM, helper-eval, and debug-adapter paths.
- HIR/syntax now parse and type-check `TIME()` as a zero-argument builtin, allow `STRING`/`WSTRING` indexing in expressions, accept string `CASE` labels and fixed-length string comparisons, and document the shipped implementer conversion extensions (`TIME_TO_DWORD`, `DWORD_TO_TIME`, `CHAR_TO_BYTE`, `WCHAR_TO_WORD`).
- `trust-ide` / `trust-lsp` now surface the shipped pointer/`ARRAY[*]`/IEC `CONSTANT` semantics consistently across completion, hover, semantic tokens, signature help, workspace symbols, inline values, refactor const-analysis, and constant-section diagnostics instead of drifting behind the HIR fixes.
- Project analysis now honors cross-file `VAR_GLOBAL CONSTANT` integer expressions in type-length contexts such as `STRING[MaxLen]`, and the OSCAT BASIC CRC/buffer helpers now use the shipped `ARRAY[*]` / `POINTER TO ARRAY[*]` compatibility forms without giant caller-side ceremony arrays.
- IEC `CONSTANT` qualifier handling now preserves parameter/`VAR_TEMP` identity in HIR, rejects writes through the shared `ConstantModification` path, blocks function block instances in `CONSTANT` sections, and keeps parameter/`VAR_TEMP` declarations out of compile-time constant-expression evaluation.
- Bytecode VM lowering now executes `EXIT` and `CONTINUE` correctly in `FOR`, `WHILE`, and `REPEAT` loops instead of rejecting those statement paths through the generic C5 fallback.
- HIR comparison rules now treat same-family `STRING` and `WSTRING` values as comparable regardless of declared max length, so fixed-length values such as `STRING[1]` and `WSTRING[1]` compare cleanly against compatible literals and variables.
- Pointer support docs and regression coverage now explicitly lock the supported string-pointer path (`ADR(str)` with typed dereference/indexing such as `p^[i]`) while keeping raw byte-array reinterpret casts and pointer arithmetic outside the supported runtime model.
- `CASE` now accepts `STRING` / `WSTRING` selectors and literal labels in runtime/lowering paths, with an explicit `IEC-DEC-024` policy that string subranges remain rejected as an implementer decision for the ambiguous IEC area.
- FUNCTION_BLOCK omitted `VAR_INPUT` calls now follow the documented runtime semantics: instance creation seeds declared input initializers, first omitted calls observe that seeded value, and later omitted calls reuse the stored instance input instead of reevaluating the declaration on every call.
- `trust-runtime bench project` now applies the project's configured runtime execution backend and the motion example bench gate accepts `TRUST_MOTION_MAX_OVERRUNS` / `TRUST_MOTION_P95_MAX_US` overrides for hardware-specific performance gating.
- `trust-runtime test` now executes discovered `TEST_PROGRAM`s even when a project includes a `CONFIGURATION`, while normal configured runtime builds continue to register only configured programs.
- `trust-runtime test --project` now compiles sources from local `[dependencies]` packages in addition to the project's own `src/`, so extracted Structured Text libraries can be consumed from real package roots instead of living inside test fixtures.
- `trust-runtime test` now preloads bytecode before per-case timeout accounting, so filtered project runs do not burn their timeout budget on the first lazy VM-module build.
- Multi-file HIR import now preserves configuration/resource global lookup bindings, so vendor-parity bare global access continues to resolve after splitting projects across source files.
- Runtime assignment writes now preserve declared scalar storage types in both interpreter and VM paths, so loop counters keep their declared `INT`/`UINT`/etc. representation and exact-source conversion calls such as `INT_TO_DINT(...)` no longer fail inside `FOR` and `WHILE` loops.
- Structured Text infix bitwise operators now accept `BOOL`/`ANY_BIT` operands with the same widening behavior as the standard `AND`/`OR`/`XOR`/`NOT` functions, and `&` now type-checks as the `AND` synonym instead of falling through as `UNKNOWN`.
- `VAR_STAT` now executes with documented vendor semantics in the runtime: function statics persist across calls, method statics persist per instance/method, and instance-bearing scopes treat `VAR_STAT` as persistent instance storage.
- VM execution now initializes declared local storage before user bytecode runs, so local declaration initializers and split-style standard-library writes to function-local outputs behave the same way in runtime and VM paths.

### Added

- Shipped the first PLCopen Motion library profile in Structured Text: Part 1 single-axis classic core, Part 1 synchronization cam/gear subset, Part 4 coordinated-motion core subset, and Part 5 homing core subset, with a compliance matrix, user-facing guide, deferred-surface guards, and deterministic ST conformance fixtures.
- truST now accepts namespaced vendor-style GVL declarations (`NAMESPACE ... VAR_GLOBAL ... END_NAMESPACE`) and resolves qualified access such as `GVL.shared` in both HIR and runtime execution.
- truST now records vendor-parity global access explicitly: top-level GVLs, namespaced GVLs, and direct global access without mandatory `VAR_EXTERNAL` are documented and covered by regression tests.
- `trust-harness` now provides a lightweight JSON-line test driver for compiled ST programs, including `cycle.dt_ms` virtual-time advancement and typed `TIME`/`LTIME` watch output for timer-oriented automation.
- LSP/type-checker numeric hazard warnings now flag floating-point equality/inequality comparisons (`W013`) and `DIV`/`MOD` expressions with literal zero divisors (`W014`), with a dedicated `warn_numeric_hazards` diagnostics toggle for vendor/workspace tuning.
- VS Code SFC visual editor integration (IEC 61131-3 style step/transition canvas, runtime panel wiring, and bundled EtherCAT Snake SFC examples) is now included in mainline extension workflows.
- MP-060 Phase A execution-backend controls for `trust-runtime`: new `runtime.execution_backend` config (`interpreter|vm`), new CLI override `--execution-backend=interpreter|vm` for `run`/`play`, startup backend-selection log event, backend mode/source fields in control `status`/`config.get`, and Prometheus backend info metric (`trust_runtime_execution_backend_info`). `vm` selection is intentionally fail-fast until VM execution lands in the next MP-060 phase.
- MP-060 Phase B VM core for `trust-runtime`: `runtime.execution_backend='vm'` now runs a real bytecode executor with deterministic program-counter dispatch, operand and call/frame stacks, trap/deadline/budget enforcement, stable trap-to-`RuntimeError` mapping, and slot/index-based VM hot-path access with symbol/source mapping tables preserved for external/debug name-based surfaces. Interpreter remains the default backend.
- MP-060 Phase C1 call-parity rollout for `trust-runtime`: VM codegen/dispatch now emits and executes `CALL_NATIVE` (`kind/symbol/arg_count`) for function/FB/method/stdlib paths, routes builtin FB and stdlib behavior through the same native-call convention, preserves named/default/IN_OUT parity, and replaces C1-required silent NOP call fallthrough with deterministic compile/validate/runtime failures.
- MP-060 Phase C2 string parity rollout for `trust-runtime`: VM bytecode lowering/execution now supports `STRING`/`WSTRING` literals (including stdlib call arguments and comparison expressions), adds deterministic UTF validation for string const payloads, and removes remaining C2 silent-NOP fallback behavior for string-literal control-flow lowering paths.
- MP-060 Phase C3 OOP parity rollout for `trust-runtime`: VM lowering/runtime now supports explicit `THIS`/`SUPER` receiver expressions (`LOAD_SELF`/`LOAD_SUPER`) for method/native-call paths, adds deterministic `SUPER` trap semantics, and extends differential/opcode conformance coverage for method/interface dispatch including malformed method receiver payload rejection.
- MP-060 Phase C4 reference/deref parity rollout for `trust-runtime`: VM lowering/runtime now supports `REF(...)` lowering, dereference execution (`LOAD_DYNAMIC`/`STORE_DYNAMIC`), and nested field/index chain composition (`REF_FIELD`/`REF_INDEX`) with deterministic validator/runtime failures for invalid field-string indices and malformed dynamic-reference operands.
- MP-060 Phase C5 parity rollout for `trust-runtime`: VM lowering/runtime now supports `SIZEOF(type)`/`SIZEOF(expr)` (`SIZEOF_TYPE`/`SIZEOF_VALUE`), adds deterministic validator/runtime opcode failure coverage for C5 paths, and replaces silent VM-lowering fallthrough for remaining C5 edge statements (`?=`/`JMP`/`RETURN`/`EXIT`/`CONTINUE`) with deterministic compile-time rejection.
- MP-060 Phase C audit follow-up hardening for `trust-runtime`: added per-subphase negative-path interpreter-vs-VM differential coverage (C1..C5), extended C2 string stdlib parity coverage to include both `FIND` found/not-found paths, and added a deterministic `SIZEOF_TYPE` recursion-depth guard (`max depth 128`) for deep non-cyclic type-table graphs.
- MP-060 Phase D debug/observability parity for `trust-runtime`: VM execution now resolves statement stops through bytecode debug-map `(pou_id, pc)` source mappings, preserves breakpoint/step and debug-write workflow parity with interpreter mode, and keeps backend metrics/runtime-event observability contracts stable across interpreter and VM backends.
- MP-060 Phase E online-change groundwork for `trust-runtime`: hot-reload semantics are now centralized in a dedicated runtime online-change contract (`apply_online_change_bytes`) with cycle-boundary swap handling, warm-restart entrypoint invalidation, retain/global/instance migration coverage, deterministic invalid-bytecode diagnostics, and explicit startup-only backend-switch diagnostics for live `config.set` requests.
- MP-060 Phase E rollout-gate closure: added a dedicated backend migration note (`docs/guides/RUNTIME_EXECUTION_BACKEND_MIGRATION.md`) covering CLI/config rollback controls and the two-release interpreter compatibility window policy, plus explicit release-evidence gate documentation for `version-release-guard` artifact aggregation.
- MP-060 benchmark groundwork: added `trust-runtime bench execution-backend` (`mp-060-corpus-v1`) for interpreter-vs-VM cycle latency/throughput comparison, plus CI evidence capture via `scripts/runtime_vm_bench_gate.sh` and published artifact/docs pointers (`gate-artifacts/runtime-vm-bench/**`, `docs/internal/reports/mp-060-runtime-vm-benchmark-corpus.md`).
- MP-060 Phase C hotspot profiling: added runtime-scoped register-VM profile controls/snapshot APIs and extended `trust-runtime bench execution-backend` fixture reports with `vm_profile` details (register execution/fallback counters, fallback reasons, top hot blocks, and profiling-overhead ratio).
- MP-060 Phase D tier-1 specialized register-executor pilot: added an experimental hot-block specialization tier in the register VM (guarded compile subset for arithmetic/compare/branch, deopt-to-register-interpreter fallback, cache capacity/eviction controls, and bytecode-reload invalidation), plus benchmark artifact fields for tier-1 specialized-executor counters/deopt reasons and focused unit coverage for startup/deopt/cache behavior.
- MP-060 register executor now caches lowered register IR per `(module,pou)` with runtime-owned invalidation on bytecode reload, eliminating per-cycle relowering and block-map allocation; benchmark reporting now includes lowering-cache hit/miss/eviction stats and uses expanded `mp-060-corpus-v2` fixtures (`call-binding`, `string-stdlib`, `refs-sizeof`, `loop-arith`).
- IDE settings coverage expanded for standalone/offline authoring: new fields for control endpoint/resource name, runtime cloud profile, extended OPC UA options, and observability options.
- IDE settings discoverability improvements: `/ide/settings` now opens in an `All Settings` view by default and adds filter-by-name/key search so MQTT/PLC/TLS/realtime/debug configuration is immediately accessible.
- IDE settings reliability hardening: `/ide/settings` now includes standalone `simulation.toml` controls (`simulation.enabled`, `simulation.seed`, `simulation.time_scale`), shows active filter summary/clear controls to prevent hidden-field confusion, and limits live `config.set` writes to backend-supported keys to avoid false save failures.
- IDE settings navigation hardening: hardware/quick-action jumps now auto-clear stale settings filters and auto-fallback to the owning category (or `All Settings`) so target fields are always visible when jumping to PLC/MQTT/TLS/realtime/debug settings.
- IDE I/O settings completeness hardening: both Settings and Hardware editors now preserve `use_system_io` from loaded I/O configs instead of silently forcing it to `false` on save.
- Default standalone Web IDE demo project (`examples/web_ui_complete_project`) now includes a multi-driver `io.toml` (Modbus TCP, MQTT, EtherCAT mock, GPIO, Simulated) so Hardware and Settings communication flows are populated immediately.
- IDE communication workflow hardening in the unified shell:
  - Settings now shows inline JSON examples for `runtime_cloud.wan.allow_write_json` and `runtime_cloud.links.transports_json`.
  - Realtime link rule parsing now accepts `source/target`, `from/to`, and `pattern` aliases for faster authoring while still writing valid `runtime.cloud.links.transports` TOML entries.
  - Runtime-cloud link transport support now spans `realtime`, `zenoh`, `mesh`, `mqtt`, `modbus-tcp`, `opcua`, `discovery`, and `web` across runtime schema validation, config APIs, and the Hardware/Settings editors.
  - Hardware `Add Link` flow now uses an in-UI transport picker (all 8 transports) plus guided source/target hinting instead of raw text prompt input.
  - Hardware inspector now deep-links all communication/runtime-cloud modules to their exact Settings fields.
  - Added MQTT connectivity probe route `POST /api/io/mqtt-test` and Hardware inspector `Test Connection` support for MQTT alongside Modbus TCP.
  - Hardware communication cards now also project `runtime.cloud.wan.allow_write` as `Cloud WAN Access`.
- IDE mobile responsiveness hardening for `/ide`: stacked layout now keeps the main workspace visible on narrow screens and avoids sidebar-only view lockups.
- Fixed settings schema mismatch for Modbus `on_error`: UI options now match runtime driver-supported values (`fault`, `warn`, `ignore`).
- Unified Web IDE shell with tab-based navigation (Code | Hardware | Settings | Logs):
  - Root `/` and `/setup` now redirect to `/ide`.
  - Legacy fleet web routes removed from the runtime web server (`/fleet`, `/app.js`, `/styles.css`, `/runtime-cloud-utils.js`, and legacy `/modules/*` fleet assets).
  - Added standalone browser IDE startup via `trust-runtime ide serve --project <path> --listen <addr>` (with deprecated alias `trust-runtime config-ui serve`).
  - Tab deep links: `/ide/code`, `/ide/hardware`, `/ide/settings`, `/ide/logs`.
  - Keyboard navigation: Ctrl+1..4 for tab switching, URL history with pushState.
  - Hardware tab: Cytoscape topology canvas, module palette with drag-and-drop, auto-address allocation, property editor with per-driver forms (Modbus TCP, MQTT, GPIO, EtherCAT), address map table with conflict/unused/used-in-code highlighting, live I/O value polling.
  - Hardware tab now auto-hydrates from workspace `io.toml` via new `/api/ide/io/config` endpoint so configured hardware/communication drivers appear on the canvas immediately (including deep-link `/ide/hardware` loads).
  - Hardware tab now renders a full workspace layout (summary KPI cards, surface controls, driver/runtime communication cards, persistent inspector), fixes tab-shell row sizing so content fills the page (status bar no longer consumes the main viewport), and hydrates runtime communication sections from `runtime.toml` into the hardware communication view.
  - Hardware tab now also hydrates from `runtime.toml` communication sections when `io.toml` is missing, so standalone projects still show communication modules on the canvas instead of an empty hardware view.
  - Settings tab: categorized form (General, Execution, Retention, Communication, Security, Advanced), direct runtime.toml editing, settings export/reset.
  - Logs tab: unified log view with severity/source/text filtering, alarm acknowledgment, CSV export.
  - Logs CSV export filenames now follow the user-story format exactly: `truST-logs-YYYY-MM-DD-HHMMSS.csv`.
  - Connection dialog: mDNS discovery scan, manual connect, recent connections.
  - Deploy flow: error gating, confirmation, sync status indicator (in-sync/modified/not-deployed).
  - Debug integration: breakpoints with gutter decorations, debug toolbar (Continue/Step Over/Step Into/Step Out/Stop), variables/call stack/watch panels, live inline value annotations, I/O force/unforce with warning banner.
  - New IDE modules: `ide-tabs.js`, `ide-hardware.js`, `ide-online.js`, `ide-debug.js`, `ide-settings.js`, `ide-logs.js`.
  - New CSS: `ide-06.css` (tabs), `ide-07.css` (hardware), `ide-08.css` (online/debug), `ide-09.css` (settings/logs).
  - 5 new unified shell contract integration tests in `web_ide_integration_part_09.rs`.
  - IDE session bootstrap now recovers reliably in standalone mode by persisting/reusing session tokens across reloads and evicting the stalest inactive session when the session cap is reached (prevents repeated `/api/ide/session` 429 lockouts during active web-UI development).
  - Hardware canvas labels now compact long address ranges and enable wrapped node text to keep TOML-derived module/address data legible on the graph.
  - Hardware Cytoscape theming now resolves IDE CSS variables to concrete colors and updates on theme changes, eliminating runtime style warnings while preserving light/dark parity.
  - Hardware tab visual redesign now uses a canvas-first layout with a much larger default graph surface, collapsible inspector/driver panels, in-canvas communication legend/filter controls, center/fullscreen actions, and SVG-based component iconography (replacing emoji palette glyphs) for a modernized runtime topology editing workflow.
  - Hardware node art polish pass now upgrades node cards with richer per-protocol gradients/glows, glass label treatment, and higher-fidelity SVG chip icons (including distinct analog/communication module icon rendering) without changing interaction workflow.
  - Hardware node visuals now render generated card-style SVG backgrounds per node (runtime/endpoint/module variants with accent rails + icon chips) and enforce higher minimum zoom/readability so topology labels/icons stay legible at dense graph sizes.
  - Hardware canvas visual hardening now uses Cytoscape-safe SVG icon rendering for node cards (fixes missing icon layers in Chromium canvas rendering) and retunes card dimensions/colors for a cleaner production graph look.
  - Hardware tab chrome simplification removes summary KPI cards, runtime/fabric control strip, and active-link/legend toolbar from the canvas stage so palette + topology become the primary workflow surface.
  - Hardware node context menu now de-duplicates endpoint settings navigation: endpoint nodes expose only one settings jump action while runtime nodes keep separate runtime and communication settings actions.
- Runtime cloud multi-host topology demo pack:
  - Added `examples/runtime_cloud/multi_host_topology_demo/` with `bootstrap.sh`, `start-5.sh`, and `stop-5.sh`.
  - Added per-runtime sample configs for 5 runtimes (`runtime-a`..`runtime-e`) with mixed `mqtt`, `modbus-tcp`, `ethercat`, `simulated`, and `loopback` I/O profiles.
  - Added preloaded manual topology seed (`topology-devices.runtime-a.json`) to exercise host containers, manual devices, and module-slot rendering during UI testing.
  - Changed bootstrap defaults to TOML/API-first: manual topology JSON and legacy link-transport JSON are now opt-in only (`TRUST_TOPOLOGY_DEMO_SEED_MANUAL=1`, `TRUST_TOPOLOGY_DEMO_SEED_LINK_JSON=1`).
  - Added TOML runtime-cloud link preference support via `runtime.cloud.links.transports` and seeded demo link lanes from `runtime-a.toml`.
  - Added TOML discovery host grouping via `runtime.discovery.host_group` and host-group-aware runtime-cloud same-host evaluation for localhost multi-host demos.
- Runtime cloud topology visual map in Web UI:
  - Reworked `Network -> Topology` into a graphical runtime map with box nodes and connection lines.
  - Added at-a-glance status-focused KPIs (`online`, `degraded`, `offline`, link health) for operator use.
  - Moved diagnostic-heavy controls (filters, tables, timeline, rollout/config control surfaces) into a collapsed advanced section to reduce default clutter.
- Runtime cloud edge transport switching in Fleet view:
  - Clicking topology links or edge cards now allows direct `Realtime` / `Zenoh` transport switching.
  - Added `/api/runtime-cloud/links/transport` with same-host validation for `realtime` routing preferences.
  - Fleet topology state now reflects selected link transport (`t0_hard_rt` overlay for realtime links).
  - Promoted topology to primary workflow with in-canvas plane/keyspace/issue filters, contextual node/edge inspector, and batch action bar for selected runtimes.
  - Moved compact timeline controls into topology map while keeping deep diagnostics under the advanced drawer to reduce default UI clutter.
  - Communication lines now render per-lane overlays (Zenoh + optional Realtime), with left-click showing only critical lane status/settings and right-click lane actions for add/remove realtime communication.
- Fleet topology deterministic ownership slots:
  - Fleet topology now renders runtime-owned driver nodes (`mqtt`, `modbus`, `ethercat`, `opcua`, `gpio`) between runtimes and endpoints.
  - Host assignment is enforced for operational endpoints: remote/discovered endpoints attach to deterministic external host containers instead of free-floating.
  - Topology edit flow now requires host placement for manual devices and blocks host removal that would orphan hosted devices.
  - Runtime cards now reserve deterministic in-card driver slot zones and driver cards render as compact protocol chips anchored to those fixed slots.
  - Empty host containers are hidden in normal view when they have no runtime or endpoint children.
  - Runtime/device cards now render a single border layer (removed Cytoscape outer border) to match card design and avoid double-outline artifacts.
  - Runtime card subtitle no longer duplicates health state when the status badge already shows it.
  - Driver slot placeholders now render without static protocol text; labels appear only on occupied slots.
  - Driver nodes are now non-draggable to preserve deterministic runtime slot placement.
  - Device edges are normalized to bind via runtime-owned driver nodes (including manual runtime->endpoint edges), and render as direct source-to-target lines from the driver chip.
  - Runtime cards now render driver summary text in the body while driver chips are laid out as a single deterministic bottom row (no multi-row chip stacks).
  - Driver chips now include compact protocol context subtitles (for example broker/address/adapter/internal) for faster visual inspection.
  - Host compound labels are pinned inside host containers for clearer ownership headings.
  - Carousel priming now guards missing `runtimeCloudState` to prevent early-page-load `ReferenceError` in browser console.
  - Fleet graph position caches are now layout-versioned and auto-relayout when stale pinned positions overlap runtime cards.
  - External-only host groups now use deterministic anchor lanes so remote endpoint hosts do not collapse onto `(0,0)` and overlap each other.
  - Runtime-internal I/O drivers (`simulated`, `loopback`) now render as deterministic runtime-owned driver cards.
  - Comms add-driver flow now includes `loopback` alongside `simulated` for runtime-internal I/O driver configuration.
  - Modbus canonical endpoint normalization now handles `address` values that already include `:port` (prevents malformed duplicate-port canonical keys).
  - Demo topology seed data now keeps manual endpoints hosted (no hostless manual operational devices).
  - Added config-mode runtime topology writeback endpoints: `POST /api/config-ui/runtime/create` and `POST /api/config-ui/runtime/delete`.
  - Topology edit UI now supports runtime create/remove actions in edit mode and routes adapter actions through TOML-backed config flows.
  - Added config-ui live connection manager APIs (`GET/POST /api/config-ui/live/targets`, `POST /api/config-ui/live/targets/remove`, `POST /api/config-ui/live/connect`, `GET /api/config-ui/live/state`) with in-memory target profiles and read-only runtime-cloud polling.
  - Added config-ui runtime lifecycle provider APIs (`GET/POST /api/config-ui/runtime/lifecycle`) for runtime status/probe plus managed start/stop/restart actions.
  - Config-mode `/api/runtime-cloud/state` now supports read-only live overlay of node/edge health from connected runtime-cloud snapshots without changing TOML-derived topology ownership/layout.
- Added config-ui integration tests for runtime create/delete and ST/runtime conflict-safe writeback paths.
- Removed active topology-devices overlay route dependency; runtime/config topology remains TOML/API-driven.
- PLCopen CODESYS global/folder parity:
  - `trust-runtime plcopen import` now imports CODESYS `addData/globalVars` into ST `VAR_GLOBAL` sources (plaintext-first with variable-node synthesis fallback).
  - CODESYS `addData/projectstructure` object trees are now used to place imported POUs/GVLs into mirrored `src/` subfolders (for example `src/Application/...`).
  - `trust-runtime plcopen export` now emits deterministic CODESYS `globalVars` and `projectstructure` metadata for ST POUs + GVL files.
  - Import/export JSON reports now include global-list and project-structure counters (`discovered/imported_global_var_lists`, folder/object-node counts).

### Changed

- `libraries/oscat/src` and the OSCAT core/negative conformance fixtures now follow the OSCAT manual chapter structure (`03_data_types` through `26_list_processing`) instead of a flat source bucket, so continued porting can proceed chapter-by-chapter with tests in the matching chapter first.
- VS Code statechart automated coverage:
  - Added editor lifecycle test coverage to verify running statechart sessions are cleaned up when a custom editor panel is disposed.
  - Added state machine engine behavior tests for awaited hardware action ordering and fail-closed guard evaluation paths.
  - Added runtime client timeout cleanup coverage to ensure request listeners are removed on timeout/error.
- Web IDE project selection flow:
  - Added `/api/ide/project` and `/api/ide/project/open` to query/switch active project root at runtime.
  - Added no-bundle startup support for `trust-runtime run` so `/ide` can start first and open a project folder from the browser.
  - Added integration coverage for no-bundle project-open flow in `crates/trust-runtime/tests/web_ide_integration.rs`.
- Web IDE workspace scope hardening:
  - Changed IDE file/tree scope from fixed `<project>/src` to full active project root with hidden/system path filtering.
  - Added project-aware command execution guard so build/test/validate require an active project selection.
  - Added task status timestamps in `/ide` task panel (`started`/`finished`) to satisfy workflow traceability requirements.

- Web IDE full implementation closure for `/ide`:
  - Added workspace tree + filesystem mutation endpoints and UI flows (`/api/ide/tree`, `/api/ide/fs/create`, `/api/ide/fs/rename`, `/api/ide/fs/move`, `/api/ide/fs/delete`) with conflict-safe errors.
  - Added IDE navigation/search surface: quick open, workspace text search with include/exclude globs, file/workspace symbol search, and command-palette actions.
  - Added project-aware language navigation endpoints and UI wiring for go-to-definition, references, and rename (`/api/ide/definition`, `/api/ide/references`, `/api/ide/rename`) with cross-file analysis context.
  - Added build/test/validate task orchestration endpoints (`/api/ide/build`, `/api/ide/test`, `/api/ide/validate`, `/api/ide/task`) with streaming output, parsed source-location links, and retry UX in the browser IDE.
  - Added browser IDE format endpoint and command flow (`/api/ide/format`) so formatted content can be applied from the editor command surface.
  - Added filesystem mutation audit trail endpoint (`/api/ide/fs/audit`) and health counters (`fs_mutation_events`) for IDE security observability.
  - Added dedicated Web IDE contract/performance/security integration coverage in `crates/trust-runtime/tests/web_ide_integration.rs` and parser/unit coverage in `crates/trust-runtime/src/web.rs` + `crates/trust-runtime/src/web/ide.rs`.
  - Added a standalone static browser demo at `docs/demo/` with all 7 LSP features running fully client-side through WebAssembly, plus GitHub Pages deployment workflow (`.github/workflows/demo-pages.yml`) for client sharing.
  - Added Browser IDE runtime docs for `/ide` in `docs/guides/WEB_IDE_FULL_BROWSER_GUIDE.md` and linked them from `README.md` and `docs/README.md`.
- EtherCAT bring-up examples:
  - Added `examples/ethercat_ek1100_elx008_v2/` for EK1100 + EL2008-only hardware chains with a validated 8-output snake pattern.
  - Added helper run scripts and docs for real-NIC bring-up and deterministic mock-mode fallback.
- HMI Phase 0 scaffold start:
  - Added runtime scaffold engine APIs `scaffold_hmi_dir` and `scaffold_hmi_dir_with_sources` in `crates/trust-runtime/src/hmi.rs` to generate deterministic `hmi/` directory artifacts from source metadata.
  - Added `trust-runtime hmi init` CLI workflow (`crates/trust-runtime/src/bin/trust-runtime/hmi.rs`) with style selection (`industrial|classic|mint`) and deterministic generation summary output.
  - Added `trust-lsp.hmiInit` `workspace/executeCommand` support in `crates/trust-lsp/src/handlers/commands.rs`, reusing runtime scaffold generation from workspace sources and returning structured summary payloads.
  - Added VS Code LM tool `trust_hmi_init` in `editors/vscode/src/lm-tools.ts` and `editors/vscode/package.json`, routing HMI scaffold generation through the new LSP command path.
  - Added Phase 0 runtime scaffold tests for external symbol filtering, widget mapping by writability/type, deterministic output stability, and repeated-instance section grouping.
  - Converted `docs/internal/testing/checklists/hmi-complete-implementation-checklist.md` into a tracked execution board with active Phase 0 lanes and work item IDs.
- HMI Phase 1.6 live descriptor refresh:
  - Added runtime descriptor watcher (`notify`) with debounce for `hmi/*.toml` updates and safe hot-reload without runtime restart.
  - Added `schema_revision` to `hmi.schema.get` payloads and wired in-memory revision bumps on successful descriptor reload.
  - Added fail-safe invalid descriptor handling that retains the last known-good schema and prevents runtime crashes on bad TOML edits.
- HMI Phase 2.1 section layout foundation:
  - Added schema-driven section rendering in web HMI (`page.sections`) with responsive 12-column spans for sections/widgets.
  - Added explicit fallback to legacy group card rendering when section metadata is absent, preserving existing trend/alarm/dashboard behavior.
  - Added widget renderers for `gauge`, `sparkline`, `bar`, `tank`, `indicator`, `toggle`, and `slider` with value updates and existing `hmi.write` integration for writable controls.
  - Added transition/micro-animation behavior for gauge/bar/tank/indicator/toggle updates and dark-mode CSS variable overrides via `prefers-color-scheme: dark`.
  - Added integration coverage for section layout assets, renderer assets, and schema span metadata used by the web renderer.
- HMI Phase 2.4 live transport foundation:
  - Added `/ws/hmi` websocket endpoint in the embedded web server for push updates (`hmi.values.delta`, `hmi.schema.revision`, `hmi.alarms.event`).
  - Added web HMI client websocket transport with reconnect/backoff and automatic fallback to HTTP polling when websocket is unavailable.
  - Added schema live-refresh behavior in the web client to re-fetch full `hmi.schema.get` payloads on `schema_revision` events.
  - Added integration coverage for websocket event flow and export transport metadata (`/ws/hmi` route + `config.ws_route`).
  - Added websocket hardening integration gates for local latency SLO budgets (p95/p99), forced socket-failure polling recovery, and reconnect churn stability.
- HMI Phase 2.5 process-page foundation:
  - Added process-page schema support (`kind = "process"`) with page-level SVG asset and bind metadata (`svg`, `bindings`) in runtime HMI contracts.
  - Added secure SVG asset serving route (`/hmi/assets/<svg>`) with path containment checks and SVG-only file restrictions.
  - Added process bind parsing guardrails for safe selector/attribute usage plus optional `format`, `map`, and `scale` transform contracts.
  - Added web client process renderer path to inline SVG assets and apply live bind updates from HMI value streams.
  - Added integration coverage for process schema metadata, asset serving, and negative filtering of unsafe/missing bind selectors.
- HMI Phase 2 completion hardening:
  - Added renderer-state regression coverage for all new widget kinds (`gauge`, `sparkline`, `bar`, `tank`, `indicator`, `toggle`, `slider`) across null/stale/good value paths.
  - Added responsive breakpoint regression checks for desktop/tablet/mobile class behavior in the web HMI layout engine.
  - Added process asset-pack integrity checks for `hmi/pid-symbols/` licensing/library presence, stable IDs in `hmi/plant.svg` and `hmi/plant-minimal.svg`, and selector alignment in `hmi/plant.bindings.example.toml`.
- HMI Phase 3 core command/tool surface:
  - Added LSP command `trust-lsp.hmiBindings` with workspace-root/file scoping and deterministic bindings catalog payloads (`programs`, `globals`) including qualifier/writable metadata and available constraints (`unit`, `min`, `max`, `enum_values`).
  - Added VS Code LM tools `trust_hmi_get_bindings`, `trust_hmi_get_layout`, and `trust_hmi_apply_patch` with dry-run conflict reporting for descriptor file operations.
  - Added VS Code command registrations `trust-lsp.hmi.init` and `trust-lsp.hmi.refreshFromDescriptor` and integrated refresh invocation from HMI patch application flows.
  - Added VS Code HMI integration coverage for LM tool valid/invalid payload handling and cancellation behavior.
- HMI Phase 3.6/3.7 completion:
  - Added LSP diagnostics for `hmi/*.toml` files with stable codes for parse failures, unknown bind paths, type/widget mismatches, and invalid widget property combinations.
  - Added near-match suggestion hints for unknown binding paths in HMI descriptor diagnostics.
  - Added multi-root LM-tool integration coverage to verify explicit `rootPath` resolution and write routing for `trust_hmi_apply_patch`/`trust_hmi_get_layout`.
- HMI Phase 4 panel refresh foundation:
  - Expanded VS Code HMI preview refresh relevance to `hmi/*.toml` and `hmi/*.svg`.
  - Added debounced filesystem watching of HMI descriptor/assets (`**/hmi/*.{toml,svg}`) so open panels refresh on descriptor edits without forced auto-open behavior.
  - Added section-aware panel rendering (`page.sections` + `widget_span`) for dashboard pages.
  - Added process-page panel rendering (`kind = "process"`) with safe binding updates and local `hmi/*.svg` asset hydration in VS Code preview.
- HMI Phase 5 export bundle foundation:
  - Updated `/hmi/export.json` payload contract to `version = 2`.
  - Added `config.descriptor` to export payloads with the resolved live `hmi/` descriptor content when present (or `null` when running legacy/no-descriptor projects).
  - Added export integration coverage for descriptor-backed bundles and renderer-capability assertions in exported `hmi/app.js`.
- HMI Phase 6 intent-to-evidence foundation:
  - Added VS Code LM tool `trust_hmi_plan_intent` to generate/update deterministic `hmi/_intent.toml` artifacts with operator goals, personas, KPI priorities, and constraints.
  - Added VS Code LM tool `trust_hmi_trace_capture` for deterministic scenario-tagged API-level runtime trace capture (`hmi.schema.get`/`hmi.values.get`) into `hmi/_evidence/<run>/trace-<scenario>.json`.
  - Added VS Code LM tool `trust_hmi_generate_candidates` for scaffold-rules-based deterministic candidate generation and ranking by readability/action-latency/alarm-salience metrics with `candidates.json` evidence output.
  - Added VS Code LM tool `trust_hmi_validate` to run machine-readable HMI validation checks, emit deterministic `hmi/_lock.json`, and write evidence runs under `hmi/_evidence/<timestamp>/`.
  - Added VS Code LM tool `trust_hmi_preview_snapshot` to emit deterministic desktop/tablet/mobile snapshot artifacts under `hmi/_evidence/<run>/screenshots/`.
  - Added VS Code LM tool `trust_hmi_run_journey` for API/event-level operator journey execution (no headless browser requirement) with pass/fail timing metrics in `journeys.json`.
  - Added VS Code LM tool `trust_hmi_explain_widget` for widget provenance reporting (canonical ID, symbol/type, write-policy allowlist state, and contract endpoint mapping).
  - Hardened `trust_hmi_run_journey` with tool-side write guardrails (read-only/disabled/allowlist checks) and machine-readable step codes before issuing runtime `hmi.write` requests.
  - Added validation retention support (`prune` + `retain_runs`, default 10) and evidence pruning behavior in `trust_hmi_validate`.
  - Added integration tests covering intent generation, candidate-ranking determinism, trace capture, validation artifact emission, snapshot output, API/event journey execution, unauthorized-write safety handling, evidence retention pruning, lock determinism, and cancellation handling for the full Phase 6 toolset.
  - Added runtime hardening regressions for write-cycle budget checks, websocket slow-consumer/backpressure stability, malformed process-SVG resilience, and rapid descriptor file-churn no-deadlock behavior.
  - Added default gitignore rule for `hmi/_evidence/` artifacts.
- Direct Siemens TIA source handoff via export adapters:
  - `trust-runtime plcopen export --target siemens` now emits a Siemens SCL sidecar bundle (`<output>.scl/*.scl`) alongside PLCopen XML.
  - Siemens-target export reports now include `siemens_scl_bundle_dir` and `siemens_scl_files[]` for automation and CI evidence.
  - Added dedicated Siemens import tutorial with exact TIA path (`External source files` -> `Add new external file` -> `Generate blocks`): `docs/guides/SIEMENS_TIA_SCL_IMPORT_TUTORIAL.md`.
  - Added runtime + CLI regression coverage for Siemens SCL bundle generation and `PROGRAM` to `ORGANIZATION_BLOCK` conversion.
- VS Code PLCopen import workflow:
  - Added command `Structured Text: Import PLCopen XML` (`trust-lsp.plcopen.import`) for CLI-backed PLCopen project import from the editor.
  - Command flow prompts for input XML + target project folder, validates conflict paths, runs `trust-runtime plcopen import --json`, and offers quick access to the generated migration report.
  - Added VS Code integration coverage for success/cancel/conflict/invalid-input paths in `editors/vscode/src/test/suite/plcopen-import.test.ts`.
  - Added explicit import usage docs in `README.md`, `docs/README.md`, `editors/vscode/README.md`, and PLCopen example READMEs.
- Guided example/tutorial expansion:
  - Added `examples/README.md` as a structured tutorial index with recommended learning order and setup checklist.
  - Added `examples/tutorials/12_hmi_pid_process_dashboard/` as a full process-HMI tutorial with a runnable PLC model, descriptor-driven `hmi/` pages, production-style `kind = "process"` SVG bindings, setpoint/deviation/alarm coverage, and step-by-step build/run/customization guidance (including screenshot/GIF capture workflow).
  - Added a full walkthrough for `examples/filling_line/README.md` (run, I/O mapping, expected outcomes, and tuning exercise).
  - Reworked example READMEs (`plant_demo`, `memory_marker_counter`, `siemens_scl_v1`, `mitsubishi_gxworks3_v1`, `ethercat_ek1100_elx008_v1`, `plcopen_xml_st_complete`, `tutorials`) into detailed step-by-step VS Code setup guides.
  - Added `examples/vendor_library_stubs/` as a tutorial for user-provided vendor library stub indexing (`[[libraries]]`) with Siemens-style sample declarations.
  - Removed `examples/openplc_interop_v1/` (content absorbed into `examples/plcopen_xml_st_complete/`).
  - Removed `examples/browser_analysis_wasm_spike/` (prototype assets moved to `docs/internal/prototypes/browser_analysis_wasm_spike/`).
- OpenPLC Interop v1 deliverable closure:
  - Added dedicated OpenPLC migration guide: `docs/guides/OPENPLC_INTEROP_V1.md`.
  - Added OpenPLC fixture and walkthrough coverage inside the PLCopen ST-complete bundle:
    - `examples/plcopen_xml_st_complete/README.md`
    - `examples/plcopen_xml_st_complete/interop/openplc.xml`
  - Added regression coverage to keep the OpenPLC sample bundle executable in CI:
    - `crates/trust-runtime/tests/plcopen_command.rs`
    - `crates/trust-runtime/tests/tutorial_examples.rs`
- truST Browser Analysis Spike (Deliverable 10):
  - Added new browser/WASM analysis adapter crate `crates/trust-wasm-analysis/` exposing deterministic diagnostics, hover, and completion APIs for virtual-document analysis.
  - Added JSON boundary wrapper `WasmAnalysisEngine` for worker/browser transport integration (`applyDocumentsJson`, `diagnosticsJson`, `hoverJson`, `completionJson`, `statusJson`).
  - Added parity + performance regression suite against native analysis in `crates/trust-wasm-analysis/tests/mp010_parity.rs`.
  - Added browser worker host prototype assets and build pipeline:
    - `docs/internal/prototypes/browser_analysis_wasm_spike/`
    - `scripts/build_browser_analysis_wasm_spike.sh`
    - `scripts/check_mp010_browser_analysis.sh`
  - Published scope contract and evidence report:
    - `docs/guides/BROWSER_ANALYSIS_WASM_SPIKE.md`
    - `docs/reports/browser-analysis-wasm-spike-20260212.md`
- EtherCAT Backend v1 (Deliverable 9):
  - Added new runtime I/O driver profile: `io.driver = "ethercat"` with EtherCrab-backed hardware transport (`adapter = "<nic>"`) and deterministic mock transport mode (`adapter = "mock"`).
  - Added module-chain process-image mapping contract for Beckhoff-style digital I/O profiles (`EK1100`, `EL1008`, `EL2008`) with size-check diagnostics.
  - Added startup/discovery diagnostics and cycle-time health telemetry (`ok`/`degraded`/`faulted`) with driver error policy handling.
  - Added EtherCAT deterministic integration coverage and runtime example project:
    - `crates/trust-runtime/tests/ethercat_driver.rs`
    - `examples/ethercat_ek1100_elx008_v1/`
  - Published EtherCAT backend guide with scope boundaries and compliance checkpoint:
    - `docs/guides/ETHERCAT_BACKEND_V1.md`
- Mitsubishi GX Works3 Compatibility v1 (Deliverable 8):
  - Added Mitsubishi vendor profile support in LSP tooling (`vendor_profile = "mitsubishi"` and alias `gxworks3`) for formatting, stdlib selection defaults, and diagnostics rule-pack aliases.
  - Added native `DIFU`/`DIFD` edge-alias support in semantic/runtime builtins (mapped to IEC `R_TRIG`/`F_TRIG` behavior) for normal ST authoring and execution.
  - Added Mitsubishi GX Works3 example project: `examples/mitsubishi_gxworks3_v1/`.
  - Added compatibility guide with supported subset, incompatibilities, and migration guidance: `docs/guides/MITSUBISHI_GXWORKS3_COMPATIBILITY.md`.
  - Added dedicated regression coverage across HIR semantics, runtime edge behavior, LSP formatting/diagnostics, and example compile tests.
- Multi-vendor Export Adapters v1 (Deliverable 7):
  - `trust-runtime plcopen export` now supports `--target <generic|ab|siemens|schneider>` for vendor-targeted interchange artifacts.
  - Export JSON contract now includes target adapter evidence fields: `target`, `adapter_report_path`, `adapter_diagnostics`, `adapter_manual_steps`, and `adapter_limitations`.
  - Vendor-target exports now emit deterministic sidecar adapter reports (`<output>.adapter-report.json`) and embedded `trust.exportAdapter` metadata in `addData`.
  - Published target-specific limitations/manual migration steps guide: `docs/guides/PLCOPEN_EXPORT_ADAPTERS_V1.md`.
- Editor Expansion v1 (Deliverable 6):
  - Official Neovim setup pack published with reference `nvim-lspconfig` profile and workflow keymaps: `editors/neovim/`.
  - Official Zed setup pack published with reference language-server profile: `editors/zed/`.
  - Editor setup/validation guide published: `docs/guides/EDITOR_SETUP_NEOVIM_ZED.md`.
  - New editor smoke gate script `scripts/check_editor_integration_smoke.sh` validates editor config contracts and runs targeted LSP workflow tests for diagnostics/hover/completion/formatting/definition.
- Vendor Library Compatibility Baseline (Deliverable 4):
  - `trust-runtime plcopen import` now applies deterministic vendor-library shim mappings for selected Siemens, Rockwell, Schneider/CODESYS, and Mitsubishi aliases.
  - Import/migration JSON contracts now include `applied_library_shims` with vendor/source/replacement/occurrence metadata.
  - Vendor-library compatibility matrix and shim catalog published in `docs/guides/VENDOR_LIBRARY_COMPATIBILITY.md`.
- Siemens SCL Compatibility v1 (Deliverable 3):
  - Siemens-style `#`-prefixed local references now parse in expression and statement contexts (including `FOR` loop control variables).
  - Siemens SCL compatibility guide published: `docs/guides/SIEMENS_SCL_COMPATIBILITY.md`.
  - Siemens SCL example project added: `examples/siemens_scl_v1/`.
  - Regression coverage added across parser, LSP formatting/diagnostics, and runtime example compile tests.
- PLCopen Interop Hardening (Deliverable 2):
  - expanded migration fixture coverage for major ecosystems (`codesys`, `beckhoff-twincat`, `siemens-tia`, `rockwell-studio5000`, `schneider-ecostruxure`)
  - structured unsupported-node diagnostics in migration reports with code/severity/node/action metadata
  - explicit compatibility coverage summary in import/migration reports (`supported_items`, `partial_items`, `unsupported_items`, `support_percent`, `verdict`)
  - dedicated compatibility/limits guide: `docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`
- Conformance Suite MVP shipped (Deliverable 1):
  - deterministic case pack + versioned expected artifacts in `conformance/cases/` and `conformance/expected/`
  - coverage for timers (TON/TOF/TP), edges, scan-cycle ordering, init/reset, arithmetic corner cases, and mapped memory behavior
  - negative/error-path coverage for runtime overflow behavior and unresolved wildcard mapping compile errors
  - external run guide and submission process (`conformance/external-run-guide.md`, `conformance/submissions.md`)
  - explicit known-gaps register (`conformance/known-gaps.md`)
- `trust-runtime conformance` CLI runner mode:
  - deterministic `case_id` ordering
  - machine-readable JSON summary contract (`trust-conformance-v1`)
  - stable failure reason taxonomy (`conformance/failure-taxonomy.md`)
  - `--update-expected` mode for deterministic artifact refresh
- ST unit-testing tutorials:
  - `examples/tutorials/10_unit_testing_101/`
  - `examples/tutorials/11_unit_testing_102/`
- Salsa hardening gates and overnight validation scripts/reports:
  - `scripts/salsa_*_gate.sh`
  - `scripts/salsa_overnight_hardening.sh`
  - `docs/reports/salsa-overnight-hardening-20260209.md`
- Runtime/UI multi-driver coverage and integration tests for Modbus + MQTT.
- New ST assertion functions in runtime/hir:
  - `ASSERT_NOT_EQUAL`
  - `ASSERT_GREATER`
  - `ASSERT_LESS`
  - `ASSERT_GREATER_OR_EQUAL`
  - `ASSERT_LESS_OR_EQUAL`
- `trust-runtime test --list` to discover test names without executing.
- `trust-runtime test --timeout <seconds>` for per-test execution timeout.
- CLI/integration tests for list/filter/timeout behavior and JSON duration fields.

### Changed

- Local developer test workflow acceleration:
  - `just test` now uses `cargo-nextest` for the default local loop (`trust-runtime` library tests), with automatic fallback to `cargo test` if `nextest` is unavailable.
  - Added split test targets in `justfile`: `test-integration`, `test-e2e`, and `test-all` (full previous gate).
  - Enabled faster local linking via `mold` (`.cargo/config.toml`) and enabled `sccache` as Cargo `rustc-wrapper`.
  - Reduced dev/test debug symbol level to `debug = 1` for faster incremental compile/link during local iteration.
- PLCopen CODESYS import hardening:
  - `trust-runtime plcopen import` now maps CODESYS `{attribute 'qualified_only'}` global variable lists into a compiler-valid `TYPE + CONFIGURATION/VAR_GLOBAL` wrapper model instead of emitting unsupported top-level `VAR_GLOBAL` files.
  - Imported POUs that reference qualified lists (for example `GVL.start`) now receive injected `VAR_EXTERNAL` declarations so cross-file global access resolves in trust-lsp/runtime builds.
  - Imported `FUNCTION` POUs without explicit result assignment now get a deterministic fallback self-assignment (for example `MyFunc := MyFunc;`) to prevent `E206 missing return value` on first import.
- Demo Pages WASM startup UX now force-updates the `WASM Ready` badge after engine readiness resolves, preventing a stale `Loading WASM...` state when the worker `ready` event arrives before listener registration.
- Demo WASM analysis clients now send an internal bootstrap `status` request on worker spawn so startup no longer depends on user interaction before `WASM Ready` appears.
- Demo Pages now publishes a root `favicon.ico` and links it from `docs/demo/index.html`, avoiding browser default `/favicon.ico` 404s.
- VS Code statechart extension quality gates now lint TSX sources and run explicit webview TypeScript typechecking (`tsconfig.webview.json`) during compile.
- Statechart import command flow now consistently resolves source/target paths and reuses shared URI/path helpers across statechart commands.
- Statechart hardware helper scripts/docs now use repository-relative paths and group-based socket permissions (`660`) instead of world-writable sockets.
- Statechart operator/developer guides and helper script output are now standardized to English for consistent repository documentation language.
- Web IDE visual styling now aligns with the runtime page sidebar theme treatment (surface, spacing, and footer layout) for a consistent `/` and `/ide` experience.
- Web IDE authoring gating no longer depends on runtime `control_mode=debug`; editor capability is now enforced via web auth role and IDE session role (viewer/editor) at API boundaries.
- Web IDE performance gates were tightened to product targets (completion/hover p95 <= 150ms, diagnostics p95 <= 300ms, workspace search p95 <= 400ms) in automated integration checks.
- Standardized example I/O mapping patterns to use `VAR_CONFIG` bindings (instead of direct `%I/%Q` declarations in `VAR`/`VAR_GLOBAL`) across communication, EtherCAT, Siemens, Mitsubishi, and unit-testing tutorial projects.
- Updated example documentation snippets to reflect `PROGRAM` variables plus `CONFIGURATION`-level `VAR_CONFIG` wiring as the recommended deterministic pattern.
- Browser/WASM position mapping now uses UTF-16 column semantics for protocol compatibility in `trust-wasm-analysis` range/position conversions.
- HMI scaffold update behavior now skips regenerating default `process.toml` when curated custom process pages already exist, and skips creating empty `control.toml` when no writable points are discovered.
- HMI auto-schematic Process scaffold now enforces deterministic grid/anchor rules (shared FIT/PT instrument template geometry, value offset one grid row above sensor centerline, stems snapped to process line, connector-anchored routing) and level-fill bindings now update both `y` and `height` for percent-consistent tank visuals.
- Tutorial `12_hmi_pid_process_dashboard` now uses grid-aligned P&ID SVG layouts (hidden `pid-layout-guides` layer) with compact overview inventory widgets and cleaned page set (no duplicate scaffold `Process` page / empty `Control` page).
- Tutorial `12_hmi_pid_process_dashboard` Process/Bypass P&ID pages now use denser operator layout (integrated status rail, calibrated tank scales, stronger typography hierarchy, and ISA-style symbol/line cleanup) with synchronized level-fill scaling.
- Tutorial `12_hmi_pid_process_dashboard` Process/Bypass P&ID pages now remove duplicated PV summary strip and fix tank header text overlap while keeping tank fill scaling aligned to visible geometry.
- Tutorial `12_hmi_pid_process_dashboard` Process/Bypass P&ID pages now render pipe runs without the extra lane background panel.
- Added grouped communication examples under `examples/communication/` for Modbus/TCP, MQTT, OPC UA, EtherCAT, GPIO, and composed multi-driver setups, and documented protocol transport gates (including `ethercat-wire` unix hardware scope and `opcua-wire` requirement) in examples/tutorial indexes and guide docs (`PLC_IO_BINDING_GUIDE.md`, `PLC_NETWORKING.md`, `PLC_DEVELOPER_GUIDE.md`).
- Restored the field-tested EtherCAT `EK1100 + EL2008` profile as `examples/communication/ethercat_field_validated_es/` (Spanish operator-focused commissioning walkthrough) so communication examples now include two EtherCAT tracks.
- Added automated runtime CLI regression coverage for grouped communication examples via `crates/trust-runtime/tests/communication_examples_cli.rs`, verifying `build --sources src` and `validate` for `modbus_tcp`, `mqtt`, `opcua`, `ethercat`, `gpio`, and `multi_driver` in CI/test runs.
- Added advanced operations tutorial `23_observability_historian_prometheus` and expanded `16_secure_remote_access` with explicit TLS commissioning/validation steps so observability and remote-hardening flows are covered end-to-end in the examples track.
- CI release-gate aggregation now includes a dedicated `Editor Expansion Smoke` gate for Neovim/Zed integration coverage.
- PLCopen XML Full ST Project Coverage (Deliverable 5):
  - Profile advanced to `trust-st-complete-v1`.
  - `trust-runtime plcopen import` now supports full ST-project model import for:
    - `types/dataTypes` (`elementary`, `derived`, `array`, `struct`, `enum`, `subrange`)
    - `instances/configurations/resources/tasks/program instances`
  - `trust-runtime plcopen export` now emits supported ST `TYPE` declarations and configuration/resource/task/program-instance model back into PLCopen XML.
  - Import/export JSON contracts now include deterministic ST-project coverage counters:
    - `data_type_count`, `configuration_count`, `resource_count`, `task_count`, `program_instance_count` (export)
    - `imported_data_types`, `discovered_configurations`, `imported_configurations`, `imported_resources`, `imported_tasks`, `imported_program_instances` (import/migration)
  - Added CODESYS ST-complete fixture packs (`small`/`medium`/`large`) with deterministic expected migration artifacts and CI schema-drift parity gate in `crates/trust-runtime/tests/plcopen_st_complete_parity.rs`.
  - Updated PLCopen compatibility/spec docs and added end-to-end import/export example project in `examples/plcopen_xml_st_complete/`.
- Project source layout is now `src/` only across runtime/CLI flows:
  - `trust-runtime build`, `test`, `docs`, and `run --project` now require `<project>/src` as the default source root.
  - Legacy `<project>/sources` fallback resolution was removed from bundle/project source lookup.
  - Setup/wizard-generated projects and shipped tutorials/examples now use `src/` consistently.
  - PLCopen import/export project-root resolution now targets `src/` only.
  - `trust-runtime setup` auto-migrates an existing `<project>/sources` folder to `<project>/src` when preparing a project.
- `trust-runtime build --sources <relative-path>` now resolves relative source overrides from `--project`, avoiding path-resolution mismatches from external working directories.
- `trust-runtime plcopen export` and `trust-runtime plcopen import` now support `--json` for machine-readable report output.
- `trust-runtime plcopen profile` now publishes a compatibility matrix plus round-trip limits/known-gaps contract fields.
- `trust-runtime plcopen import` compatibility scoring now accounts for shimmed vendor-library aliases as partial-coverage items.
- PLCopen ecosystem detection now recognizes Mitsubishi GX Works markers (`mitsubishi-gxworks3`) for migration reporting/shim selection.
- PLCopen ecosystem detection now recognizes OpenPLC markers (`openplc`) for migration reporting/shim selection, with OpenPLC fixture-backed regression coverage.
- PLCopen vendor-library shim matching now also normalizes keyword-style vendor aliases (for example `R_EDGE`) when used in supported type/call positions.
- Migrated `trust-hir` semantic path to Salsa-only backend and upgraded Salsa to `0.26`.
- Enabled VS Code extension integration tests in CI under virtual display (`xvfb`).
- Expanded cancellation checks in workspace-scale LSP operations.
- CI now includes a dedicated conformance gate with repeated-run deterministic comparison.
- VS Code extension marketplace metadata now declares dual-license SPDX (`MIT OR Apache-2.0`) and monorepo repository directory (`editors/vscode`).
- Documentation organization:
  - Public durable reports remain in `docs/reports/`.
  - Working remediation checklists are no longer published in `docs/reports/`.
- `trust-runtime test` output now reports per-test elapsed time and total elapsed time in human output.
- `trust-runtime test --output json` now includes `duration_ms` per test and in summary.
- Tutorial 10/11 docs updated for list/timeout usage and expanded assertion coverage.

### Fixed

- VS Code SFC webview TypeScript message/edge typing regressions that previously failed extension compile gates in CI.
- Standalone Web IDE bootstrap now serves fully composed split module assets (`ide-editor-language`, `ide-editor-pane`, `ide-workspace-tree`, `ide-workspace-files`, `ide-observability`, `ide-commands`) so session startup/open-project flows no longer fail with missing runtime symbols (for example `refreshProjectSelection`).
- Standalone Web IDE bootstrap no longer stalls waiting for WASM analysis readiness when the analysis worker repeatedly restarts; IDE startup now completes and remains usable with analysis disabled.
- Web IDE UI-mode detection now accepts both wrapped (`{ok,result}`) and direct (`{ok,mode}`) API payloads, restoring correct standalone-mode behavior (for example disabling runtime-only controls in `standalone-ide` mode).
- Web IDE Hardware tab now rehydrates after IDE session bootstrap events (including deep-link startup on `/ide/hardware`) and performs delayed canvas relayout/fit passes after tab activation so hardware + communication modules from `io.toml`/`runtime.toml` reliably render without clipped/off-screen graphs.
- Web IDE workspace tabs now publish explicit ARIA tab semantics (`tablist`/`tab`/`tabpanel`, `aria-selected`, and active-tab `tabindex` management) for keyboard/screen-reader correctness.
- Config-UI Fleet topology projection no longer reports planned runtimes/endpoints as online by default; `/api/runtime-cloud/state` in config mode now renders offline/degraded runtime state and failed links until live runtime connectivity exists.
- Config-UI now serves read-only `/api/runtime-cloud/config` and `/api/runtime-cloud/rollouts` snapshots in offline mode to prevent Web UI polling 404 noise during TOML/ST-only editing sessions.
- Runtime cloud 5-host topology demo is now self-contained and deterministic: `start-5.sh` auto-bootstraps missing projects, performs clean restarts, waits for runtime-cloud readiness, and ships explicit mesh/link-transport seed data for mixed `realtime`/`zenoh` topology behavior.
- Fleet/Comms add-driver deep-link handling now preserves unknown preselected driver types instead of silently falling back to `modbus-tcp`.
- Fleet/Comms schema-number handling now preserves valid `0` values (for example Modbus `unit_id = 0`) and enforces min/max validation before save.
- Fleet topology device context menu now resolves touch coordinates reliably (long-press/taphold) and always unregisters its outside-click dismissal listener on close.
- VS Code statechart custom editor packaging now loads the webview template from bundled extension code instead of `src/**` runtime paths excluded by `.vscodeignore`.
- VS Code statechart editor lifecycle now stops active execution sessions when the panel closes, ensuring timers/runtime connections are cleaned up.
- State machine engine transition execution now awaits exit/transition/entry hardware actions before completing transitions.
- Hardware-mode guard handling now fails closed on parse/read/runtime errors instead of allowing unsafe transitions.
- Runtime control client request timeout/error handling now removes per-request listeners reliably to prevent listener leaks.
- Statechart traffic-light example now starts from a valid non-dead initial state (`Red`).
- Web IDE hover behavior now follows VS Code semantics by relying on Monaco hover tooltips; the redundant Output-side Hover panel was removed to prevent duplicate/stale hover text.
- Web IDE hover provider failures are now logged as `[ide] hover failed:` in browser console output to improve diagnostics when hover requests fail.
- Web IDE analysis cache refresh now skips unrelated unreadable/oversized `.st` files so hover/completion keep working for the active source, Monaco hover/suggest widgets now use overflow-safe rendering to avoid hidden popups, and `/ide` assets are served with `Cache-Control: no-store` so browser refreshes pick up the latest IDE fixes.
- Web IDE hover/completion hardening now normalizes Monaco hover content payloads, falls back to local symbol suggestions when completion analysis is unavailable, and debounces hover popup triggering on mouse idle so behavior matches VS Code more closely on Chromium-based runtime kiosks.
- Browser analysis worker-recovery CI gate now includes the Node test fixture file (`docs/internal/prototypes/browser_analysis_wasm_spike/web/analysis-client.test.mjs`) in tracked repository contents, fixing missing-file failures on GitHub runners.
- GitHub Pages demo language features are now strict WASM/LSP-only (no local fallback synthesis), completion request flow is hardened via deduplicated document sync and longer WASM request budgets for slower devices, and FB hover signatures now recover declared member type text from source when semantic type IDs are unresolved.
- Browser WASM completion ranking now prioritizes typed-prefix matches before result limiting, and completion context recovery now keeps in-scope program variables available during parser recovery edits (fixing missing `Status`/`Pump` suggestions in the demo).
- Browser IDE/WASM LSP navigation for structured type fields is now project-wide: `Go to Definition`, `Go to References`, and rename now resolve `STRUCT`/`UNION` members across files (fixing `No definition/references found` on demo symbols like `Enable`, `ActualSpeed`, and `ramp`).
- GitHub Pages demo rename wiring now consumes WASM rename edits directly, restoring `F2` rename in Monaco, and walkthrough instructions now match the exact Monaco UI labels/actions (`Ctrl+Left-click`, `Go to References`, `Fn+F2` note for laptops).
- GitHub Pages demo LSP request handling is now resilient on slower browser hosts: WASM timeouts were raised for diagnostics/navigation/rename/completion, Monaco cursor boundary fallback now retries nearby symbol offsets for strict WASM requests, URI mapping accepts normalized location schemes, and diagnostics walkthrough text now reflects a clean-by-default project (use a temporary typo to demonstrate squiggles).
- GitHub Pages demo strict-WASM LSP cursor anchoring is now delimiter-aware (`.`, `#`, `:` boundaries): completion/definition/references/rename retry nearby identifier edges, context-menu requests set the cursor to the clicked symbol, and rename preflight resolves identifier ranges from the same fallback matrix.
- Browser analysis worker-recovery checks now also run demo cursor-position fallback regressions (`docs/demo/lsp-position-resolver.test.mjs`) and no longer fail hard when the legacy prototype test fixture path is absent.
- GitHub Pages demo walkthrough actions are now deterministic and WASM-only for investor flow: steps 1-7 target the correct symbols/files, `Shift+F12` and `F2` are triggered through Monaco actions, cross-file navigation opens in-editor via a registered Monaco editor opener, and bundled Monaco now includes the required references/rename contributions to keep references/rename/go-to-definition working in Pages builds.
- Added local GitHub Pages replica serving for the demo (`scripts/serve_demo_local_replica.py`, `scripts/run_demo_local_replica.sh`) with `/<repo>/` routing and no-cache headers so local behavior matches deployed Pages URLs.
- IDE symbol-target resolution now anchors token/scope lookup to the resolved identifier range (not raw punctuation cursor token), fixing `Go to Definition`, references, and rename misses when the cursor lands on adjacent punctuation in demo/editor workflows.
- GitHub Pages demo release packaging now rebuilds and ships refreshed `docs/demo/wasm/` artifacts from the current `trust-wasm-analysis` engine so deployed Pages behavior matches committed source fixes.
- VS Code extension project workflows now follow `src/`-based projects consistently: ST test run root detection accepts `src`, PLCopen export integration coverage uses `src`, and new-project scaffolding no longer generates legacy `sources/`.
- CI Windows build no longer fails on missing `wpcap.lib` when `ethercat-wire` is enabled by default; EtherCAT wire dependency wiring is now unix-target gated while preserving mock-driver support cross-platform.
- MP-001 parity baseline updated for newly added Mitsubishi LSP regression tests so discovery parity gate remains deterministic.
- Parser diagnostics now report a targeted error (`expected identifier after '#'`) for malformed Siemens SCL `#` local-reference syntax instead of generic expression errors.
- Schneider EcoStruxure vendor detection is now distinct from generic CODESYS-family heuristics in PLCopen migration reports.
- GitHub license detection no longer reports an extra `Unknown` license entry after removing the non-standard root `LICENSE` stub (dual-license files remain `LICENSE-MIT` and `LICENSE-APACHE`).
- Release packaging metadata:
  - VS Code extension package versions are now aligned to the workspace release version to avoid duplicate publish artifacts from prior extension versions.
- Release workflow hardening:
  - VS Code Marketplace publish now runs per-VSIX with retry/backoff on transient network timeouts and treats already-published artifacts as idempotent success for reruns.
- VS Code Marketplace screenshots now use absolute image URLs from GitHub raw content so images render reliably in extension listing pages.
- `trust-runtime validate` no longer reports false `undefined program` errors for valid bundles; it now validates decoded bytecode/module metadata directly and enforces task-program name integrity at bytecode-validation time.
- Bytecode encoding/validation now handles enum-typed constant payloads correctly during module build/validation.
- `%MW` memory marker force/write synchronization in runtime I/O panel flow.
- Debug adapter force latch behavior and state-lock interaction.
- Debug runner now respects configured task interval pacing.
- Windows CI/test path issues (`PathBuf` import and path hygiene guardrails).
- `Harness::run_until` now has a default cycle guard and explicit `run_until_max` limit to prevent hangs.
- Filtered test runs now clearly report when zero tests match but tests were discovered.
- `version-release-guard` now tolerates short ordering races between `main` and tag pushes by polling for the expected version tag before failing.
