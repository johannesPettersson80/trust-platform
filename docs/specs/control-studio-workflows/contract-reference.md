# Control Studio — complete engineering workflow specification

Status: normative technical capability requirements. Implementation and
rendered acceptance remain separate. This contract preserves the engineering
methods derived from the 82-lesson map; it does not define a second UI.

## 1. User workflow authority

The [Identify](../control-studio-identify-page.md),
[Tune](../control-studio-tune-page.md) and
[Validate](../control-studio-validate-page.md) specifications exclusively own
page contents. The [shared automatic flow](../control-studio-complete-workflow-specification.md#three-page-automatic-flow)
owns progression, compact editors, autosave and recovery. Controller selection
belongs on Tune; Apply and restoration belong on Validate. Capabilities below
are calculations and runtime operations, not additional tabs, forms or reports.

Use the [shared truST UI/UX contract](../35-vscode-control-studio.md#shared-trust-uiux-contract--mandatory)
for clarity, consistency, accessibility and real rendered acceptance.

## 2. Engineer inputs and loop setup

Studio MUST discover compiled controller instances, actual types, owning tasks,
symbolic/addressed variables and available runtime values. Guided setup confirms:

- Controlled quantities, setpoints where applicable and disturbance channels.
- Raw measurement, filtered measurement and independent reference roles.
- Requested command, bounded target, applied command and physical actuator
  feedback; identical unit strings do not make these the same quantity.
- Engineering units, scaling, polarity, sample/acquisition timing and quality.
- Actuator capabilities: continuous command, permitted modulation, or genuinely
  ON/OFF-only equipment; minimum switching times come from equipment constraints.
- Controller parameters, retained-state/adoption conventions, initialization,
  mode/ownership, faults, recovery, stop and restoration bindings.
- Operating range, limits, required accuracy, response windows, overshoot,
  disturbance recovery, effort/activity and applicable resource/deadline limits.

Open-loop observation does not require an invented SP/controller. Multiple
loops and MIMO channels retain their identities and cannot cross-write. Offline
model work does not require a connected runtime or live Control.PID instance.
Custom ST uses a supported controller adapter or declared representation;
signal discovery alone does not reveal an arbitrary program's control law.

The engineer can open an ordinary non-course project and complete the workflow
without handwritten internal JSON or a hand-authored lesson wrapper.

For an existing proportional ST program, model-free setup may declare
`controller.law = { kind: "proportional", gainSymbol: "<compiled symbol>" }`.
This explicitly confirms `u = Kp * (SP - PV)`; variable names never establish
that equation. Guided setup selects the gain, BOOL Manual-mode variable and
numeric manual-command variable from the compiled catalog, confirms gain bounds
and owning task, and stores their existing parameter/manual-output bindings.
The gain uses output-unit/process-unit coordinates. This declaration contains no
plant coefficients: Find or Supply provides the selected plant before building
the characteristic equation, tuning or verification. Other ST laws retain their
supported declared-analysis or observation route; setup cannot invent a plant
or silently treat an arbitrary controller as proportional.

For this proportional guided route, the observation duration must fit the
automatic adapter before setup is saved. With scan period h milliseconds and
requested duration d seconds, the trial uses n = ceil(max(2, d) * 1000 / h)
intervals. Require n + 1 <= 12,000 recorded samples and n * h <= 600,000 ms.
Reject an incompatible duration with the supported limit; do not silently
clamp it or save a configuration that Find model cannot execute.

The shared recording client uses the runtime's declared symbolic or addressed
bindings. An unaddressed compiled BOOL mode is recorded by symbol together with
SP/PV/MV, not rejected or replaced by an invented address. Preserve exact binding
identities and per-channel quality; a forced, bad, missing or invalid mode value
must not become observed Auto/Manual. Symbolic recordings retain their program
identity. Address-only historical responses remain compatible, while any supplied
quality vector must be validated and honoured.

Guided setup retains the compiled `REAL`/`LREAL` type of the selected manual
command as `controller.manualOutputType`, alongside `manualOutputSignal`.
Automatic transfer must use that declared type, never infer it from the output
channel's type. Legacy loops without this field remain readable; the transfer
adapter requires a compiled type binding before commissioning them in Auto.

## 3. Find model and Supply model converge

### 3.1 Find model

Studio proposes a bounded experiment campaign appropriate to the process and
available actuation. It can reuse qualified records or import external records
through a units/channel/timestamp mapping preview. It must not always use a
held step: pulses, relay, PRBS, multisine, operating-point tests and supported
closed-loop identification have distinct applicability.

The campaign checks baseline drift, excitation span, sample timing, gaps,
quality, clipping, saturation, noise evidence and operating constraints.
A preliminary small experiment may inform a subsequent experiment only within
the reviewed campaign's bounds and budget. Unknown/unstable integrating motion
must not receive an indefinite open-loop command.

Fit candidate structures appropriate to the evidence: static, integrating,
first-/higher-order, delayed, oscillatory, nonminimum-phase, unstable,
multivariable and supported physical/grey-box/local nonlinear models. Not every
method applies to every structure. Closed-loop data requires a suitable method
and recorded applied input; it must not be treated naively as an open-loop step.

Model selection uses independent validation, residual behavior, identifiability,
physical plausibility and complexity. It cannot select solely by fitting error.
Failed or ambiguous identification retains the record and identifies a useful
next measurement/experiment. The engineer does not have to guess coefficients
before pressing Find model.

For an existing ST controller with explicit manual command ownership,
`controller.manualOutputSignal` names the compiled command variable used in
Manual; `controller.manualOutputAddress` and `controller.manualOutputType`
declare its writable process-image address and compiled type;
`parameterBinding.mode` declares the corresponding mode and polarity. This is
a confirmed binding, never inferred from a variable name. In isolated
simulation, a bounded step campaign may set that mode/command, acquire actual
plant input and output, restore the command, allow settling, and repeat an
independent trial.
It must fit and independently validate those actual records before selecting
a model. A supplied model declaration is not identification data.

The automatic campaign retains each completed raw trial before checking its
qualification. A failed bounds/quality check, a later trial failure, cancellation
or fitting failure must not erase earlier trials. Retained campaign outcomes
include status and stop reason; only a successfully qualified campaign can
select a model. A retry creates a distinct campaign while prior records remain
inspectable. If the runtime supplies no partial record, show that absence rather
than reconstructing samples or claiming restoration was observed.

### 3.2 Supply model

Accept typed transfer functions, continuous/discrete state-space models,
compatible configured simulation models and supported physical/nonlinear models.
Show states/channels/coordinates, units, operating point/range, sample time,
delay, initial conditions, uncertainty and provenance. A canonical realization's
latent state must not be silently labelled a physical measured state.

Distinguish supplied/unvalidated, externally validated with an accepted reference,
identified/same-run preview, identified/independently qualified, and exact
simulation model. A supplied validation label is not fabricated Studio evidence.
Unvalidated models permit explicitly provisional analysis; automatic design
qualification and application must reflect the required evidence still missing.

### 3.3 Shared model use

The selected model revision MUST directly feed all compatible analysis, tuning,
simulation and verification workflows. An identified model MUST NOT need to be
re-entered as a supplied polynomial to access Routh, root locus, frequency
analysis or state-space design. Supported conversions retain domain, units,
state/coordinate meaning and conversion assumptions.

Model validation and controller verification are different operations. The
same data cannot silently serve as both estimation and independent validation.
Existing sensor/filter/actuator dynamics must be represented once, with their
location recorded. Changing a filter invalidates a model that included that
filter unless a justified conversion or new identification establishes the change.

### Operating-point coordinates

A selected linear model may declare `operatingPoint: {inputs: [...], outputs: [...]}`
in ordered absolute engineering units. Its transfer/state equations then relate
input/output deviations from that point. Vectors must match channel dimensions,
be finite and lie in the declared operating ranges. An omitted point means zero.
Routh and frequency calculations use the unchanged deviation dynamics.

The existing P law remains `u=Kp*(r-y)`; an operating point does not secretly add
controller bias. For the first-order P target-response adapter, start from zero
model deviations and actual command u0, and use absolute target y0+deltaR.
With K=b/a0, tau=a1/a0 and q=K*Kp, target steady error is
`(deltaR+K*u0)/(1+q)`. For automatic q>=0 the transient bound amplitude is
`max(abs(deltaR),abs(K*u0))`. Manual stable candidates use the conservative
amplitude `max(abs(deltaR),abs(q*deltaR/(1+q)-K*u0/(1+q)))`.
Only requested disturbance cases contribute additional design constraints.
Both baseline and candidate ST runs use identical physical/model initial
conditions and absolute references; the actual ST must produce its own output.

Identification retains the measured pre-excitation input/output point with the
fitted dynamics. It must not use supplied coefficients to find that point or
invent state coordinates from one output measurement. Nonsteady or incompatible
starting conditions require an explicit qualification failure/recovery path.

The current deterministic simulation step adapter reads one coherent, fresh,
good, unforced MV/PV/Manual scan before its campaign. It requires Manual mode
and bounded values; it never replaces an unavailable reading with zero. Each
isolated trial initializes the actual MV and manual command to that measured
MV, while preserving the configured plant initial state. Before fitting, require
at least three pre-excitation samples, all good and unclipped, whose MV/PV stay
within `1e-6 * max(1, abs(measured coordinate))` of that scan. Report this
numerical baseline tolerance in progress. This is a deliberately narrow
steady deterministic simulator check, not a noise or hidden-state certificate.
If the configured initial condition disagrees, retain the trial and ask the
engineer to establish compatible initial conditions before retrying; never
infer or overwrite hidden states. Noisy baseline qualification remains a
separate supported-method requirement.

## 4. Controller recommendation and simplicity

Recommendation depends on **model + actuator capabilities + measurements + task
requirements**, not model order alone. First discard incompatible structures.
Evaluate the least-complex applicable designs and escalate only for an unmet
requirement. The chosen family is provisional until Tune/Verify succeed.

| Detected condition/task | Initial recommendation | Escalation only when needed |
| --- | --- | --- |
| ON/OFF-only equipment | ON/OFF with justified hysteresis and required dwell | Revisit equipment/requirements if the temperature/process band cannot be met |
| Stable nearly static continuous process | P | PI if required disturbance/steady-error performance fails |
| Stable first-order process | P where sufficient; otherwise PI | Additional dynamics only for a demonstrated requirement |
| Significant transport delay | Delay-aware PI | Predictor/other supported design only if simple control cannot meet the task with uncertainty |
| Integrating process | P where adequate; PI when required load-offset rejection demands it | Further structure only after feasibility and robustness assessment |
| Torque-to-position motion | P if adequate; PD for damping | PID for required persistent-load error rejection; filtering where justified |
| Oscillatory/higher-order process | Simplest stabilizing P/PI/PD/PID compatible with the task | Lead/lag, notch or state design only where required |
| Open-loop unstable model | First find feasible stabilizing simple structures | Supported state/other design when simpler structures cannot qualify; report no feasible design otherwise |
| Nonminimum-phase model | Conservative applicable simple feedback | Respect inverse response and achievable bandwidth; never unstable cancellation |
| Operating-point dependence | Fixed controller across the range | Scheduling only when no simpler fixed candidate qualifies |
| Weak interaction | Suitable pairings and decentralized loops | Decoupling/multivariable design only for failed interaction requirements |
| Missing required states | Direct measured-output control where adequate | Observer/state feedback or LQG when justified by the task and available evidence |
| Interacting future constraints/resource limits | Simple constrained arrangement if adequate | MPC only when prediction/coordination is required |
| Measured disturbances/intermediate measurements | Simple feedback baseline | Feedforward/cascade when their information improves an unmet requirement |

Among verified sufficient designs, complexity precedes improvements beyond the
stated requirements. Complexity includes added sensors/actuators/loops,
controller/estimator states, settings, model dependence, execution demands and
commissioning/maintenance burden. A universal PI-before-PD ordering is wrong
for some motion tasks. Protective constraints are not removable complexity.

Within comparable complexity, use the engineer's declared performance/effort/
robustness preferences. No universal 10% equivalence band or 'fastest wins' rule
may override the simplest-sufficient policy. If no candidate qualifies, retain
failures and explain the limiting requirements; do not silently loosen them.
Claims identify the supported candidate families/search coverage, not global
optimality over every conceivable controller.

## 5. Tune includes all applicable design work

Tune uses the selected controller family and selected model. It automatically
chooses applicable methods, calculates settings, checks relevant stability and
creates candidates for Verify. It does not silently replace a pinned family.

Supporting choices are part of the candidate: filter need/type/placement/order/
coefficients, setpoint weighting, anti-windup/tracking, sample-time treatment,
reference/disturbance compensation, schedules and estimators. Bypass/simple
alternatives are always considered where meaningful. A smoother trace is not
an acceptance objective unless its physical cost/benefit meets the requirements.

Known controller adapters use the actual positional/incremental form, gain
units/conventions, filter discretization, limits, tracking and lifecycle behavior.
A structure change produces an isolated executable candidate plus an explicit
ST integration change for review. It must preserve unrelated program logic and
single actuator ownership. A parameter-only change must not replace the program.

| Analysis question | Automatic method selection and result |
| --- | --- |
| Time response/poles/zeros | Connected model, retained internal modes, initial-condition response and numerical confidence |
| Stability of polynomial continuous system | Routh–Hurwitz/appropriate polynomial methods, strict boundary conditions and supported parameter regions |
| Damping/response choice through a scalar gain | Named-parameter root locus with other parameters frozen; candidate selection from requirements |
| Delay/frequency-dependent stability | Complete applicable Nyquist/frequency analysis, known pole count and directed contour; frequency grid alone is not an exact certificate |
| Sampled controller | Exact supported hold/discrete conversion, pending-command/delay states and unit-circle/Jury/Schur checks |
| Disturbance/noise tradeoff | Correct sensitivity/complementary-sensitivity and actuator paths; Bode/Nyquist/Nichols and margins where applicable |
| Multivariable/state design | Ordered channel models, rank/conditioning, realizability, feedback/observer synthesis and connected poles |
| Nonlinear/uncertain system | Supported local analysis or certified regional/robust method; finite simulations labelled as tests |

A state-space connection must be well posed, including direct-feedthrough
algebraic-loop checks. Reduced input-output models must not hide unstable
internal modes. The full domain and approximations remain visible in results.

## 6. Explicit Routh–Hurwitz workflow and minimum scope

Routh–Hurwitz is a first-class required workflow, detailed in
[WF-048](28-54-implementation-motion-and-classical-analysis.md#wf-048).
It is available automatically during Tune when applicable and manually through
analysis. It consumes the selected compatible model from either entry path.

1. Obtain plant and actual controller representations and the parameters selected
   for design. For delay-free scalar negative feedback, form `Dp*Dc+Np*Nc`;
   for a supported state representation derive the connected characteristic
   polynomial while retaining internal states.
2. Display the polynomial, coefficient meanings and fixed parameters.
3. Construct the Routh table and show row calculations, pivots, first-column
   signs and the established right-half-plane pole count.
4. Derive strict inequalities and supported stable intervals/regions, including
   disconnected intervals, configured-domain intersections and excluded boundaries.
   A sampled gain table is not the result of this operation.
5. Resolve supported zero pivots with a rigorous limiting/symbolic method and
   all-zero rows using the auxiliary polynomial. A finite epsilon substitution
   is not an exact proof. Report unresolved special cases or numerical uncertainty.
   A zero row alone does not establish imaginary-axis roots.
6. Pass eligible candidates and the region directly to tuning/verification.
   Do not require the learner to enter each gain to discover the region.

Minimum delivery must include numerical tables through degree 16 and region
calculation for the single-parameter affine polynomial families through degree 4
needed by the course's P/PI/PID cases, plus the two-parameter second-order
region below. Additional families have explicit capability/domain contracts;
the existing degree 1/2 implementation limit is an implementation gap.

Required independent mathematical acceptance examples:

| Polynomial/model | Required automatic result |
| --- | --- |
| `s²+s+Kp` | `Kp>0`; `Kp=0` has poles0,-1; negative Kp has one RHP pole |
| `s³+8s²+16s+Ki` | `0<Ki<128`; origin at0; imaginary pair at128; two RHP poles at200 |
| `s⁴+2s³+3s²+4s+k` | `0<k<2`; boundary at2 contains an imaginary pair |
| `J s²+(B+Kd)s+Kp`, fixed `J>0` | Region `Kp>0` and `Kd>-B`, intersected with configured design bounds |
| Zero pivot / all-zero row / near-singular numerical case | Correct supported resolution or explicit unresolved status; no false stable count |
| Model with exact transport delay | Explain polynomial-method inapplicability and offer the appropriate delay analysis; no silent Padé substitution |

Repeat model-to-analysis tests through both a supplied and an identified-model
entry. Demonstrate a non-shaft plant so the result cannot be hardcoded.

## 7. Verify: actual implementation, matched cases

Verify instantiates a compatible simulated plant from the selected model and
runs the actual baseline/candidate ST implementations in isolated executions.
The model, program, parameter, binding and scenario revisions are retained.
No physical I/O drivers may be inherited by an isolated simulation.

One immutable test specification defines physical initial state, target/load
trajectories, noise/uncertainty cases, sample/update sequence, duration, limits
and stopping rules. Controller initialization is candidate-specific but must
represent the same physical operating condition; P and PI need not have
identical internal state vectors. Altered controller code is an explicit
candidate difference, not grounds to pretend program revisions are identical.

Run all applicable requirements: accuracy, rise/settling, overshoot/undershoot,
load rejection, output/rate/saturation/recovery, switching/dwell, cross-effects,
state/actuator limits, timing/deadline, initialization/transfer, parameter adoption,
measurement validity, network/fault and deliberate recovery. Long processes
have process-appropriate durations, not a universal 30 s monitoring window.

Every requirement reports Pass/Fail/Inconclusive, value, limit, unit, interval,
evidence source and reason. Failed mandatory requirements precede ranking.
Missing or excluded required data cannot create Pass. Model prediction,
ST-executed simulation and physical observation are distinct evidence classes.

Controller verification failure returns to the relevant cause: data/acquisition,
model structure/identifiability, settings, controller structure, implementation,
or equipment/requirements. It must not always increase gains or complexity.

Required sampled-data acceptance additionally includes a held integrator:
continuous `s+Kp` is stable for `Kp>0`; with zero-order hold/sample period `h`,
`x[k+1]=(1-h*Kp)x[k]` is stable for `0<h*Kp<2`; adding one pending-command
update gives `z²-z+h*Kp`, stable for `0<h*Kp<1`. Studio must distinguish them.

The debugger's live simulation snapshot must remain available between scans,
including tasks whose wall-clock period exceeds the control-request timeout.
The runner releases runtime and simulation locks before waiting for the next
scan. A sleeping controller must not make an active simulator appear absent or
prevent Verify from inspecting its configuration.

### 7.0 Cancellation confirmation and retained verification attempts

A cancel reply acknowledges a request; queued/running is not a terminal stop.
Studio must await the owned job's completed/failed/cancelled status and fetch
its available terminal evidence before reporting cancellation finished. The
endpoint, job and project identity remain pinned through this recovery. A job
that finished before cancellation is retained as completed, but a cancelled
Verify operation must not authorize Apply or start its next scenario.

If confirmation fails or times out, retain `cancellation-unconfirmed` with the
job identity and reason. Report the uncertainty, block new dependent work and
allow Cancel to retry confirmation of that same job. Never infer terminal
cancellation from an acknowledgement or fabricate samples that were not returned.
Retained verification attempts belong to the current study and remain separate
from a passing baseline/candidate comparison.

### 7.1 Selected simulation model in isolated execution

`control.simulation.run.start` accepts optional `simulation_override` with
`model_revision`, `model_provenance` and `simulation_toml`. These identify the
selected supplied/identified model and its materialized compatible simulation.
The runtime validates the bounded inline configuration using the existing
simulation parser before starting a worker. Empty identity/provenance, invalid
configuration, disabled simulation or absence of a plant system is rejected.
This field cannot select files, external drivers or physical I/O.

The worker freezes the selected configuration in memory and executes the loaded
ST source revision against it. Returned conditions retain the actual simulation
configuration digest (`simulationConfigRevision`, SHA-256 of the exact UTF-8
TOML bytes, prefixed `sha256:`) and the declared selected-model identity/provenance;
provenance text alone is not independent model-validation evidence. Initial
conditions, events, sample timing and candidate parameter overrides remain
explicit and matched. Without an override the existing configured-plant path
is unchanged. Cancellation, failure and project-revision checks apply equally
to both paths.

## 8. Apply, recovery and continuous use

The task-boundary mode/reference/state protocol is specified in
[guarded controller transfer](../control-studio-runtime-protocol.md#e6-transfers).
The connected P operator sequence is specified in
[Auto commissioning](../35-vscode-control-studio.md#c13-connected-p-commissioning-proportional-only-adapter).

Apply reviews the exact target, current/candidate settings or ST change,
required mode, evidence and restoration policy. Parameters commit coherently at
the owning update boundary after rechecking type, bounds, revision, values,
force/authority and mode. Confirm observed state; queued is not applied.
Initialization/tracking follows the real controller convention, including P/D/
feedforward contributions where applicable. Intentional tighter limits are
reported separately from accidental transfer bumps.

Retain the prior confirmed revision for guarded rollback. Editor close/reload,
network interruption, stale data and cancellation cannot leave an uncontrolled
experiment. Normal completion, abort and fault may have different declared
restoration actions; zero output is not a universal safe state.

A reviewed campaign may authorize its listed bounded physical trials once.
No new approval is needed for its internal calculations or permitted trials;
materially different actions require a revised review. Applying to a physical
target remains a separate explicit action unless already included in a valid
specific authorization. Runtime protection and ownership checks remain active.

The actual control algorithm executes independently of the Studio window.
Deployed adaptive/MPC/estimator behavior must be part of its verified runtime
implementation, not an editor callback controlling the process.

Selected-model, candidate and validation eligibility remain separate runtime
study states. Input/model changes invalidate dependent results; reopened
historical evidence does not acquire current validation or write authority.
Present these states only through the current three-page interaction contract.

Replacing the selected plant through Supply model is rejected while an Engineer
operation is active or cancellation remains unconfirmed. Reject before changing
model, analysis, candidate or records. Finish/cancel the owned operation, retain
its terminal evidence, then retry the import. Internal acceptance of the current
Find campaign is a separate owned handoff and remains allowed. Rejected imports
must not overwrite current stage readiness with a generic missing-data state.

## 9. Manual operations and result continuity

Every detailed workflow exposes its inputs, formulas/method, outputs and manual
controls. Users may pin model, controller family, gains, filter, estimator,
method or scenarios and automate the remaining choices. Inapplicable mathematics
returns a reason; manual mode does not fabricate a result or bypass runtime
permissions and physical constraints.

Manual exploratory candidates can exist without automatic qualification and
remain labelled. Their missing evidence is shown when Verify/Apply is requested.
Returning to automatic mode preserves the study and pinned choices.

Models, records and candidates transfer directly between stages and projects
through reviewed imports with original provenance; imported historical evidence
cannot create write authority on a different project. No handwritten JSON or
retyped calculated settings are required. Retain raw records, exclusions,
residuals, mathematical results (including Routh tables/regions), every candidate,
failed/inconclusive tests, recommendation reasons and confirmed application.
Changing data/model/structure/timing invalidates dependent results; display-only
changes do not. Historical evidence must never appear as a current live value.
The candidate record retains its actual controller structure and distinguishes
unapplied prediction, actual-ST validation, history and confirmed application.
Invalidated candidates cannot retain application eligibility. Manual staging
must not be reported as Auto adoption. The current page contracts own visible
results; this does not create a candidate header or status strip.

### 9.1 Engineer study persistence

Save captures its original editor/loop state before asynchronous work. If that
editor closes or another loop opens while Save is pending, finish persisting
only the captured original result. Do not read the new editor's models/history
or publish the old Save completion into its UI. Pending storage is not authority
to change the newly opened study. A late Save failure is likewise scoped to
its original editor; it must not become an error on the newly opened loop.

A schema-v4 workflow result MAY retain an Engineer Study independently of a
current application transaction. The study retains the selected `GeneralModel`
with its original source, validation label and evidence, the declared
`EngineerRequirements`, current supported controller family, and completed,
failed or cancelled Find campaigns. A failed Find campaign remains saveable
without a selected model, task or controller family. Campaign records have a
distinct session identifier and retain their raw isolated-simulation runs and
recomputed requirement evaluations. Historical baseline and candidate
verification records MAY also be retained when available.
The optional `initialization` result retains the same validated actual-ST run,
frozen request and recomputed evaluation contract for the Auto-entry case.
Reopening that record restores history, not current transfer qualification.

The study accepts at most 16 terminal campaigns and at most 16 MiB of serialized
study data. A save or restore that exceeds either bound is rejected with a
specific reason; it MUST NOT silently discard a campaign, run or sample.
Malformed model/task/run data, a mismatched stored evaluation, or an unknown
study field is rejected. Stored historical evidence does not restore a runtime
client, pending job, review, approval, queued parameter write, or authority to
Apply. Reopening it remains historical until the engineer starts a new explicit
verification or application review.

Opening another saved study while Find, Verify or an application transaction
is active must fail before changing the current model, history or approval
state. Explain that the engineer must finish the operation, or cancel it and
wait for its retained terminal evidence, then retry Open. A late completion
from the old operation must never append records to the newly opened study.

The optional `verificationAttempts` list retains at most16 verification outcomes.
Each has `status` (completed, failed, cancelled or cancellation-unconfirmed),
`jobId`, `projectRevision`, the frozen `request`, `reason` and optional actual
`run`. Unconfirmed cancellation is evidence of missing confirmation, not a
terminal runtime classification. Update an existing job's retained outcome on
retry rather than adding a duplicate. Saving remains subject to the study's
16MiB bound. Reopening never recreates a running client or approval.

### 9.2 Manual P gain within the connected first-order adapter

Automatic gain selection remains the default. For the existing declared P law,
choose Manual, enter a gain and press Tune to commit that pin as the candidate
input. Keep the selected model and all downstream actual-ST verification and
application checks. Never replace a valid manual gain with the automatically
calculated gain because it misses a target. Label it manual/exploratory until
Verify supplies actual execution evidence; unstable/boundary predictions have
no claimed finite asymptotic error bound. Run only the bounded isolated tests.

For the stable first-order manual case define K=b/a0, tau=a1/a0 and q=K*Kp.
The pole is -(1+q)/tau. A reference transient bound uses
`|r| * max(1, |q/(1+q)|) * exp(pole*T)`; the extra factor is necessary for
stable negative q. Overflow or underflow must report numerical uncertainty,
not masquerade as a stability boundary or finite error bound.

The optional schema-v4 Engineer Study field `gainChoice` is
`{mode: "automatic" | "manual", pin?: {parameterSymbol, unit, loopRevision,
modelRevision, value}}`. Manual requires a pin. Its finite value must fit the
existing gain binding; REAL values are realized with IEC single-precision
rounding before analysis and the realized value is displayed. Pin identities
must match the active loop revision, parameter symbol, units and selected model
revision. Reject incompatible imported pins before restoring any study state.

Switching to Automatic and pressing Tune retains the compatible pin for later
manual use while choosing a separate automatic candidate. Task edits retain
the pin but invalidate candidate/verification evidence. A changed model, loop
law, parameter binding or units invalidates the pin and its dependent evidence.
Saving/reopening retains the choice and compatible pin, restores their controls,
and never restores Apply authority. A manual pin that fails Verify remains
inspectable and cannot be applied through the verified Engineer workflow.
This P slice does not establish pins for other gains, filters or estimators.

### 9.3 Recorded response evidence

Retain actual baseline/candidate recordings independently of recalculation.
Preserve missing/forced/bad samples as gaps, actual command bounds and recorded
events with their time windows. Never substitute live or predicted data for a
selected execution record. Any exported comparison uses those same records and
retains their model/program/source identity. The Validate page owns the visible
measured response; this contract does not prescribe additional tabs or toolbars.

## 10. Complete topic workflow catalogue

The three normative companions below specify **every mapped topic individually**.
Each workflow defines trigger/inputs, automatic operations, manual controls,
recommendation/escalation and verification/acceptance/failure behavior. Their
numeric lesson cases are acceptance examples, not limits on the product to a
particular heater, shaft or tank. This master contract governs shared defaults.

- [WF-001–WF-027: foundations, identification and PID](01-27-foundations-identification-and-pid.md)
- [WF-028–WF-054: implementation, motion and classical analysis](28-54-implementation-motion-and-classical-analysis.md)
- [WF-055–WF-082: industrial, multivariable and advanced control](55-82-industrial-multivariable-and-advanced-control.md)

The traceability register at the end lists every source and its workflow.
No generic 'advanced analysis' row substitutes for a named topic such as
Routh–Hurwitz, Nyquist, controllability, Kalman filtering or Lyapunov stability.

## 11. Lessons and implementation acceptance

Preserve the 82-row progression and user-approved explanations. Each lesson must
answer its headings, explain the topic/purpose/mechanism/formulas/symbols with a
worked calculation, predict a result, and teach the actual Studio operation.
Manual arithmetic teaches why; it is not a missing automatic-feature workaround.
Calculations inside the actual controller or an explicitly implemented physical
model remain legitimate ST work, distinct from Studio's engineering analysis.

Before accepting a lesson, independently check the complete prose, model and
actual ST/plant/reset/timing/units, native behavior, rendered operations and
packaged Start-from-example path. Wrong executable physics is repaired to the
approved lesson, not used to rewrite the lesson's physical case. Missing product
operations are implemented generically and tested, not hidden behind a TODO.

Minimum product acceptance journeys:

- Non-course existing ST project: Find model -> independent model validation ->
  controller recommendation/override -> Tune -> actual-ST Verify -> Apply/readback
  and rollback using the permitted simulated target.
- The same downstream operations from Supply model, including Routh/other
  compatible mathematics without re-entering coefficients.
- Custom ST and recognized-controller cases; symbolic and addressed signals;
  multiple loops; rectangular and square MIMO; continuous and discrete models.
- Simple adequate candidate wins over a more complex candidate that merely
  exceeds requirements; escalation happens only for a stated missing capability.
- Automatic filter/no-filter selection and controller co-design tested against
  noise and real disturbances; no smoothing-only acceptance.
- Invalid/incomplete data, ambiguous model, infeasible requirement, no qualifying
  controller, mathematical uncertainty, cancelled campaign, stale/forced/mismatched
  target, failed adoption/restoration and recovery.
- Light/dark/high-contrast rendered views; accessible controls and understandable
  errors; source/package/readme/model/program consistency.

For implementation, use written contract -> focused expected assertion red ->
minimum behavior -> same green. Register real extension tests; verify the real
rendered surface separately. Full required gates remain required before product
completion. Specification coverage is not implementation or course acceptance.

## 12. Source-to-workflow traceability

| Workflow | Topic and lesson source | Invoked by default at |
| --- | --- | --- |
| [WF-001](01-27-foundations-identification-and-pid.md#wf-001) | [1. Control-loop elements](../../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/README.md) | Setup / Monitor |
| [WF-002](01-27-foundations-identification-and-pid.md#wf-002) | [2. ON/OFF control](../../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/README.md) | Choose / Tune |
| [WF-003](01-27-foundations-identification-and-pid.md#wf-003) | [3. Hysteresis and dwell](../../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/README.md) | Tune / Verify |
| [WF-004](01-27-foundations-identification-and-pid.md#wf-004) | [4. Cold-room commissioning](../../../examples/control/14-integrated-capstones/80-cold-storage/README.md) | Verify |
| [WF-005](01-27-foundations-identification-and-pid.md#wf-005) | [5. Proportional control](../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/README.md) | Choose / Tune |
| [WF-006](01-27-foundations-identification-and-pid.md#wf-006) | [6. PI control](../../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi/README.md) | Choose / Tune |
| [WF-007](01-27-foundations-identification-and-pid.md#wf-007) | [7. Saturation and windup](../../../examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-saturation-windup/README.md) | Tune / Verify |
| [WF-008](01-27-foundations-identification-and-pid.md#wf-008) | [8. Anti-windup](../../../examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-anti-windup/README.md) | Tune / Verify |
| [WF-009](01-27-foundations-identification-and-pid.md#wf-009) | [9. Setpoint and disturbance response](../../../examples/control/01-feedback-foundations-and-temperature-control/09-setpoint-and-disturbance-response/README.md) | Verify |
| [WF-010](01-27-foundations-identification-and-pid.md#wf-010) | [10. Loop diagnosis](../../../examples/control/09-industrial-control-structures/60-loop-diagnosis/README.md) | Find model / Verify |
| [WF-011](01-27-foundations-identification-and-pid.md#wf-011) | [11. Manual PI tuning](../../../examples/control/06-pid-tuning-methods/28-manual-pi-tuning/README.md) | Tune (manual or assisted) |
| [WF-012](01-27-foundations-identification-and-pid.md#wf-012) | [12. Process dynamics](../../../examples/control/01-feedback-foundations-and-temperature-control/13-process-response-classes/README.md) | Find model |
| [WF-013](01-27-foundations-identification-and-pid.md#wf-013) | [13. Energy balance](../../../examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/README.md) | Supply model / Find model |
| [WF-014](01-27-foundations-identification-and-pid.md#wf-014) | [14. Identification experiments](../../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment/README.md) | Find model |
| [WF-015](01-27-foundations-identification-and-pid.md#wf-015) | [15. Step-response identification](../../../examples/control/05-experiments-identification-and-validation/23-step-response-identification/README.md) | Find model |
| [WF-016](01-27-foundations-identification-and-pid.md#wf-016) | [16. Model structure](../../../examples/control/05-experiments-identification-and-validation/24-choose-model-structure/README.md) | Find model |
| [WF-017](01-27-foundations-identification-and-pid.md#wf-017) | [17. Identifiability](../../../examples/control/05-experiments-identification-and-validation/25-parameter-identifiability/README.md) | Find model |
| [WF-018](01-27-foundations-identification-and-pid.md#wf-018) | [18. Model validation](../../../examples/control/05-experiments-identification-and-validation/26-independent-model-validation/README.md) | Find model / Supply model qualification |
| [WF-019](01-27-foundations-identification-and-pid.md#wf-019) | [19. Model-based PI tuning](../../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning/README.md) | Tune |
| [WF-020](01-27-foundations-identification-and-pid.md#wf-020) | [20. Relay tuning](../../../examples/control/06-pid-tuning-methods/29-relay-tuning/README.md) | Find model / Tune |
| [WF-021](01-27-foundations-identification-and-pid.md#wf-021) | [21. Tuning comparison](../../../examples/control/06-pid-tuning-methods/31-tuning-method-comparison/README.md) | Verify |
| [WF-022](01-27-foundations-identification-and-pid.md#wf-022) | [22. Setpoint weighting](../../../examples/control/01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting/README.md) | Tune |
| [WF-023](01-27-foundations-identification-and-pid.md#wf-023) | [23. Measurement filtering](../../../examples/control/01-feedback-foundations-and-temperature-control/11-measurement-filtering/README.md) | Tune |
| [WF-024](01-27-foundations-identification-and-pid.md#wf-024) | [24. Sampling and timing](../../../examples/control/01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling/README.md) | Find model / Tune / Verify |
| [WF-025](01-27-foundations-identification-and-pid.md#wf-025) | [25. Controller initialization](../../../examples/control/03-production-quality-digital-pid/39-controller-mode-initialization/README.md) | Tune / Verify / Apply |
| [WF-026](01-27-foundations-identification-and-pid.md#wf-026) | [26. Bumpless transfer](../../../examples/control/03-production-quality-digital-pid/40-bumpless-transfer/README.md) | Verify / Apply |
| [WF-027](01-27-foundations-identification-and-pid.md#wf-027) | [27. Parameter updates](../../../examples/control/03-production-quality-digital-pid/41-atomic-parameter-update/README.md) | Apply / Verify |
| [WF-028](28-54-implementation-motion-and-classical-analysis.md#wf-028) | [28. Measurement validity](../../../examples/control/03-production-quality-digital-pid/42-invalid-stale-data/README.md) | Verify / Monitor |
| [WF-029](28-54-implementation-motion-and-classical-analysis.md#wf-029) | [29. Heater commissioning](../../../examples/control/14-integrated-capstones/81-water-heater/README.md) | Verify |
| [WF-030](28-54-implementation-motion-and-classical-analysis.md#wf-030) | [30. Actuator tracking](../../../examples/control/01-feedback-foundations-and-temperature-control/08-heating-valve-rate-tracking/README.md) | Find model / Tune / Verify |
| [WF-031](28-54-implementation-motion-and-classical-analysis.md#wf-031) | [31. Valve characteristics](../../../examples/control/09-industrial-control-structures/58-valve-characterization/README.md) | Find model |
| [WF-032](28-54-implementation-motion-and-classical-analysis.md#wf-032) | [32. Valve stiction](../../../examples/control/09-industrial-control-structures/59-valve-stiction/README.md) | Find model / Verify |
| [WF-033](28-54-implementation-motion-and-classical-analysis.md#wf-033) | [33. Position control](../../../examples/control/02-derivative-action-and-dynamic-response/32-proportional-position-stiffness/README.md) | Choose / Tune |
| [WF-034](28-54-implementation-motion-and-classical-analysis.md#wf-034) | [34. PD control](../../../examples/control/02-derivative-action-and-dynamic-response/33-derivative-motion-damping/README.md) | Tune |
| [WF-035](28-54-implementation-motion-and-classical-analysis.md#wf-035) | [35. PID load rejection](../../../examples/control/02-derivative-action-and-dynamic-response/34-servo-pid-holding-torque/README.md) | Tune |
| [WF-036](28-54-implementation-motion-and-classical-analysis.md#wf-036) | [36. Derivative filtering](../../../examples/control/02-derivative-action-and-dynamic-response/36-filtered-derivative/README.md) | Tune |
| [WF-037](28-54-implementation-motion-and-classical-analysis.md#wf-037) | [37. Discrete PID](../../../examples/control/03-production-quality-digital-pid/37-discrete-integral-derivative/README.md) | Tune / Verify |
| [WF-038](28-54-implementation-motion-and-classical-analysis.md#wf-038) | [38. Positional and incremental PID](../../../examples/control/03-production-quality-digital-pid/38-positional-incremental-pid/README.md) | Tune / Verify |
| [WF-039](28-54-implementation-motion-and-classical-analysis.md#wf-039) | [39. Servo commissioning](../../../examples/control/14-integrated-capstones/82-servo/README.md) | Verify |
| [WF-040](28-54-implementation-motion-and-classical-analysis.md#wf-040) | [40. Volume balance](../../../examples/control/01-feedback-foundations-and-temperature-control/14-conservation-and-accumulation/README.md) | Supply model / Find model |
| [WF-041](28-54-implementation-motion-and-classical-analysis.md#wf-041) | [41. Gravity outflow](../../../examples/control/01-feedback-foundations-and-temperature-control/16-gravity-tank-nonlinearity/README.md) | Supply model / Find model |
| [WF-042](28-54-implementation-motion-and-classical-analysis.md#wf-042) | [42. Linearization](../../../examples/control/01-feedback-foundations-and-temperature-control/17-gravity-tank-linearization/README.md) | Find model / Tune |
| [WF-043](28-54-implementation-motion-and-classical-analysis.md#wf-043) | [43. Gain scheduling](../../../examples/control/09-industrial-control-structures/56-gain-scheduling/README.md) | Choose / Tune / Verify |
| [WF-044](28-54-implementation-motion-and-classical-analysis.md#wf-044) | [44. Laplace transforms](../../../examples/control/01-feedback-foundations-and-temperature-control/18-transfer-function-from-physics/README.md) | Supply model / Tune |
| [WF-045](28-54-implementation-motion-and-classical-analysis.md#wf-045) | [45. Poles and zeros](../../../examples/control/02-derivative-action-and-dynamic-response/19-poles-zeros-and-response/README.md) | Find model / Tune |
| [WF-046](28-54-implementation-motion-and-classical-analysis.md#wf-046) | [46. Characteristic equations](../../../examples/control/07-classical-stability-and-frequency-design/43-characteristic-equation/README.md) | Tune |
| [WF-047](28-54-implementation-motion-and-classical-analysis.md#wf-047) | [47. Root locus](../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/README.md) | Tune |
| [WF-048](28-54-implementation-motion-and-classical-analysis.md#wf-048) | [48. Routh–Hurwitz stability](../../../examples/control/07-classical-stability-and-frequency-design/44-routh-hurwitz/README.md) | Tune |
| [WF-049](28-54-implementation-motion-and-classical-analysis.md#wf-049) | [49. Periodic excitation](../../../examples/control/05-experiments-identification-and-validation/22-excitation-for-dynamics/README.md) | Find model |
| [WF-050](28-54-implementation-motion-and-classical-analysis.md#wf-050) | [50. Frequency response](../../../examples/control/07-classical-stability-and-frequency-design/46-bode-frequency-response/README.md) | Find model / Tune |
| [WF-051](28-54-implementation-motion-and-classical-analysis.md#wf-051) | [51. Sensitivity functions](../../../examples/control/07-classical-stability-and-frequency-design/48-sensitivity-functions/README.md) | Tune |
| [WF-052](28-54-implementation-motion-and-classical-analysis.md#wf-052) | [52. Loop shaping](../../../examples/control/07-classical-stability-and-frequency-design/49-loop-shaping/README.md) | Tune |
| [WF-053](28-54-implementation-motion-and-classical-analysis.md#wf-053) | [53. Nyquist stability](../../../examples/control/07-classical-stability-and-frequency-design/47-nyquist-stability/README.md) | Tune |
| [WF-054](28-54-implementation-motion-and-classical-analysis.md#wf-054) | [54. Sampled-data stability](../../../examples/control/07-classical-stability-and-frequency-design/50-position-z-plane/README.md) | Tune / Verify |
| [WF-055](55-82-industrial-multivariable-and-advanced-control.md#wf-055) | [55. Feedforward control](../../../examples/control/09-industrial-control-structures/51-feedforward/README.md) | Choose / Tune |
| [WF-056](55-82-industrial-multivariable-and-advanced-control.md#wf-056) | [56. Cascade control](../../../examples/control/09-industrial-control-structures/52-cascade/README.md) | Choose / Tune |
| [WF-057](55-82-industrial-multivariable-and-advanced-control.md#wf-057) | [57. Ratio control](../../../examples/control/09-industrial-control-structures/53-ratio-control/README.md) | Choose / Tune |
| [WF-058](55-82-industrial-multivariable-and-advanced-control.md#wf-058) | [58. Split-range control](../../../examples/control/09-industrial-control-structures/54-split-range/README.md) | Choose / Tune |
| [WF-059](55-82-industrial-multivariable-and-advanced-control.md#wf-059) | [59. Override control](../../../examples/control/09-industrial-control-structures/55-override-control/README.md) | Choose / Tune |
| [WF-060](55-82-industrial-multivariable-and-advanced-control.md#wf-060) | [60. Smith prediction](../../../examples/control/09-industrial-control-structures/57-smith-predictor/README.md) | Choose / Tune |
| [WF-061](55-82-industrial-multivariable-and-advanced-control.md#wf-061) | [61. Vessel commissioning](../../../examples/control/14-integrated-capstones/83-reactor-control/README.md) | Verify |
| [WF-062](55-82-industrial-multivariable-and-advanced-control.md#wf-062) | [62. Multivariable process paths](../../../examples/control/10-multivariable-control/61-mimo-paths/README.md) | Find model |
| [WF-063](55-82-industrial-multivariable-and-advanced-control.md#wf-063) | [63. Relative gain array](../../../examples/control/10-multivariable-control/62-relative-gain-array/README.md) | Choose / Tune |
| [WF-064](55-82-industrial-multivariable-and-advanced-control.md#wf-064) | [64. Decentralized PI](../../../examples/control/10-multivariable-control/63-decentralized-pi/README.md) | Tune |
| [WF-065](55-82-industrial-multivariable-and-advanced-control.md#wf-065) | [65. Static decoupling](../../../examples/control/10-multivariable-control/64-decoupling/README.md) | Tune |
| [WF-066](55-82-industrial-multivariable-and-advanced-control.md#wf-066) | [66. Multivariable design selection](../../../examples/control/10-multivariable-control/65-robust-mimo-selection/README.md) | Verify |
| [WF-067](55-82-industrial-multivariable-and-advanced-control.md#wf-067) | [67. State-space models](../../../examples/control/02-derivative-action-and-dynamic-response/20-state-equations-from-stores/README.md) | Supply model / Find model |
| [WF-068](55-82-industrial-multivariable-and-advanced-control.md#wf-068) | [68. Controllability](../../../examples/control/11-state-space-estimation-and-predictive-control/66-controllability/README.md) | Choose / Tune |
| [WF-069](55-82-industrial-multivariable-and-advanced-control.md#wf-069) | [69. Observability](../../../examples/control/11-state-space-estimation-and-predictive-control/67-observability/README.md) | Choose / Tune |
| [WF-070](55-82-industrial-multivariable-and-advanced-control.md#wf-070) | [70. State feedback](../../../examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/README.md) | Tune |
| [WF-071](55-82-industrial-multivariable-and-advanced-control.md#wf-071) | [71. State observers](../../../examples/control/11-state-space-estimation-and-predictive-control/70-state-observer/README.md) | Tune |
| [WF-072](55-82-industrial-multivariable-and-advanced-control.md#wf-072) | [72. Kalman filtering](../../../examples/control/11-state-space-estimation-and-predictive-control/71-kalman-filter/README.md) | Tune |
| [WF-073](55-82-industrial-multivariable-and-advanced-control.md#wf-073) | [73. LQR control](../../../examples/control/11-state-space-estimation-and-predictive-control/69-lqr/README.md) | Tune |
| [WF-074](55-82-industrial-multivariable-and-advanced-control.md#wf-074) | [74. LQG control](../../../examples/control/11-state-space-estimation-and-predictive-control/72-lqg/README.md) | Tune |
| [WF-075](55-82-industrial-multivariable-and-advanced-control.md#wf-075) | [75. Model predictive control](../../../examples/control/11-state-space-estimation-and-predictive-control/73-mpc/README.md) | Tune |
| [WF-076](55-82-industrial-multivariable-and-advanced-control.md#wf-076) | [76. Lyapunov stability](../../../examples/control/12-nonlinear-adaptive-and-robust-control/74-lyapunov-stability/README.md) | Tune / Verify |
| [WF-077](55-82-industrial-multivariable-and-advanced-control.md#wf-077) | [77. Feedback linearization](../../../examples/control/12-nonlinear-adaptive-and-robust-control/75-feedback-linearization/README.md) | Tune |
| [WF-078](55-82-industrial-multivariable-and-advanced-control.md#wf-078) | [78. Adaptive control](../../../examples/control/12-nonlinear-adaptive-and-robust-control/76-adaptive-control/README.md) | Find model / Tune / Apply / Monitor |
| [WF-079](55-82-industrial-multivariable-and-advanced-control.md#wf-079) | [79. Robust control](../../../examples/control/12-nonlinear-adaptive-and-robust-control/77-robust-control/README.md) | Tune / Verify |
| [WF-080](55-82-industrial-multivariable-and-advanced-control.md#wf-080) | [80. Networked control](../../../examples/control/13-networked-fault-aware-and-diagnostic-control/78-networked-control/README.md) | Verify / Monitor |
| [WF-081](55-82-industrial-multivariable-and-advanced-control.md#wf-081) | [81. Fault-aware control](../../../examples/control/13-networked-fault-aware-and-diagnostic-control/79-fault-aware-control/README.md) | Verify / Monitor |
| [WF-082](55-82-industrial-multivariable-and-advanced-control.md#wf-082) | [82. Independent design](../../../examples/control/14-integrated-capstones/84-independent/README.md) | Complete workflow |

Coverage checks establish document traceability only. Implementation requires
all operation-specific assertions, real rendered workflows and packaged
lesson checks stated above; 82 rows do not mean 82 accepted implementations.

A P gain pin is bound to the complete selected model data, including operating
coordinates, as well as the displayed revision. Reusing a supplied revision
with changed coefficients or coordinates invalidates the pin. Restoring a
study binds its pin to that study's validated selected-model snapshot.
