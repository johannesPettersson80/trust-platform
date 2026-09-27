# General Control Studio models

Shared purpose and completion claims: [Control Platform Purpose and Acceptance Contract](control-platform-purpose.md).
This document retains its detailed technical or teaching authority.

Companion to Specification 35 section 17.2. This is a truST analysis contract;
it does not change IEC language semantics.

A supplied model declares its revision, source (provided, fitted, simulation),
validation status, evidence references, ordered input/output channel IDs and
units, operating ranges, and delay in seconds. A validated model requires an
evidence reference. No live controller or identification record is required
for predicted analysis of an unvalidated supplied model.

Transfer functions use descending continuous-time polynomial coefficients.
State-space models use A, B, C, D matrices in declared state/input/output order;
discrete models additionally require a positive sample interval in seconds.
Channel count is independent of state count and need not be square. Matrices,
polynomials, channel metadata and bounds are checked before numerical work.

Frequency analysis returns one complex response for every output/input pair
at each requested positive angular frequency, preserving channel order and
model provenance. Continuous models evaluate at j omega; discrete models at
exp(j omega T), rejecting frequencies at or above Nyquist. Delay multiplies
each response by exp(-j omega delay). Solves and eigenvalues reuse the existing
numerical adapter. Singular frequencies reject the request instead of emitting
nonfinite points. Pole results explicitly exclude pure delay dynamics.

Relative gain analysis requires a square nonsingular DC gain matrix; its
unavailability does not disable rectangular frequency response. Time response
and transfer conversion remain explicitly unavailable until supported by a
qualified adapter. Model pole stability uses the left half-plane for continuous
models and unit circle for discrete models; this is not closed-loop stability.

## Supplied model analysis and job execution

An explicit manual gain remains selected even if P is the only realizable
family. Declared parameter bounds still apply. A rejected model import cannot
qualify analysis or replace a previous valid model.

Selected-channel analysis retains input/output units, frequency magnitude and
phase, complex response, poles, provenance and relative-gain availability.
These are plant responses, not inferred controller margins or closed-loop
stability. The current page specifications own their presentation; this
technical contract does not add model-import forms or analysis toolbars.

Import text is limited to 256000 bytes. Frequency requests require 2–512 points,
a positive increasing range, and at most 16384 total channel-frequency points.
One worker-thread job runs per panel; it has a ten-second wall-clock timeout.
Cancel, replacement import and disposal terminate the worker and suppress late
results. No large numerical solve runs on the extension-host or webview thread.
Messages bind project, editor session, loop and revision. Explicitly saved v4
results retain the provided model, requested grid and finite calculated result;
restoring evidence does not recompute or invent an identification recording.

## Explicit selected-channel negative feedback

Remove controller is available only while a controller is attached. Removing it
clears the file selection and restores plant-only analysis without an empty
removal action.

A `GeneralFeedbackDefinition` contains the exact supplied `plant` and
`controller` GeneralModels, explicit `{inputId, outputId}` channel selections
for each, and `feedback: "negative"`. Controller input units equal plant output
units and controller output units equal plant input units. Both models must
be continuous, or both discrete with exactly the same sample interval. Other
inputs remain fixed at their declared operating point. This selected-channel
closure does not qualify an arbitrary full MIMO feedback controller.

The analysis retains plant and controller provenance separately and calculates
L = G_selected C_selected on the requested grid. Bode and the directed Nyquist
curve show this open-loop response. The −1 point is shown as a reference; a
finite sampled curve alone never establishes the complete encirclement count.
Continuous transfer-function pairs may report bounded-range classical margin
estimates using the existing numerical method. No observed phase crossover is
reported as unavailable, not infinite margin. These are sampling-limited
estimates, not a stability qualification. If adjacent estimator frequencies
would advance pure-delay phase by more than pi/2, margin estimation is
unavailable until the range is narrowed. Plotted phase is explicitly wrapped.
State-space margin calculation is
explicitly unavailable until a qualified adapter supports it.

For delay-free models, use the exact selected-channel state-space interconnection
and existing eigenvalue solver for closed-loop poles. Proper continuous transfer
functions use their canonical state-space realization without silent pole-zero
cancellation. For plant Dp and controller Dc, reject a singular or numerically
ill-conditioned algebraic loop 1+Dp Dc before feedback analysis. Stability means
all closed-loop poles strictly in the left half-plane (continuous) or unit circle
(discrete); a boundary pole is marginal, not stable. Controller internal dynamics
and all plant states remain in the interconnection. Nonlinear actuator limits,
sampling of a continuous model and undeclared filters are not represented.

A positive pure delay is preserved exactly in frequency response. Exact finite
closed-loop pole/stability calculation is then unavailable, not automatically
unstable. No delay approximation is silently inserted. Supplied-model v4 results
may retain a distinct optional `feedback` definition/result alongside the plant
response. Changing either model, channel selection or frequency request invalidates
saved analysis until recalculated. Controller selection and analysis never write
live parameters.

## State-space structural analysis

For a supplied continuous or discrete state-space model, Control Studio displays
A, B, C, D and the numerical controllability and observability matrices/ranks
using the existing state-space numerical module. It retains the declared state,
input and output order/units. Full rank is the model's local unconstrained
controllability/observability statement, not bounded physical reachability,
arbitrary steady targets, or useful estimation in noise. Continuous-time
stabilisability/detectability conclusions are not reused for discrete models.

For the taught cascade A=[[-.04,0],[.04,-.04]], B=[2,0]T, C=[0,1],
[B,AB]=[[2,-.08],[0,.08]] and [C;CA]=[[0,1],[.04,-.04]], both rank2.
Changing only C to[1,0] leaves controllability rank2 but observability rank1.
The matrices are calculated in the bounded Studio worker, not in lesson ST
as a substitute for analysis. Saved model evidence retains the structural result.

## Connected state-controller coefficient mapping

A project-owned loop may declare `controller.stateFeedback`. Its `family` is
`state-feedback`, `LQR` or `LQG`, and its convention is explicitly
`sampled-deviation-negative-feedback`: `du = -K dx + Nr dr`. The ordered
`gainSymbols` and `referenceGainSymbols` matrices identify the existing compiled
REAL/LREAL parameter fields; LQG additionally identifies `observerGainSymbols`.
These fields belong to the same existing parameter binding and task. Coefficient
units follow the declared input/state and input/controlled-output coordinates.
Auxiliary model outputs remain measurements, not extra controlled objectives.

Pole placement and LQR use the existing numerical design functions. Continuous
models use the existing exact-ZOH conversion at the actual task period. Explicit
pole targets are discrete poles. LQR weights may be supplied; automatic weights
normalize by each coordinate's declared operating span. LQG requires supplied
process/measurement covariance and zero direct measurement feedthrough in this
adapter. Its estimator follows the existing prediction/correction convention. The binding
also declares the ST estimator's fixed `estimatorModel` matrices A/B/C and sample
period; they must equal the selected sampled model. A newly selected plant must
not silently tune an estimator that still executes different model coefficients.

The resulting record enters the common actual-ST Verify and Manual parameter
staging operations. Pins retain explicit coefficients, recompute mapped sampled
poles, and discard any original optimality claim. A typed Auto state-transfer
adapter is still required before commissioning; this mapping alone never grants
Auto authority. The UI must offer Manual staging explicitly when that adapter is
missing. Neither a matrix calculation nor the coefficient mapping is execution
or commissioning evidence.

## Declared controller initialization and native tuning

State, ON/OFF and project-owned controller mappings may declare a
`declared-state-v1` initialization record. Each state has its compiled IEC type,
unit, bounds and either an explicit initial value or a selected-model coordinate.
The nominal actual-ST run loads these states and observes their declared bounds.
Auto uses the same initialization record through the existing task-boundary
transfer client and commissioning owner. Coupled transfer binds every actuator and its typed Manual command separately
from the controlled references. The existing task-boundary transaction stages
all states/mode together and checks each first output. The reviewed primary
output-change tolerance is converted to the same fraction of each additional
actuator's declared span; subsequent target changes use the full reviewed spans.
Commissioning sets all references in that transaction and assesses every recorded
plant-channel requirement. Recovery restores the original reference vector and
parameter record. Every actuator command and state still requires a compiled
binding; coefficient mapping alone grants no runtime authority.

Guided ON/OFF setup selects existing compiled mode, Manual command, hysteresis,
dwell and retained-state symbols. Identification uses the actuator's full binary
levels and changes timing between independent records. Verification and live
commissioning assess output levels and completed ON/OFF dwell intervals through
the shared observed-requirement evaluator. Missing complete intervals are
inconclusive.

Project-owned industrial and predictive controllers can declare a finite tuning
grid, pinned settings, a mandatory measured objective and candidate/cycle/wall
budgets. The existing isolated actual-ST service executes each frozen candidate
campaign. Selection uses only candidates passing every mandatory requirement;
it retains all attempted records and stops on program revision changes. This
bounded grid selection is not a global optimization or stability-region proof.
It does not replace family-specific analytical design where that is required.

Before an isolated Start request, the host persists the assigned job identity
and frozen request in workspace state. Reopen restores that owner and reconciles
its original runtime job; it never replays Start. Journal updates are serialized
across closing/opening panels. Manual parameter recovery likewise retains the
exact reviewed and requested records before dispatch and requires matching
runtime revision/value history before reporting adoption.

These paragraphs describe source contracts. No tests, builds, lint, type checks
or rendered verification were executed for this implementation pass.

## Connected Main workflow input and recovery ownership

The same selected model supplies Find/Supply, controller selection, Tune, Verify
and reviewed application. Automatic identification applies
`identificationPolicy.maximumNormalizedError` across low-order, frequency, state
and physical fits (default 0.15); `maximumOrder` bounds state-history fitting
(default 4, supported 1–8). Each accepted fit uses distinct estimation and
validation recordings. For low-order step fits, the declared normalized error
limit applies both to the full independent response and to the RMS prediction
error over its final ten percent of samples (at least three samples), using the
same measured output span for normalization. A good average transient fit cannot
hide an incorrect response near the end of the validation interval. This checks
the observed interval; it does not claim that the process reached steady state.
Selection remains the simplest model satisfying both limits, without ranking a
more complex passing model ahead solely for its lower error.
Unsupported or inadequate fits retain their evidence and
return to acquisition or model review.

Replacing the selected model invalidates dependent fit/analysis results.
A valid supplied model remains retained even when independence or channel
compatibility prevents recommendation. Never associate the previous model's
measured fit with its replacement. A preview is not a completed analysis
record, and cannot overwrite a restored study of the same model.

Verification uses the intersection of selected-model operating ranges and bound
signal/actuator limits. Delayed scalar native plants retain output-deviation
state, so companion initial state is converted before materialization. ON/OFF
plants begin at a declared level even when the equilibrium input lies between
levels. The state-recovery scenario perturbs initial plant state once.

Editable studies are automatically retained by `engineerDraftStorage`; task,
controller choice, pins, acquisition configuration, scenarios and existing
recordings survive close/reopen. Incomplete numeric edits cannot replace a valid
task. Storage budgets are enforced explicitly. Restored evidence never restores
an Apply approval. Parameter, isolated-job and commissioning journals own runtime
recovery separately; current durable ownership takes precedence over historical
study status. Interrupted rollback can be reviewed and retried until restoration,
recording closure and persistence all complete.

Declared controller initialization can use fresh task-boundary `model-input`,
`model-output`, `model-input-is`, integral, weighted-error and notch-history
formulas. Actuator inputs use bound applied feedback where available. Explicit
model-state coordinates remain declared nominal state. Coherent first-output
rejection prevents publication of the rejected output image. This contract does
not relax operating-point, type, unit or state-bound checks.

### Exact one-state sampled model route for classical tuning

A discrete SISO model with one state, a positive scalar transition `a`, sample
period `T`, and zero direct feedthrough may supply classical tuning through its
exact held-input continuous plant equivalent:

`A = log(a) / T`, `B = b * A / (a - 1)`, with `B = b / T` when `a = 1`.
The output coefficient and operating-point coordinates are unchanged. The
transfer function is `c * B / (s - A)`. Negative/zero sampled poles, direct
feedthrough and higher-order sampled realizations require their supported
sampled/state-controller route; no hidden modes are cancelled to force a fit.

This is an analysis adapter. The selected model stays discrete, including its
sample period, independent evidence and revision. Verify executes that original
model and actual ST. The continuous plant equivalent does not certify the
sampled closed loop or replace its mandatory verification campaign.

### Complete relay-cycle boundaries

Relay qualification starts each measured period at an observed low-to-high output
transition. Recording baseline, an initial partial high leg, and restoration are
not complete oscillation periods. Complete intervals must retain the declared
relay output levels within IEC floating-point tolerance; clipped or intermediate
output values invalidate that interval. At least three complete cycles still
must satisfy the existing amplitude and period repeatability limits.

## UI authority

The [Identify](control-studio-identify-page.md),
[Tune](control-studio-tune-page.md) and
[Validate](control-studio-validate-page.md) specifications exclusively define
page contents. The [shared flow](control-studio-complete-workflow-specification.md#three-page-automatic-flow)
defines navigation, continuation, compact editors, save and restart behavior.
