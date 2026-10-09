# Control Studio workflows 01–27: foundations, identification, and PID

The [complete workflow specification](../control-studio-complete-workflow-specification.md)
governs shared automatic/manual behavior, evidence and simplicity policy.
Numbers tied to lesson examples are acceptance benchmarks, not global product
limits. Approved teaching requirements take precedence over an incomplete
README or incorrect executable project. This is a product workflow contract;
Studio designs and analyses, while deployed control executes in ST/runtime.
The [Identify](../control-studio-identify-page.md),
[Tune](../control-studio-tune-page.md) and
[Validate](../control-studio-validate-page.md) specifications own the page
layouts. The capability inputs and results below do not prescribe additional
controls, panels or navigation.

Status: required product workflow specification. This document specifies future
behavior; it does not claim that any Control Studio view, runtime operation, ST
example, simulator, or physical plant already provides it.

## Scope and common contract

The shared three-page flow orchestrates acquisition/model qualification,
controller recommendation, tuning, actual-ST validation and explicit Apply.
It reuses the same calculation and runtime operations for automatic and manual
method choices. A supplied model can support prediction without fabricating
identification recordings. No operation weakens requirements or applies
settings without the user's action.

Every result retains loop/binding and controller revisions, source
(`Simulation`, `Physical plant`, `Recorded data`, or `Offline model`), units,
range, sample-time sequence, model provenance and validation state, immutable
conditions, method, raw/usable recording identities, candidate and requirement
results. The selected immutable study records form the hand-off between stages: model
selection, tuning, verification and apply consume its typed revision directly.
The UI MUST NOT require handwritten JSON or copied coefficients. Simulation,
mathematical prediction, executed ST/simulator recording, and physical
observation remain visibly distinct evidence classes.

Before a write, Test checks fresh compatible values, declared bounds, required
Manual/Auto mode, force state, authority, timeout, stop and restoration. One
approved bounded physical campaign may cover its declared internal trials and
analysis; approval is renewed only for a material change or a physical action outside that campaign. Apply still needs separate approval. Abort, timeout, bad/stale values or
failed restoration stop at the operation, preserve prior valid records and
state the recovery action. Manual analysis remains available when the automatic
route is blocked.

**Selection and qualification policy.** Identify evaluates supported structures
on immutable estimation data and scores them on a distinct immutable held-out
record with a different amplitude or sequence. It recommends the simplest
acceptable structure, never the highest same-run fit. Tune evaluates only
applicable controller structures and selects the simplest passing candidate for
the declared process, actuator and task (for example, PD can be simpler than PI
for a motion-damping task). Extra complexity is selected only when it is needed to satisfy a mandatory
requirement; improved performance beyond requirements does not displace a
simpler qualifying design.
All mandatory requirements must be `Pass`; missing evidence is
`Inconclusive`, never pass. A relay record is eligible only after its actual
feedback switching cycle, extrema, period, quality and restoration are
qualified; a clock-driven square wave is not relay evidence.

**Verify and Apply.** Verification uses one immutable conditions record for
current and candidate: initial state, events, sample sequence/duration,
disturbance/noise seed, limits/rates and model variation. It reports values,
limits, units, source and `Pass`/`Fail`/`Inconclusive`; it cannot change the
running controller. Apply consumes the exact reviewed current/candidate
parameter record and validation evidence, validates it and writes atomically at a
scan boundary, confirms a later read-back, then monitors for the
task/process-declared bounded observation duration with the exact prior
confirmed revision available for rollback. Project-owned controllers need a
typed design adapter; no generic PID parameter write is permitted. Studio
computations are design, replay and analysis only: the actual control law runs
independently in the configured ST/runtime task.

<a id="wf-001"></a>

## WF-001 — Control-loop elements

Source: [Lesson 1 README](../../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/README.md).

- **Trigger, inputs, prerequisites:** engineer opens an unbound or observation-only loop and supplies declared SP/PV/MV or open-loop input/output bindings, units, ranges, source and process direction; live values must be fresh and type-compatible.
- **Automatic operation:** `Discover loop` validates non-overlapping typed bindings and records the direction from a bounded observation or supplied model; it outputs a loop map, source label and “feedback not configured” where no controller exists. It calculates only displayed error `e=SP-PV`; this is analysis, not an ST command.
- **Manual controls:** Configure control loop, choose compiled bindings, select signals, start/stop observation recording, annotate disturbance/event, and Supply model are direct controls.
- **Recommendation/escalation:** recommend observation-only when action direction is unknown; recommend ON/OFF only after the engineer declares a binary actuator and acceptable switching constraints. Reject ambiguous bindings or unit mismatch; escalate to a bounded MV test/physical review rather than infer symbols.
- **Verification:** simulator/ST case: commanded refrigeration ON then OFF and door disturbance show PV falling then rising while command follows the fixed sequence. Accept a complete synchronized record with declared command/PV direction; no movement is inconclusive when the interval/amplitude/process lag cannot reveal it, and requires a larger safe test or model. Fail reversed direction, stale/gapped data, or any claim that the fixed sequence is feedback.

<a id="wf-002"></a>

## WF-002 — ON/OFF control

Source: [Lesson 2 README](../../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/README.md).

- **Trigger, inputs, prerequisites:** binary actuator, declared direct/reverse action, SP/PV units and threshold equality policy are available; engineer asks for binary feedback or chooses the project controller.
- **Automatic operation:** evaluate `u=ON` when the declared comparison (the lesson example is `PV >= SP`) is true, otherwise OFF; preview expected crossings and command changes from a record, then output an ON/OFF candidate marked project-owned, not PID.
- **Manual controls:** select comparison polarity/equality, edit threshold, run Observe without excitation, inspect command-change events, and select or reject the candidate.
- **Recommendation/escalation:** recommend hysteresis/dwell when repeated threshold crossings exceed declared switching allowance; block generic PID Apply because project-owned ON/OFF has no PID adapter. Unknown compressor limits require engineer input, not a guessed dwell.
- **Verification:** actual ST/simulation run from 4 °C with 2 °C target and door event must switch ON above/equal threshold and OFF below it. Accept only if every recorded transition obeys the predicate; fail if a fixed sequence is presented as feedback or rapid threshold chatter violates the declared switch limit.

<a id="wf-003"></a>

## WF-003 — Hysteresis and dwell

Source: [Lesson 3 README](../../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/README.md).

- **Trigger, inputs, prerequisites:** ON/OFF loop has upper/lower thresholds, prior applied state, update time, normal minimum ON/OFF dwell and an explicit abort rule.
- **Automatic operation:** compute requested state from upper/lower thresholds and retain it in-band; separately compute eligibility `elapsed >= dwell` before changing applied state. Output requested/applied traces, blocked transitions and interval durations; it never treats an abort as a normal dwell completion. It proposes thresholds/dwell only from declared actuator ratings, process bounds and sensor uncertainty.
- **Manual controls:** edit thresholds/dwell, view requested versus applied signals, review each transition, abort, and choose whether a supplied equipment constraint replaces the lesson’s 8 s example.
- **Recommendation/escalation:** recommend this structure when WF-002 chatters; require equipment limits and measurement uncertainty for physical values. Escalate when safety shutdown policy conflicts with dwell, preserving immediate forced-OFF semantics.
- **Verification:** ST/simulation case with 2.5/1.5 °C thresholds and 8 s dwell accepts only normal intervals >=8.0 s, binary applied command, retained request in-band, and immediate abort OFF recorded separately. Fail 7.9 s normal reversal, state toggling inside the band, or delayed abort.

<a id="wf-004"></a>

## WF-004 — Cold-room commissioning

Source: [Lesson 4 README](../../../examples/control/14-integrated-capstones/80-cold-storage/README.md).

- **Trigger, inputs, prerequisites:** an identified project-owned cold-room controller, declared initial/reset state, door scenario, temperature band, normal dwell and abort expectations exist.
- **Automatic operation:** build an immutable commissioning plan, execute/compare bounded simulation records and calculate interval duration `t_next transition-t_entry`; output each requirement separately, never an averaged “pass.”
- **Manual controls:** set scenario/reset, record runs, inspect temperature/door/command events, mark abort interval, select Verify and export the retained result.
- **Recommendation/escalation:** recommend commissioning only after WF-003 passes component behavior; if temperature band and dwell conflict, return `No acceptable controller found` and ask for process/equipment requirements rather than changing physics.
- **Verification:** supplied simulation example accepts 1–3 °C from 40 s, normal ON/OFF intervals >=8 s and binary command under both stated door cases. Fail any separate limit, incomplete reset equivalence, or an assertion that simulator commissioning approves a physical cold room.

<a id="wf-005"></a>

## WF-005 — Proportional control

Source: [Lesson 5 README](../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/README.md).

- **Trigger, inputs, prerequisites:** continuous actuator, signed action, `Kp`, limits and SP/PV are declared; a controller is either a typed PID binding or project adapter.
- **Automatic operation:** calculate `e=SP-PV`, `u_r=Kp e` and applied clamp; predict/measure residual steady error under a held load. It outputs a P candidate and its required holding-error warning, not an inference that the plant has integral action.
- **Manual controls:** edit `Kp`, choose direct/reverse action, preview response/effort, import a candidate, and run a bounded setpoint record.
- **Recommendation/escalation:** recommend P only when requirements allow steady error or an accepted integrating model supplies plant integral action; otherwise recommend PI and require model/experiment evidence before tuning.
- **Verification:** actual heater ST/simulation setpoint run must show larger error gives larger request and a nonzero holding error when heat loss requires power. Accept correct direction/limits and measured residual; fail reversed action, unclamped output, or calling calculated equilibrium an observed trace.

<a id="wf-006"></a>

## WF-006 — PI control

Source: [Lesson 6 README](../../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi/README.md).

- **Trigger, inputs, prerequisites:** P behavior shows unacceptable steady error; `Ki`, elapsed time, retained state and output limits are declared.
- **Automatic operation:** use `I_new=I_old+Ki*e*dt`, then `u_r=Kp*e+I_new`; show integral contribution and time basis. It generates PI only from an applicable selected method/model or engineer-entered values, and labels retained I as controller state.
- **Manual controls:** edit `Kp/Ki`, pause or reset state only through explicit lifecycle controls, inspect I/P/request/applied traces, and run the matched P-versus-PI trial.
- **Recommendation/escalation:** recommend PI when verified P residual violates requirement; escalate to anti-windup configuration whenever saturation is possible, rather than silently limiting I.
- **Verification:** matched ST/simulation heater trials accept PI only if it removes the defined steady error while `dt`-scaled increments match timestamps and output stays bounded. Fail use of scan count as time, lost retained state, or a PI run that reaches target only by changing the scenario.

<a id="wf-007"></a>

## WF-007 — Saturation and windup

Source: [Lesson 7 README](../../../examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-saturation-windup/README.md).

- **Trigger, inputs, prerequisites:** PI loop declares output bounds and a reachable/unreachable target or load scenario; requested and applied output are separately bound.
- **Automatic operation:** calculate `u_a=min(max(u_r,u_min),u_max)`, detect mismatch `u_a-u_r`, accumulated I during saturation and recovery metrics; use declared model/limits to predict windup risk and propose anti-windup before a physical saturation demonstration. It outputs a finding/candidate, not an automatic reset.
- **Manual controls:** edit limits, choose saturation-return test, inspect requested/applied/I, set an unreachable target only in a bounded approved simulation/physical plan, and abort.
- **Recommendation/escalation:** recommend anti-windup from declared limits/model risk or sustained recorded mismatch; fail feasibility before controller ranking when target exceeds declared capacity.
- **Verification:** heater simulation beginning at 35 °C/75%, requesting the 45 °C unreachable target then returning to 35 °C, accepts correct 0–100% clamp and recorded delayed recovery. Fail if `u_r` is overwritten with `u_a`, capacity is claimed to increase, or windup is hidden.

<a id="wf-008"></a>

## WF-008 — Anti-windup

Source: [Lesson 8 README](../../../examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-anti-windup/README.md).

- **Trigger, inputs, prerequisites:** declared actuator limits/model predict possible saturation or WF-007 records it; tracking time `Tt>0`, requested/applied outputs, `dt`, and a retained integrator are available.
- **Automatic operation:** compute trial `I*`, request, clamp, then `I_next=I*+(dt/Tt)(u_a-u_r)`; compare identical saturation-return records with tracking enabled/disabled and output integral, saturation time and recovery difference.
- **Manual controls:** enter `Tt`, enable/disable back-calculation per candidate, inspect each term and select a comparison result; engineers may retain another typed anti-windup adapter with its method named.
- **Recommendation/escalation:** recommend tracking when model/replay predicts or qualified evidence shows materially better recovery without oscillation; reject nonpositive/nonfinite `Tt` and escalate to actuator model/limits if applied output cannot be trusted.
- **Verification:** the same actual ST/simulation scenario accepts tracking when mismatch correction has correct sign at each limit and recovery meets declared conditions. Fail if it changes heater capacity, corrects when `u_a=u_r`, or compares unmatched resets.

<a id="wf-009"></a>

## WF-009 — Setpoint and disturbance response

Source: [Lesson 9 README](../../../examples/control/01-feedback-foundations-and-temperature-control/09-setpoint-and-disturbance-response/README.md).

- **Trigger, inputs, prerequisites:** controller candidate, separately declared SP and disturbance bindings/scenarios, limits and measurement windows are present.
- **Automatic operation:** make two immutable trials: target event changes error directly; load event changes plant response. Calculate peak, settling/error window, minimum PV and MV limits separately and retain the source/event identity.
- **Manual controls:** choose setpoint/load step, amplitudes/times within declared bounds, specify requirement windows, record/compare current/candidate, and inspect response events.
- **Recommendation/escalation:** no candidate qualifies unless both required trials pass; recommend setpoint weighting only for excess target kick and filtering only for measured noise, not as substitutes for disturbance rejection.
- **Verification:** actual heater cases use 35→38 °C and separate 30 kW load at 35 °C; accept peak <=39 °C, `|error|<=0.2 °C` in 30–60 s and disturbance minimum >=33 °C with same final window, plus 0–100% applied output. Fail averaging one good trial against one failed trial.

<a id="wf-010"></a>

## WF-010 — Loop diagnosis

Source: [Lesson 10 README](../../../examples/control/09-industrial-control-structures/60-loop-diagnosis/README.md).

- **Trigger, inputs, prerequisites:** synchronized SP/PV/MV, mode, requested/applied output, quality and event history exist for a bounded diagnosis interval.
- **Automatic operation:** classify evidence patterns—wrong direction, saturation, oscillation, no response, stale/bad measurement, manual ownership or actuator mismatch—against declared rules; output observations and confidence-limited next tests, never a fabricated root cause.
- **Manual controls:** choose interval/signals, mark an event, inspect raw values and quality, select a proposed test/model path, and annotate known maintenance context.
- **Recommendation/escalation:** recommend the narrowest safe next action (binding review, direction test, saturation check, held-input record or maintenance review). Physical fault/safety diagnosis escalates to operator procedure; no automated excitation on uncertain/forced values.
- **Verification:** ST/simulation fault-injection records must retain the actual quality/mode and distinguish command saturation from failed movement and stale PV from no process response. Fail a diagnosis that uses only chart appearance or offers PID tuning before data validity/direction is established.

<a id="wf-011"></a>

## WF-011 — Manual PI tuning

Source: [Lesson 11 README](../../../examples/control/06-pid-tuning-methods/28-manual-pi-tuning/README.md).

- **Trigger, inputs, prerequisites:** valid PI lifecycle/limits, immutable setpoint and disturbance acceptance plan, known source and reset are available.
- **Automatic operation:** Automatic Tune may create a bounded constrained comparison around a qualified model candidate, but it must identify such candidates as `Constrained comparison`; it does not label manual edits as a formula method.
- **Manual controls:** engineer edits `Kp/Ki`, saves named unapplied candidates, runs identical reset trials, compares peak/settling/activity and chooses a candidate explicitly.
- **Recommendation/escalation:** recommend the least aggressive candidate satisfying all mandatory cases; if no candidate passes return `No acceptable controller found`, preserve evidence and direct the engineer to Identify or requirements review.
- **Verification:** executed ST/simulation parameter sweep uses identical 35→38 °C and load records, reports each requirement and command activity `sum |u_k-u_(k-1)|`. Fail comparison across changed reset/load/limits or a manual setting silently applied.

<a id="wf-012"></a>

## WF-012 — Process dynamics

Source: [Lesson 12 README](../../../examples/control/01-feedback-foundations-and-temperature-control/13-process-response-classes/README.md).

- **Trigger, inputs, prerequisites:** engineer requests Find model; Manual ownership, steady baseline, applied MV/PV bindings, bounds and a recording path exist.
- **Automatic operation:** qualify/reuse a held-input record and estimate displayed gain `K=delta PV_steady/delta u`; detect delay and response class without deriving plant dynamics from closed-loop settling. Output suitability plus direct link to the current record.
- **Manual controls:** select MV step/observation, amplitude/duration, baseline and bounds; Review test, Confirm and start, Stop recording, Save result, then inspect applied input/PV.
- **Recommendation/escalation:** recommend WF-014 campaign design when no qualified held-input record exists. A naïve open-loop fit to a closed-loop trace is rejected, but feedback-aware identification may use closed-loop data when the controller/excitation, applied input, reference/disturbance, timing and validity assumptions are retained and its method qualifies them. Escalate insufficient movement/noise to a new bounded campaign.
- **Verification:** lesson simulator/ST project’s 35 °C/75% start, 85% applied step for 30 s accepts a complete applied-input/PV trace and approximately 2 °C/10-point gain observation. Fail computed equilibrium offered as measurement, PI active during the test, or stale/incomplete recording.

<a id="wf-013"></a>

## WF-013 — Energy balance

Source: [Lesson 13 README](../../../examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/README.md).

- **Trigger, inputs, prerequisites:** engineer selects Supply model or compares a physical model to a current record; state, input, ambient/loss/disturbance units and delay are declared.
- **Automatic operation:** evaluate the supplied differential model `C dT/dt=a u(t-L)-H(T-Ta)-D`, derive `K=a/H`, `tau=C/H`, and produce predicted traces labelled model analysis with parameter units/provenance.
- **Manual controls:** enter/import typed coefficients, units, operating range and delay; edit a model revision, select prediction/measurement overlay and accept/reject it for Identify.
- **Recommendation/escalation:** recommend supplied physics only when units, signs and operating range validate; a mismatch recommends alternate structure or identification, never coefficient guessing. It cannot certify physical parameters from a derivation alone.
- **Verification:** compare prediction for the specified 75→85% step (`K=.2`, `tau=5 s`, `L=1 s` example) with a separate actual record. Accept unit-consistent calculation and clearly labelled prediction; fail numerical model treated as executed ST or physical qualification.

<a id="wf-014"></a>

## WF-014 — Identification experiments

Source: [Lesson 14 README](../../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment/README.md).

- **Trigger, inputs, prerequisites:** Find model has no reusable qualified record; declared input/output, Manual mode, authority, bounds, uncertainty, baseline interval and restoration are available.
- **Automatic operation:** propose a bounded campaign with amplitude/duration that gives observable response within limits; calculate baseline slope `(last-first)/(t_last-t_first)`, predicted span and timeout. It checks the engineer-reviewed plan but does not silently enlarge/repeat a physical test.
- **Manual controls:** choose step, pulse, PRBS, multisine or relay where eligible; edit amplitude/duration/seed/limits, approve physical test, abort, and save raw record.
- **Recommendation/escalation:** recommend a step for low-order monotone dynamics, PRBS/multisine for frequency content, and relay only under WF-020 conditions. Reject drift above declared threshold, inadequate span or unknown interlock; request a new operating point/measurement instead of fitting.
- **Verification:** heater example accepts baseline slope <=0.01 °C/s, 10-point step expected to exceed ±0.05 °C uncertainty, 30 s bounded hold and confirmed restoration. Fail Auto-mode MV forcing, no actual applied MV record, out-of-bound PV/MV, or reuse of an unqualified run.

<a id="wf-015"></a>

## WF-015 — Step-response identification

Source: [Lesson 15 README](../../../examples/control/05-experiments-identification-and-validation/23-step-response-identification/README.md).

- **Trigger, inputs, prerequisites:** complete qualified held-step estimation record has declared initial/final windows, input change, sample times, quality and no disqualifying saturation/gaps.
- **Automatic operation:** fit FOPDT `PV(t)=PV0+K*delta u*(1-exp(-(t-L)/tau))` after delay; estimate K, tau and L from full record, show residuals/units/range and save the fit linked to raw data.
- **Manual controls:** choose/exclude intervals without deleting raw samples, set windows/initial guesses, select FOPDT or another declared structure, inspect log-linear diagnostic and accept/reject a fitted revision.
- **Recommendation/escalation:** recommend simplest fit only as a same-run preview until WF-018 held-out qualification; escalate first-rise ambiguity, clipping/noise or persistent residual shape to WF-016/WF-017 rather than treating detection time as true delay.
- **Verification:** a 35→37 °C response to 10 points should recover approximately `K=.2 °C/point`, `tau=5 s`, `L=1 s` for the lesson model. Fail a fit without raw revision/window identity, a nonfinite/nonpositive time constant, or a candidate sent to Tune before held-out validation.

<a id="wf-016"></a>

## WF-016 — Model structure

Source: [Lesson 16 README](../../../examples/control/05-experiments-identification-and-validation/24-choose-model-structure/README.md).

- **Trigger, inputs, prerequisites:** an accepted fit has systematic residual mismatch or engineer supplies alternate physical states/sensors; same estimation data and a distinct validation record are retained.
- **Automatic operation:** compare supported static, integrating, first/second-order and dead-time structures using residual shape and held-out score; for a sensor-lag candidate evaluate `Cs dTs/dt=Gs(T-Ts)` and retain all structural assumptions.
- **Manual controls:** select structures, add declared sensor/reference channel, edit typed parameters, inspect Fit/Residuals/Poles and zeros and accept a model revision.
- **Recommendation/escalation:** choose the simplest structure whose held-out requirements pass; recommend extra sensor/reference calibration when lag attribution is not observable. Reject “more parameters improves fit” as a selection rule.
- **Verification:** same heater record comparison must show the instant-sensor and 2 s sensor-lag predictions separately and require independent water/probe evidence before attribution. Fail a model change that silently refits validation data or labels a structural assumption as measured fact.

<a id="wf-017"></a>

## WF-017 — Identifiability

Source: [Lesson 17 README](../../../examples/control/05-experiments-identification-and-validation/25-parameter-identifiability/README.md).

- **Trigger, inputs, prerequisites:** two or more parameter sets/structures give materially similar observed response, with measurement uncertainty and experiment horizon retained.
- **Automatic operation:** calculate competing predictions and parameter sensitivity/interval warning; flag practical ambiguity when differences fall within uncertainty and structural ambiguity when input-output equivalence remains. It outputs a disambiguation requirement, not false precision.
- **Manual controls:** inspect candidate overlays, uncertainty, sensitivity and residuals; choose longer record, new excitation/amplitude or independent measurement, then launch that bounded campaign.
- **Recommendation/escalation:** recommend the cheapest discriminating safe experiment; block Tune when parameters needed by the selected method are unresolved. More samples of the same indistinguishable signal are not recommended as structural resolution.
- **Verification:** compare `(K,tau)=(.2,5)` and `(.4,10)` early responses with ±.05 °C uncertainty and sensor/process-lag swaps. Accept explicit ambiguity and new-evidence proposal; fail a unique-model claim based only on equivalent fit.

<a id="wf-018"></a>

## WF-018 — Model validation

Source: [Lesson 18 README](../../../examples/control/05-experiments-identification-and-validation/26-independent-model-validation/README.md).

- **Trigger, inputs, prerequisites:** immutable accepted estimation model and a distinct immutable record with a different excitation amplitude or sequence exist.
- **Automatic operation:** freeze coefficients, calculate residual `r_i=measured_i-predicted_i`, RMSE and timing/final-value checks on held-out data; retain both residual vectors and record identities, then mark model validated/unvalidated.
- **Manual controls:** choose held-out record, requirements/windows, run/import a new validation test, inspect overlay/residuals, accept/reject model and export result.
- **Recommendation/escalation:** accept for tuning only if all model qualification limits pass; failed model directs engineer to structure, identifiability or experiment workflow. Refit is a new identification revision, never a repair to failed held-out validation.
- **Verification:** lesson case accepts residuals within ±.2 °C and final-window RMSE <=.1 °C on unused data. Fail use of the fitting run as validation, missing sample identity, unitless RMSE, or silent coefficient change.

<a id="wf-019"></a>

## WF-019 — Model-based PI tuning

Source: [Lesson 19 README](../../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning/README.md).

- **Trigger, inputs, prerequisites:** WF-018 accepted FOPDT/integrating model has finite applicable parameters, declared controller polarity, units, limits, sample time and immutable candidate-validation scenarios.
- **Automatic operation:** offer two explicitly distinct methods. **Lesson coefficient matching** for its ideal delay-free local model uses `Kp=tau/(K*lambda)` and `Ki=1/(K*lambda)`; the supplied `K=.2`, `tau=5 s`, `lambda=5 s` case is therefore `Kp=5`, `Ki=1`. **SIMC FOPDT** is separately applicable when delay/structure satisfy its domain and uses `Kp=T/(K*(lambda+L))`, `Ti=min(T,4*(lambda+L))`, `Ki=Kp/Ti`. The method name, equations, delay treatment, polarity and unapplied candidate are retained; response choices change lambda only where that method defines them.
- **Manual controls:** select formula/response lambda, enter parallel PID values, import a typed candidate, inspect predicted response/Bode/margins and choose candidate for Verify.
- **Recommendation/escalation:** recommend the simplest task-appropriate candidate from the common P/PI/PD/PID comparison, not formula availability alone. Reject zero/nonfinite gain, invalid delay/time constant, unvalidated model or polarity mismatch; negative plant gain is valid when the declared controller action/sign convention is compatible. Route unsupported structure to manual/project adapter.
- **Verification:** lesson coefficient-matching calculation with `K=.2`, `tau=5 s`, `lambda=5 s` yields `Kp=5`, `Ki=1` points/(°C·s); SIMC results are displayed as a separate delayed-model method and never substituted for that example. Each must pass WF-009 matched executed/simulation verification. Fail a calculation applied without review or use of a naïve closed-loop fit as plant T.

<a id="wf-020"></a>

## WF-020 — Relay tuning

Source: [Lesson 20 README](../../../examples/control/06-pid-tuning-methods/29-relay-tuning/README.md).

- **Trigger, inputs, prerequisites:** Manual mode and write authority, fresh PV/MV, declared range/limits/restoration, bounded duration, relay amplitude and noise-protected PV hysteresis are confirmed.
- **Automatic operation:** runtime performs feedback relay: write high, hold until fresh PV reaches upper threshold, write low until PV reaches lower threshold; retain every applied transition/crossing/extremum/quality. After qualified cycles calculate `Ku≈4d/(pi*a)`, period `Pu`, then selected empirical PI rule (`Kp=.45Ku`, `Ti=Pu/1.2`, `Ki=Kp/Ti`) as an unapplied candidate.
- **Cycle repeatability:** qualify at least three complete measured cycles only when the peak-to-peak amplitude spread and period spread are each at most 10% of their respective means (inclusive, with floating-point tolerance). A record outside either bound remains unqualified; report which quantity varies and request a longer settled record or another permitted identification method. Do not average nonrepeatable cycles into an accepted candidate. This tolerance establishes repeatability only; it does not establish sinusoidal shape or full-model identifiability.
- **Manual controls:** select Feedback relay on Identify, set amplitude/duration and permitted larger hysteresis, review high/low levels and restoration, confirm/abort and inspect the cycles; then choose the **Relay rule** method on Tune, which calculates the unapplied candidate from the qualified cycles (numerical annex N5.R).
- **Recommendation/escalation:** recommend relay only if actual sustained near-sinusoidal feedback cycles, enough qualified periods and small hysteresis relative to response occur; otherwise return a specific qualification failure and suggest step/PRBS. It never substitutes a time-driven square wave.
- **Verification:** actual simulator/ST relay run accepts only fresh feedback-driven crossings, in-range writes, qualified amplitude/period, restored MV and a candidate calculated only by Tune's Relay rule. Fail precomputed toggling, stale/forced PV, fewer valid cycles, clipped output, failed restoration or automatic Apply.

<a id="wf-021"></a>

## WF-021 — Tuning comparison

Source: [Lesson 21 README](../../../examples/control/06-pid-tuning-methods/31-tuning-method-comparison/README.md).

- **Trigger, inputs, prerequisites:** manual, model-based and/or relay candidates retain their method/model revisions; one immutable setpoint/disturbance/noise/limit plan exists.
- **Automatic operation:** execute or predict each candidate under identical conditions, reject any mandatory failure before ranking, then partition qualifying candidates by task-appropriate structural complexity and rank performance only within the simplest qualifying partition using declared worst-settling, activity `A=sum |u_k-u_(k-1)|`, robustness and constraint criteria; retain raw results.
- **Manual controls:** pin/select candidates, choose response objective and declared requirements before results, inspect current/candidate overlays and rejection reasons, and select an unapplied winner.
- **Recommendation/escalation:** choose the simplest qualifying candidate first, then the best declared performance among candidates of that same complexity; never average failed temperature limit away or use an arbitrary default score band. If none passes, output `No acceptable controller found` and retain all evidence.
- **Verification:** separate reset runs for each candidate must demonstrate both setpoint and disturbance requirements, command activity and exact conditions identity. Fail differing event sequences, hidden rejected candidates, or ranking an inconclusive candidate.

<a id="wf-022"></a>

## WF-022 — Setpoint weighting

Source: [Lesson 23 README](../../../examples/control/01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting/README.md).

- **Trigger, inputs, prerequisites:** PI candidate passes/assesses separate target and load trials, has a declared reference/weight `0<=beta<=1` and uses full error for integral action.
- **Automatic operation:** calculate weighted P `Kp[beta(SP-T0)-(PV-T0)]`; compare candidate beta values on matched target and disturbance plans while preserving full-error integral path. Output target-kick/activity versus response tradeoff.
- **Manual controls:** edit beta, choose raw/filtered PV source per current parameter revision, inspect P/I/request, and compare beta=1 with an engineer-selected value.
- **Recommendation/escalation:** recommend lower beta only when target command jump is a stated problem and disturbance performance remains acceptable; do not lower Kp globally as a surrogate. Escalate noise-driven movement to WF-023.
- **Verification:** 3 °C target example with `Kp=2` accepts P jump 6 points at beta=1 and 3 at beta=.5 while identical measured-PV change retains coefficient. Fail integral using weighted error or a disturbance-only conclusion.

<a id="wf-023"></a>

## WF-023 — Measurement filtering

Source: [Lesson 24 README](../../../examples/control/01-feedback-foundations-and-temperature-control/11-measurement-filtering/README.md).

- **Trigger, inputs, prerequisites:** raw PV quality/sample timestamps, acquisition chain, existing controller filter/estimator placement and a noise/real-disturbance comparison plan exist.
- **Automatic operation:** inspect acquisition rate, quantization, gaps, existing filtering/estimation and spectra/replay evidence to determine whether filtering is needed, which signal/type is affected and where it belongs. Evaluate bypass, supported first-/second-order low-pass candidates (the first-order update is `alpha=dt/(Tf+dt)`, `yf_new=yf_old+alpha(yr-yf_old)`), and a typed notch only when a declared narrow periodic disturbance and adequate sampling justify it; estimators remain scoped to their declared state/model and are never presented as generic noise filters. Replay candidates first, then co-design with closed-loop response/effort before recommendation.
- **Manual controls:** select raw/PV/derivative path and placement, edit Tf/bypass or typed notch/estimator parameters, initialize state from fresh PV, inspect raw/filtered plots, replay a record and run matched closed-loop noise and real-disturbance trials.
- **Recommendation/escalation:** recommend no filter, the smallest adequate low pass, or a justified notch/estimator only when the declared evidence and closed-loop tradeoff support it; report aliasing/data-age issues to WF-024 rather than hiding them with a filter.
- **Verification:** `Tf=1 s`, `dt=.1 s`, raw 35→36 °C first output 35.0909 °C is arithmetic acceptance; actual ST/simulation accepts only if same noise and disturbance records show stated activity/recovery. Fail negative Tf, state reset each scan, or a filter portrayed as recovering unmeasured information.

<a id="wf-024"></a>

## WF-024 — Sampling and timing

Source: [Lesson 25 README](../../../examples/control/01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling/README.md).

- **Trigger, inputs, prerequisites:** acquisition time, controller update time, applied-command time, freshness/gap rules and retained timestamp baseline are bound.
- **Automatic operation:** calculate all state updates from accepted elapsed `dt`, detect missing/incompatible samples, gap/jitter and Nyquist/aliasing risk; output timing evidence and block Identify/Test when completeness requirements fail.
- **Manual controls:** inspect timestamp lanes, configure declared sample/update period where the adapter permits, choose delayed-update test and review rejected samples; reset timing baseline explicitly.
- **Recommendation/escalation:** recommend a faster/compatible acquisition campaign or a lower-bandwidth design when sampling cannot resolve dynamics. Do not use an assumed task period for actual dt or claim a post-filter fixes aliasing.
- **Verification:** constant 1 °C error with `Ki=.5` accepts increments .05 at .1 s and .10 at .2 s from actual timestamps; sampled sine alias case must visibly show ambiguity. Fail first-update integration without baseline, stale sample reuse, or successful control claim from UI snapshots alone.

<a id="wf-025"></a>

## WF-025 — Controller initialization

Source: [Lesson 26 README](../../../examples/control/03-production-quality-digital-pid/39-controller-mode-initialization/README.md).

- **Trigger, inputs, prerequisites:** Manual owns output; fresh PV, applied MV, weighted P, filter state, valid timing and one persistent PID instance are available.
- **Automatic operation:** before Auto authority, initialize filtered PV, timestamp baseline and `I_initial=u_applied-P`, expose Ready, and ensure a one-shot gate. It outputs no automatic command until initialization succeeds.
- **Manual controls:** request initialize, inspect mode/readiness/P/I/request/applied/timestamp/owner, select Auto only after Ready, reset lifecycle state, and abort/restore.
- **Recommendation/escalation:** recommend initialization whenever controller starts/restarts or model/candidate changes ownership; block Auto on invalid PV/MV/timestamp and direct to data-quality recovery rather than use a 75% fixed bias.
- **Verification:** actual ST/simulation hot state PV=SP=35 °C, applied 75%, filtered 35 °C accepts `P=0`, `I=75`, Manual ownership during initialization and first Auto request continuity. Fail duplicate initialization accumulation, two output writers, unready Auto, or treating initialization as physical sensor proof.

<a id="wf-026"></a>

## WF-026 — Bumpless transfer

Source: [Lesson 27 README](../../../examples/control/03-production-quality-digital-pid/40-bumpless-transfer/README.md).

- **Trigger, inputs, prerequisites:** Ready controller, Manual applied output, fresh weighted P/filtered PV and an explicit rising Auto request with a single output selector exist.
- **Automatic operation:** while Manual, track `I=u_a-P`; on accepted transfer snapshot fresh P, set I again, reconstruct first request equal to `u_a`, establish timing baseline and skip ordinary integration for exactly that transfer scan.
- **Manual controls:** change manual MV, request Auto, view transfer flag/owner/P/I/request/applied, run an isolated untracked counterexample, abort and reset.
- **Recommendation/escalation:** recommend transfer tracking whenever ownership changes; block transfer on invalid/stale inputs or ambiguous owner. Preserve physical delayed delivered heat separately from output-boundary continuity.
- **Verification:** actual 75→60% manual hold then Auto run accepts boundary request difference <=.1 point, single ownership change, skipped transfer-scan integration and normal next-scan PI. Fail 15/19-point stale-state jump, manual interval integrated as Auto, or claim that continuous command fixes PV dynamics.

<a id="wf-027"></a>

## WF-027 — Parameter updates

Source: [Lesson 28 README](../../../examples/control/03-production-quality-digital-pid/41-atomic-parameter-update/README.md).

- **Trigger, inputs, prerequisites:** complete typed candidate record contains finite Kp/Ki/beta/filter/limits/gap/revision; active record, snapshot SP/raw/filtered PV and retained I are current.
- **Automatic operation:** validate whole record, calculate same-snapshot `P_old` and `P_new`, set `I_new=I_old+P_old-P_new`, then publish complete record/revision at one task boundary and perform exactly one ordinary update for the control interval. Invalid candidate changes nothing.
- **Manual controls:** edit a staged record, validate, review old/new P and predicted limit effect, submit/publish, inspect active revision, compare/rollback candidate and run follow-up Verify.
- **Recommendation/escalation:** recommend atomic revision only after validation; reject partial fields, `min>max`, out-of-range beta, negative filter or stale expected revision. Tightened limits may deliberately change applied output and require tracking; do not promise continuity beyond the available limit.
- **Verification:** nonzero-error example (`Pold=4`, `Pnew=6`, `Iold=71`) accepts `Inew=69` and unchanged 75% unlimited request; invalid 80%/20% limits must leave active revision and candidate-independent I unchanged. Follow-up actual ST/simulation target and load trials must still pass WF-009. Fail partial publication, double/suppressed ordinary integration, or reporting queued state as applied.

## Source-read manifest

The 27 mapped source READMEs above were read in full from the course mapping in
[control-course-teaching-specification.md](../control-course-teaching-specification.md), together with
`control-course-content/01-11-foundations-and-pi.md`,
`12-21-models-and-tuning.md`, and `22-32-implementation-and-actuators.md`.
SHA-256 source inventory is retained in the work handoff; hashes are not product
identity and MUST NOT be used to infer workflow validity.
