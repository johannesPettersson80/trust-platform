# Control Studio workflows 55–82

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

This specification defines the learner-facing Control Studio workflows mapped
to rows 55–82 of
[`control-course-teaching-specification.md`](../control-course-teaching-specification.md).
The mapped lesson README and the relevant content companion are the physical
authority for each workflow's case, units, parameters, limits and required
decision. This document defines how an engineer uses those cases in Control
Studio; it does not replace the lesson explanation or claim that an existing
implementation already provides every operation named here.

## Common workflow contract

Every workflow uses the shared **Identify → Tune → Validate** flow and explicit
Apply action. Controller recommendation and selection belong on Tune. The
technical model-acquisition and supply paths below feed that same flow.

“Find model” identifies an unknown plant from an informative applied-input and
measurement record, then validates the unchanged model on independent data.
“Supply model” is a separate route for a reviewed physical or teaching model;
its record retains provenance, units, revision, operating point, delay, uncertainty
and evidence class. Both routes feed the same Tune → Verify → Apply pipeline;
neither route may silently turn a supplied number into an identified result.

The recommendation must start with the simplest controller that can meet the
stated requirements under the model and measurements. Escalate only when a
specific requirement remains unmet and the next structure supplies the missing
capability. The recommendation records the reason, evidence and unresolved
risks. The engineer may override it, but the override records the selected
method, parameters, reason and expected consequence.

Tune performs numerical design or parameter calculation and displays its
inputs, equations, units, candidate settings, constraints and result. Verify executes the actual candidate ST and compares its recorded behavior
against requirements fixed before results are viewed. Independent model
validation remains a separate prerequisite where required; model-only
calculations do not substitute for controller verification. Apply is a deliberate reviewed
operation that writes an exact candidate revision; it never follows Tune or
Verify implicitly. Every operation has a manual route: the engineer can enter
the model, parameters, candidate settings, test event, requirements and
decision directly and inspect the same calculations and evidence.

Numeric values in a lesson README are benchmarks for that physical case. They
are examples for checking arithmetic and reset behavior, not global product
limits, mandatory gains, or universal model coefficients. Tune must obtain or
ask for the plant/model quantities needed by its selected method rather than
requiring the engineer to provide already-computed gains, matrices, ranks,
decouplers or observer gains.

For each operation, Studio exposes the required available commands,
measurements, setpoints, states, quality and timing with explicit unavailable
channels. It must not invent unmeasured states or an SP for an open-loop process.
ST verification uses the actual engineer program and physical interfaces;
the mapped lesson project is its teaching acceptance example. A native assertion is accepted
only when it checks the stated behavior at the project boundary; a displayed
number, catalog row, simulation truth value or metadata field is not execution
proof. A simulator result applies to its declared plant, timing and uncertainty
range. It does not establish hardware behavior. Model-only prediction,
including a number calculated in ST, never substitutes for a recorded ST
execution and an independent validation decision.

The workflow may recommend a controller only when the model and measurement
contract supports it. A finite simulation grid cannot establish global
nonlinear stability or all intermediate uncertainty values. A local linear
model cannot establish global nonlinear performance. An estimator's hidden
truth may evaluate accuracy in simulation, but it is not an available sensor
and cannot establish true-state accuracy on equipment without an independent
reference.

<a id="wf-055"></a>

## WF-055 — Feedforward control

Source: [feedforward README](../../../examples/control/09-industrial-control-structures/51-feedforward/README.md).

Engineer inputs and trigger: identify or supply the heater model, measured
disturbance and manipulated-output channels, operating point, validity/age
contract and event. The workflow starts when the engineer confirms the PI-only
or PI-plus-feedforward trial; Tune identifies the disturbance path $G_d$ and
control path $G_u$ from the model or records.

Automatic calculations/actions/results: calculate
$u_{FF}=D_m/2$ percentage points, add it to the PI request, limit and track the
complete request, and record estimate age, feedforward, P, I, requested and
applied output. On invalid or stale $D_m$, disable feedforward and align the
integral to preserve applied output; invalid PV follows the zero-heat fault
path. Tune derives a realizable feedforward map from $G_d/G_u$, including sign,
units, delay, filtering and actuator limits; it rejects a noncausal or
nonrealizable map and preserves feedback authority. Show any benchmark holding
request and measured first-second effect separately.

Manual controls: enter or override $D_m$, validity, event time, candidate gains,
tracking time and trial selection; start, abort, restore and reset explicitly.

Recommendation and escalation: recommend PI alone when no trustworthy measured
disturbance exists. Recommend feedforward only when its timing and estimate
are available and PI remains the corrective fallback. Escalate to cascade only
when an earlier measured intermediate variable is required.

ST verification and acceptance: verify sign, units, complete-request limiting,
stale-data fallback and bumpless transition in actual ST. Accept only matched
PI-only and PI-plus-feedforward records meeting minimum PV 33 °C and late error
±0.2 °C for the declared 30 kW event, with applied output 0–100%. Fail on
invalid-PV nonzero heating, stale-load continuation, output discontinuity beyond
the declared limit, or a claim that feedforward removes the one-second delay.

<a id="wf-056"></a>

## WF-056 — Cascade control

Source: [cascade README](../../../examples/control/09-industrial-control-structures/52-cascade/README.md).

Engineer inputs and trigger: identify or supply the jacketed-vessel model,
measurements, actuator path, reset and inner/outer requirements. Trigger Tune
first on the inner loop, then on the outer loop using the accepted inner-loop
model and measured response.

Automatic calculations/actions/results: Tune inner gains and verify inner
bandwidth and limits before tuning outer gains. The outer PI produces
jacket-temperature setpoint in °C; inner PI produces heater percent; each limit
and tracker uses its own units. Calculate the jacket and vessel balances,
response time to 63.2%
of the relevant step, and selected architecture. Preserve the fact that the
inner loop may react before vessel PV moves.

Manual controls: disable outer loop, choose inner setpoint, select single-loop
or cascade, edit gains and limits, start each reset event, abort and restore.

Recommendation and escalation: recommend a single loop when no measured
intermediate variable improves a stated requirement. Recommend cascade only if
the inner response is demonstrably faster and vessel requirements improve.
Escalate to split-range or override only for a cooling or constraint
requirement, not merely because two loops are available.

ST verification and acceptance: verify separate outer/inner states, units,
ownership, tracking and applied heater output. Accept only if both the 35→37 °C
peak/error and jacket-disturbance minimum/error requirements pass and the inner
response is faster. Fail if outer °C output is treated as heater percent,
inactive integral causes transfer jumps, or a cascade benefit is claimed when
the speed separation is absent.

<a id="wf-057"></a>

## WF-057 — Ratio control

Source: [ratio-control README](../../../examples/control/09-industrial-control-structures/53-ratio-control/README.md).

Engineer inputs and trigger: identify or supply the additive-pump model,
carrier/additive measurements, required ratio, flow limits, tracking time and
carrier-change/stop events. Trigger when a carrier-flow change or stop event is
confirmed; Tune designs additive PI around the measured pump path.

Automatic calculations/actions/results: calculate
$q_{a,sp}=Rq_c$, feedback error, PI request, limiting, pump lag and tail volume
$V_{tail,k+1}=V_{tail,k}+q_{a,k}dt/60$. When carrier flow is below 5 L/min or
invalid, command additive flow to zero within one update, inhibit ratio
division, track the applied command and mark ratio not evaluated. Report
steady ratio, lag, stop tail and dose. Tune exposes the selected pump model,
gains, lag and feasibility before a candidate is applied.

Manual controls: choose ratio, event profile, pump gains, tail limit, sensor
validity and re-enable; start, abort, isolate, restore and reset.

Recommendation and escalation: recommend ratio control over fixed additive flow
when carrier flow varies. Escalate to feedforward or cascade only if the pump
lag or an earlier intermediate measurement prevents the ratio requirement.

ST verification and acceptance: verify no division by zero, unit conversion,
one-update stop action, tail integration and explicit re-enable. Accept  ±.005
ratio after 10 s through 60 s, 0–20 L/min additive flow, and ≤.20 L tail dose
for the declared stop case. Fail on evaluating a ratio at zero carrier flow,
instantaneous-zero claims, or stale sensor continuation.

<a id="wf-058"></a>

## WF-058 — Split-range control

Source: [split-range README](../../../examples/control/09-industrial-control-structures/54-split-range/README.md).

Engineer inputs and trigger: identify or supply vessel heating/cooling paths,
signed net heat demand, heater/cooler scales and limits, neutral/service-
exclusion band and tracking time. Trigger separate excess-heat and heating-
demand events.

Automatic calculations/actions/results: Tune the net-demand PI, then map signed demand through
$Q=2u_h-u_c$; positive demand selects heater and zero cooler, negative demand
selects cooler and zero heater, then tracks actual net applied heat. Display
both service commands, net heat, selector state and temperature.

Manual controls: choose demand, scales, neutral band, service enable and event;
start, abort, isolate the services, restore and reset.

Recommendation and escalation: recommend one actuator when one service covers
the requirement. Escalate to split-range only when heating and cooling are both
required. Escalate to override when a separate constraint needs authority.

ST verification and acceptance: verify unequal scales, mutual exclusion,
limits, applied-net-heat tracking and transfer continuity. Accept declared
heating/cooling temperature requirements with no simultaneous service violation.
Fail if command percentages are added as if they were kW, both services are
active outside the declared band, or tracking uses requested rather than
applied net heat.

<a id="wf-059"></a>

## WF-059 — Override control

Source: [override README](../../../examples/control/09-industrial-control-structures/55-override-control/README.md).

Engineer inputs and trigger: identify or supply normal and constraint plant
paths, jacket constraint, limits and reset. Trigger when normal demand
and constraint demand compete, including the intentionally infeasible 40 °C
request.

Automatic calculations/actions/results: Tune both normal and limit controllers
against their measurements and constraints, then calculate both requests and select
$u_r=\min(u_{normal},u_{limit})$ for this heating constraint; apply one limit,
track both inactive and selected controllers, and report authority, jacket
temperature, vessel error and transfer state.

Manual controls: enable/disable override, edit requests and gains, force a
selected controller only for a reviewed test, start, abort, restore and reset.

Recommendation and escalation: recommend ordinary PI if no constraint can be
violated. Recommend override when a measured constraint needs priority.
Escalate to equipment/requirement change when the balance makes the requested
temperature incompatible with the jacket limit; do not tune away infeasibility.

ST verification and acceptance: verify selector direction, inactive tracking,
single actuator ownership and latched safety behavior. Accept jacket ≤42 °C and
the vessel requirements for feasible tests; record the 40 °C infeasibility and
recovery separately. Fail if the inactive controller causes a transfer jump,
the selector is treated as an independent protective device, or incompatible
demands are reported as tuning failure.

<a id="wf-060"></a>

## WF-060 — Smith prediction

Source: [Smith-predictor README](../../../examples/control/09-industrial-control-structures/57-smith-predictor/README.md).

Engineer inputs and trigger: identify or supply the heater model and delay
evidence, choose PI/predictor requirements, residual/fallback policy and delay
uncertainty. Trigger each PI-only or predictor trial; Tune designs both the
ordinary PI and predictor/controller arrangement.

Automatic calculations/actions/results: update delay-free $T_0$ and delayed
$T_L$ models from actual applied history and calculate
$y_c=T_0+(y_m-T_L)$. Select measured-feedback fallback on invalid history or
residual threshold, preload integral at transfer, and expose both states,
correction, fallback and requested/applied output. Tune checks realizability
from command history, sample interval and computation budget, then compares it
with ordinary PI. Do not claim heat arrives sooner or that the predictor knows
an unseen disturbance.

Manual controls: choose model delay, residual threshold, predictor/fallback,
PI gains, event, start, abort and reset.

Recommendation and escalation: recommend ordinary PI if delay is negligible or
model confidence is inadequate. Recommend Smith prediction only for known delay
with a validated model. Escalate to robust or fallback design when mismatch
exceeds the declared threshold.

ST verification and acceptance: verify update ordering from previous applied
command, nominal 35→38 and 30 kW trials, mismatch fallback and invalid-PV
zero-heat path. Accept only per-event peak/minimum and late-error requirements
with full resets. Fail on future-state comparison, requested-history use,
healthy-disturbance misclassification or fallback discontinuity.

<a id="wf-061"></a>

## WF-061 — Vessel commissioning

Source: [vessel-commissioning README](../../../examples/control/14-integrated-capstones/83-reactor-control/README.md).

Engineer inputs and trigger: choose two suitable taught arrangements for the
jacketed vessel, requirements, actuator ownership, lifecycle rules and optional
cooling/constraint cases. Tune generates gains for each arrangement; trigger
each frozen candidate from the same physical reset.

Automatic calculations/actions/results: check heat feasibility, run matched
target/load tests, compare vessel/jacket temperatures, services, state,
validity and recovery, and produce requirement-by-requirement candidate
results. Do not add every structure by default.

Manual controls: choose plant, candidates, requirements, mode/invalid-data
case, event and safe restoration; exact gains remain an explicit override.

Recommendation and escalation: recommend the simplest arrangement meeting all
requirements. Escalate only for measured jacket disturbance, cooling need or
jacket constraint; report none if no candidate qualifies.

ST verification and acceptance: verify one actuator owner, initialization,
manual/automatic transfer, tracking, limits and fault latch. Accept only both
baseline events plus selected extensions and lifecycle case; fail on smoothness
as sole criterion, untested extension, or unsupported physical claim.

<a id="wf-062"></a>

## WF-062 — Multivariable process paths

Source: [MIMO paths README](../../../examples/control/10-multivariable-control/61-mimo-paths/README.md).

Engineer inputs and trigger: identify or supply the two-vessel model, input and
output channels, operating point, step design, limits and validation tolerance.
Trigger one actuator step at a time with the other fixed; for an $m\times n$
plant, Tune designs all $m n$ path experiments and their excitation order.

Automatic calculations/actions/results: calculate the complete $m\times n$
steady/dynamic path map and predicted outputs; record applied histories,
windows and gain estimates with units. Do not assume a square or symmetric
matrix.

Manual controls: select heater, step, hold, reset, tolerance, bounds and abort.

Recommendation and escalation: recommend independent pairing only after all
four paths are measured. Escalate to RGA when the other feedback loop will move
its actuator; do not infer decoupling from one path.

ST verification and acceptance: verify both old-state derivatives, opposite
coupling flow, simultaneous next-state storage and PI-off operation. Accept all
four paths within ±.02 °C under the declared resets. Fail on missing cross path,
partial final window, clipped state or identification claim beyond the supplied
local model.

<a id="wf-063"></a>

## WF-063 — Relative gain array

Source: [RGA README](../../../examples/control/10-multivariable-control/62-relative-gain-array/README.md).

Engineer inputs and trigger: identify or supply a validated gain map, input and
output dimensions, pairing candidates, isolated/held-output experiments,
uncertainty and reset. Trigger the fixed-output and held-output comparisons.

Automatic calculations/actions/results: for square nonsingular static maps,
calculate compensated gains using the inverse and
$\Lambda=K\circ(K^{-1})^T$; for singular or rectangular maps, report that a
right/left pseudoinverse, pairing restriction or dynamic method is required.
For integrating or strongly dynamic paths, reject static RGA as sufficient and
route to a dynamic interaction design. Show condition/rank, units, signs and
pairing recommendation. The result is steady pairing evidence, not PI tuning.

Manual controls: select matrix, pairing, fixed compensation, steps, event,
reset and report uncertainty.

Recommendation and escalation: recommend a near-one positive pairing when the
steady comparison supports it. Escalate to decentralized PI and transient tests
for interaction; escalate to decoupling only when a static map addresses a
stated problem.

ST verification and acceptance: verify scalar inverse/elementwise products,
units and guarded ratios. Accept measured isolated/compensated values within
declared tolerance and report uncertainty propagation. Fail on matrix versus
scalar multiplication, ratio/tolerance confusion, or transient qualification
from fixed compensation alone.

<a id="wf-064"></a>

## WF-064 — Decentralized PI

Source: [decentralized-PI README](../../../examples/control/10-multivariable-control/63-decentralized-pi/README.md).

Engineer inputs and trigger: choose validated pairing, two loop requirements,
sampling/tracking policy, reset, target directions and load events. Tune both
PI controllers together under the interaction model, then trigger all matched
event classes for each frozen gain set.

Automatic calculations/actions/results: coordinate tuning of the separate
controllers, then calculate separate P/trial-I/request,
limit and tracking for each loop; record changed and unchanged vessel metrics,
load recovery and clipping. Retune only within declared bounded comparison.

Manual controls: choose pairing/gains, target/load direction, reset, event and
abort.

Recommendation and escalation: recommend decentralized PI after all interaction
requirements pass. Escalate to static decoupling when a known cross effect
requires coordination; do not call two independent loops decoupled.

ST verification and acceptance: verify separate retained states and one scalar
controller call per vessel. Accept both target directions and both load tests,
including unchanged-vessel ±.2 °C and loaded-vessel requirements. Fail on a
single-direction pass, state sharing, output clipping outside bounds or missing
cross-effect evidence.

<a id="wf-065"></a>

## WF-065 — Static decoupling

Source: [decoupling README](../../../examples/control/10-multivariable-control/64-decoupling/README.md).

Engineer inputs and trigger: identify or supply the interaction model and
requirements, then choose virtual coordinates and physical limits. Trigger
virtual requests and compare paired PI with mapped physical commands.

Automatic calculations/actions/results: calculate $D$ automatically from the
validated interaction map and chosen virtual coordinates, then calculate
$u=Dv$, add the operating bias, limit
in physical coordinates, map physical tracking error through $D^{-1}$, and
report virtual/physical requests, applied outputs, clipping and cross error.

Manual controls: enter matrix, virtual request, gains, event and candidate
selection; start, abort, restore and reset.

Recommendation and escalation: recommend paired PI when static cancellation has
no requirement benefit. Escalate to decoupling only when known cross effects
justify the map; escalate to robust MIMO selection when coupling is uncertain.

ST verification and acceptance: verify matrix products, virtual state
initialization, physical limits and coordinate tracking. Accept only if every
WF-064 requirement passes with feasible outputs. Fail on treating static inverse
as dynamic cancellation, absolute-bias mapping, or ignored clipping.

<a id="wf-066"></a>

## WF-066 — Multivariable design selection

Source: [multivariable design README](../../../examples/control/10-multivariable-control/65-robust-mimo-selection/README.md).

Engineer inputs and trigger: identify/supply uncertainty and requirements,
generate paired and decoupled architecture candidates, and freeze each before
verification. Trigger all declared architecture/uncertainty/event records.

Automatic calculations/actions/results: generate and Tune frozen architecture
candidates, calculate feasibility for each uncertainty case, run candidates,
evaluate every requirement and create a case ledger. Report tested-grid scope;
do not interpolate it into a global robustness claim.

Manual controls: choose architectures, coupling cases, event order, reset,
stop and report decision.

Recommendation and escalation: recommend the simplest architecture passing all
cases. Escalate only to a structure that addresses the failed requirement;
report infeasible capacity separately from controller failure.

ST verification and acceptance: verify coupling parameter is physical and map
revision remains fixed, with limits/fault behavior preserved. Accept only all
feasible declared cases. Fail on retuning per case, omitted direction, or
claiming values between the tested grid without a bound.

<a id="wf-067"></a>

## WF-067 — State-space models

Source: [state-space README](../../../examples/control/02-derivative-action-and-dynamic-response/20-state-equations-from-stores/README.md).

Engineer inputs and trigger: import the two-tank local model, state/input/output
order, initial h1=.27 m/h2=.25 m, q=.010 m³/s, flow deviation cases and 1 s
interval. Trigger model analysis before the pulse comparison.

Automatic calculations/actions/results: display $A,B,C,D$, state order, model
forecast and physical measured levels; run zero and +.001 m³/s deviations and
compare derivatives and recovery. Do not replace measured nonlinear levels
with model values.

Manual controls: import model, choose flow deviation, run/reset, inspect state
and stop.

Recommendation and escalation: recommend the simplest scalar model only when
one state predicts the requirement. Escalate to state-space when connected
storage or multiple measurements require internal states.

ST verification and acceptance: verify both old state entries, one-second
timing, no clipping of truth/model and fault/clock handling. Accept matching
initial derivative signs and declared model error through the pulse. Fail on
one-level substitution, unsynchronized reset or nonlinear/global claims.

<a id="wf-068"></a>

## WF-068 — Controllability

Source: [controllability README](../../../examples/control/11-state-space-estimation-and-predictive-control/66-controllability/README.md).

Engineer inputs and trigger: supply $A,B$, state/input order, bounded pulse
design and full reset. Trigger model analysis then bounded physical pulses.

Automatic calculations/actions/results: for $n$ states calculate
$\mathcal C=[B,AB,\ldots,A^{n-1}B]$, numerical rank, controllability of unstable
modes and stabilizability; display direct/propagated columns and observed
levels. The engineer supplies the model and objective, not a precomputed rank.
Separate unconstrained local controllability from steady feasibility.

Manual controls: import model, choose pulse sign/duration, inspect rank, start,
abort and reset.

Recommendation and escalation: recommend ordinary output feedback only when
available measurements suffice; escalate to state feedback/observer when
independent state influence or measurement is required.

ST verification and acceptance: verify pulse interface, bounds and clock.
Accept the declared numerical rank with tolerance/conditioning and local
scope; determinant is only an illustrative square-matrix check. Fail if rank is
used to claim arbitrary steady levels, actuator authority outside limits or
global nonlinear controllability.

<a id="wf-069"></a>

## WF-069 — Observability

Source: [observability README](../../../examples/control/11-state-space-estimation-and-predictive-control/67-observability/README.md).

Engineer inputs and trigger: supply $C$, $A$, measured downstream level,
initial estimate/uncertainty, clean/noisy records and validation metrics.
A true hidden initial state is a simulation verification input only. Trigger model
analysis then matched clean/noisy observations.

Automatic calculations/actions/results: for $n$ states calculate the stacked
observability matrix $\mathcal O=[C;CA;\ldots;CA^{n-1}]$, numerical rank,
whether all unobservable modes are stable (detectability) and diagnostic reconstruction;
display measured history,
truth-only evaluation, RMS and peak error. Do not call noisy differentiation an
estimator.

Manual controls: choose noise sequence, window, model import, validation
requirements, reset and stop.

Recommendation and escalation: recommend direct measurement when available;
escalate to an observer/Kalman filter when a hidden state is needed and noise
makes differentiation inadequate.

ST verification and acceptance: verify first-sample priming, no initial truth
seeding, invalid/stale handling and separate clean/noisy records. Accept rank
and clean reconstruction evidence; report noisy degradation. Fail on hidden
truth as controller input or on observability as accuracy proof.

<a id="wf-070"></a>

## WF-070 — State feedback

Source: [state-feedback README](../../../examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/README.md).

Engineer inputs and trigger: supply local $A,B$, measured states, desired
closed-loop objectives or poles, reset and limits. Trigger at the confirmed
initial state; Tune computes $K$ by pole placement or an equivalent stated
design method.

Automatic calculations/actions/results: compute $K$ from the objectives/poles,
then calculate $A-BK$ or $A_d-B_dK$ in the selected domain, the characteristic polynomial, requested/applied flow and
local model prediction. Record nonlinear
measured levels separately and report clipping.

Manual controls: edit poles or the explicit gain matrix K, inspect state order/units, enable/disable,
choose reset and abort.

Recommendation and escalation: recommend state feedback only with all required states
measured or credibly estimated. Escalate to observer/Kalman when measurement
availability requires it; retain output feedback if state evidence is absent.

ST verification and acceptance: verify row products, q offset, limits and
truth/model separation. Accept only local late-error requirement without
clipping. Fail on global nonlinear stability or gain validity outside the local
model.

<a id="wf-071"></a>

## WF-071 — State observers

Source: [state-observer README](../../../examples/control/11-state-space-estimation-and-predictive-control/70-state-observer/README.md).

Engineer inputs and trigger: supply $A,B,C$, measurement/noise contract,
initial estimated state, applied flow and clean/noisy cases. Trigger observer
design after a valid model and measurement are available; Tune computes $L$
from poles or an explicit noise/performance tradeoff.

Automatic calculations/actions/results: compute $L$ from the selected observer
objectives, then calculate prediction, innovation and
$\dot{\hat x}=A\hat x+Bv+L(y-C\hat x)$; expose estimates, truth-only error,
innovation, bounds and fallback. The observer must use applied input.

Manual controls: select noise, gain, reset, run, abort and validation windows.

Recommendation and escalation: recommend an observer when a model and usable
measurement support hidden-state estimation. Escalate to Kalman when uncertainty
weights should set correction; use direct measurement if it is sufficient.

ST verification and acceptance: verify old-state ordering, first zero innovation,
no truth seeding, stale-data stop and clean/noisy metrics. Accept declared clean
error requirement and report noisy errors; fail on true-state controller input
or claiming accuracy without reference truth.

<a id="wf-072"></a>

## WF-072 — Kalman filtering

Source: [Kalman-filter README](../../../examples/control/11-state-space-estimation-and-predictive-control/71-kalman-filter/README.md).

Engineer inputs and trigger: supply or estimate sampled $A_d,B_d,C$, initial
estimate/covariance, clock, measurement records and uncertainty evidence.
Trigger prediction/correction per accepted measurement; Tune estimates or
accepts justified $Q/R$ and then tunes/validates the filter.

Automatic calculations/actions/results: calculate $\hat x^-$, $P^-$, innovation,
$S$, gain and $\hat x^+,P^+$; retain applied input and covariance; compare
clean/noisy RMSE and peak error against hidden simulation truth only.

The prediction/correction calculation must expose
x_pred=Ad*x+Bd*u_previous, P_pred=Ad*P*Ad^T+Q,
innovation=y-C*x_pred, S=C*P_pred*C^T+R,
K=P_pred*C^T*S^(-1), and x_corrected=x_pred+K*innovation.
Use a numerically appropriate solve and covariance update, such as Joseph form
P=(I-KC)*P_pred*(I-KC)^T+K*R*K^T. Check dimensions, covariance units,
symmetry/positive-semidefiniteness and singular innovation cases. Distinguish
continuous noise intensity from covariance per discrete update.

Manual controls: edit declared covariance, noise sequence, reset, quality and
validation windows; freeze/restart explicitly.

Recommendation and escalation: compare the simplest suitable measurement
filter/observer/estimator against the stated task. Kalman design needs justified
model and second-moment noise assumptions, not necessarily Gaussian noise.
If Q/R cannot be distinguished from model error or disturbances, retain the
ambiguity and request useful evidence or explicit assumptions; do not claim a
deterministic observer is automatically more credible. Invalid/outlier data
requires the corresponding validity/fault policy.

ST verification and acceptance: verify all covariance entries from old predicted
matrix, invalid/stale freeze/fallback and no truth input. Accept arithmetic and
matched RMSE comparison. Linear minimum-variance design does not require a
Gaussian distribution; Gaussian assumptions support stronger distributional or
optimality claims, which require evidence. Fail on covariance presented as
actual error or an unsupported Gaussian-optimal claim.

<a id="wf-073"></a>

## WF-073 — LQR control

Source: [LQR README](../../../examples/control/11-state-space-estimation-and-predictive-control/69-lqr/README.md).

Engineer inputs and trigger: supply or identify the sampled tank model,
performance/effort scales, horizon policy, terminal cost, initial state and
limits. Tune automatically scales physical weights, computes one-step and
finite/infinite-horizon Riccati candidates, and freezes the selected candidate
before each verification run.

Automatic calculations/actions/results: calculate one-step gain and Riccati
recursion, report fixed $K_0$ versus any time-varying $K_k$ sequence, predicted
cost, requested/applied flow, clipping and late state error. Distinguish design
cost from measured energy or wear.

For discrete finite-horizon design, expose terminal P_N and backward recursion
G_k=R+B^T*P_(k+1)*B, K_k=solve(G_k,B^T*P_(k+1)*A),
P_k=Q+A^T*P_(k+1)*A-A^T*P_(k+1)*B*K_k.
An infinite-horizon mode solves the applicable algebraic Riccati equation.
Retain whether the runtime uses a time-varying finite sequence, a repeated
first finite-horizon gain, or a steady-state gain; these are distinct policies.

Manual controls: choose Q/R/terminal weights, horizon, solver mode, manual K override, model revision, candidate,
reset, abort and comparison metrics.

Recommendation and escalation: recommend fixed state feedback when the stated
tradeoff is simple and requirements pass. Escalate to MPC only for future
constraints/shared resources; use observer/Kalman when state measurements are
missing.

ST verification and acceptance: verify finite-horizon gain application, actual
applied flow history, limits and separate cost signals. Accept both fixed-R
records against the late error requirement; fail on claiming infinite-horizon optimality for a finite-horizon benchmark
or hiding clipping from the cost/verification. An independently solved infinite-
horizon mode must check its stabilizability/detectability and solver conditions.

<a id="wf-074"></a>

## WF-074 — LQG control

Source: [LQG README](../../../examples/control/11-state-space-estimation-and-predictive-control/72-lqg/README.md).

Engineer inputs and trigger: supply or identify plant/measurement models,
control objectives, uncertainty evidence, initial estimated state, flow and
reference policy. Tune synthesizes the estimator and controller, then triggers
prediction, correction and connected estimated-state feedback.

Automatic calculations/actions/results: synthesize LQR and estimator gains,
check their connected update order and stability/constraint assumptions, then
calculate applied-input prediction,
Kalman correction, $v=-K\hat x^+$, limits and compare level error, estimation
error and pump deviation with full-state reference. Truth is evaluation only.

Manual controls: choose reference/estimated mode, noise/covariance, reset,
fault case, requirements and stop.

Recommendation and escalation: recommend full-state feedback when all required
states are measured. Escalate to LQG when estimator assumptions and sensor
availability justify it; escalate to MPC only for future shared constraints.

ST verification and acceptance: verify previous applied input, update order,
stale-data recovery and separate truth/reference paths. Accept fixed late-error
and bound results plus estimator metrics. Linear minimum-variance evidence may
use non-Gaussian or deterministic records; stronger Gaussian-distribution or
Gaussian-optimality claims require the corresponding noise assumptions and
evidence. Fail on hidden truth input.

<a id="wf-075"></a>

## WF-075 — Model predictive control

Source: [MPC README](../../../examples/control/11-state-space-estimation-and-predictive-control/73-mpc/README.md).

Engineer inputs and trigger: select a supplied or identified supported model, performance/effort objectives,
constraints, solver/computation budget, fallback and reset.
Tune chooses horizon and weights and either invokes a bounded solver or a
declared finite enumeration; trigger replanning at each new measurement.

Automatic calculations/actions/results: enumerate or solve the configured
candidate plans over the selected prediction/control horizon, preserving
previous applied-command delay; reject infeasible plans and score the declared
tracking, effort/move and terminal cost J,
select the least-cost feasible plan, apply only its first move and retain
fallback PI states. Report rejected plans, selected plan, cost, applied command
and planning failure.

Manual controls: choose grid/horizon/cost/priority within reviewed bounds,
override candidate selection, inspect plans, abort and reset.

Recommendation and escalation: recommend independent PI when shared constraints
are absent. Escalate to MPC only when prediction and resource constraints are
required; use the specified priority fallback on planning failure.

ST verification and acceptance: verify the configured solver/enumeration,
previous applied
delay, feasibility, first-move-only action, replan and invalid-PV zero output.
For the lesson benchmark, compare the231 permitted two-heater command pairs
with three prediction steps and move blocking, and verify the60s temperature/
power requirements. Those dimensions are not product limits. Test hard versus
explicitly soft constraints, infeasible initial conditions, solver failure and
deadline fallback. Fail a claimed global optimum from a finite grid or local
nonlinear solve, an ignored constraint, or a plan violating the supply sum.

<a id="wf-076"></a>

## WF-076 — Lyapunov stability

Source: [Lyapunov README](../../../examples/control/12-nonlinear-adaptive-and-robust-control/74-lyapunov-stability/README.md).

Engineer inputs and trigger: select a supported nonlinear/linear model and equilibrium,
actuator limits and region objective. Tune proposes a supported positive
candidate $V$ and certificate region; trigger analytic certificate calculation
before sampled runs.

Automatic calculations/actions/results: calculate $V$, analytically derive
$\dot V=\nabla V\,f(x)$, and verify positivity, decay/invariance, torque bounds
and certificate region. A sampled derivative estimate is diagnostic only and
never proof. Simulate inside/outside cases and report proof applicability
separately from trajectory.

Manual controls: choose initial state, timestep comparison, certificate,
stop/reset and safe torque.

Recommendation and escalation: recommend PD when the certified region meets
the requirement. Escalate to feedback linearization for nonzero-target gravity
compensation; use robust analysis for model uncertainty.

ST verification and acceptance: verify radians, sine/cosine, torque limits,
invalid timing and no outside-region classification. The upright-arm benchmark must establish the stated ideal continuous,
unsaturated V<=.1 region. General supported certificates retain their own
model, equilibrium, positivity/decay or invariance conditions, actuator bounds
and domain; a failed certificate search is not a proof of instability. Fail on global nonlinear
stability, sampled equivalence or outside-certificate instability claims.

<a id="wf-077"></a>

## WF-077 — Feedback linearization

Source: [feedback-linearization README](../../../examples/control/12-nonlinear-adaptive-and-robust-control/75-feedback-linearization/README.md).

Engineer inputs and trigger: supply the arm model, target, measurements,
actuator limits and model uncertainty. Tune derives gravity compensation from
the supplied model, checks relative degree/zero dynamics, realizability and
input limits, then triggers each full-reset compensation trial.

Automatic calculations/actions/results: calculate P, D, model-derived
$-\hat G\sin\phi$,
requested/applied torque and residual $(G-\hat G)\sin\phi$; evaluate peak,
late error and torque activity. Do not substitute tangent for sine.

Manual controls: select coefficient/noise, target, reset, start, abort and
fallback.

Recommendation and escalation: recommend ordinary PD when gravity residual is
acceptable. Escalate to feedback linearization only when known model gravity
causes a stated error; escalate to adaptive/robust methods for coefficient
change or uncertainty.

ST verification and acceptance: verify measured-state use, sign, limits and
invalid-data zero torque. Accept nominal/imperfect coefficient cases against
peak/error requirements; fail on true-angle controller input, saturation-hidden
comparison or exact-cancellation claim under coefficient error.

<a id="wf-078"></a>

## WF-078 — Adaptive control

Source: [adaptive-control README](../../../examples/control/12-nonlinear-adaptive-and-robust-control/76-adaptive-control/README.md).

Engineer inputs and trigger: identify or supply the heater balance, known
quantities and an informative record, adoption bounds, residual/split-window
criteria and atomic update boundary. Trigger estimation only after a completed
valid record.

Automatic calculations/actions/results: calculate centered derivative, delayed
balance $y=au-C\dot T$, least-squares $\hat H=\sum xy/\sum x^2$, residual and
split-window agreement; reject unqualified estimates, independently validate
the unchanged model, then atomically adopt the estimate, update gains and align
I. Compare fixed/revised gains on both standard trials.

Manual controls: choose record/window, bounds, candidate, update approval,
event, reset and reject/retain decision.

Recommendation and escalation: recommend fixed gains when data cannot support
an estimate. Escalate to adaptation only after excitation and identifiability
criteria pass; escalate to robust design when uncertainty remains bounded but
unresolved.

ST verification and acceptance: verify centered windows avoid delay edges,
positive denominator, D=0, atomic state continuity and no whole-integral
rescale. Accept only H criteria plus matched target/load performance. Fail on
unknown D/sensor bias confounding, live unavailable centered derivative or
automatic update after failed qualification.

<a id="wf-079"></a>

## WF-079 — Robust control

Source: [robust-control README](../../../examples/control/12-nonlinear-adaptive-and-robust-control/77-robust-control/README.md).

Engineer inputs and trigger: identify or supply a coherent uncertainty set,
feasibility equations, model revision and requirements. Tune first generates candidates in the simplest applicable controller family
(PI for the heater benchmark) within the uncertainty set, freezes each candidate, then triggers
the declared independently reset cases.

Automatic calculations/actions/results: check $u_{ss}=[H(T-20)+D]/2$,
calculate declared small-gain bound where supplied, run every grid case,
evaluate performance and clipping, and report robust-stability and
robust-performance results separately.

Manual controls: choose fixed candidate, model case, event, reset and evidence
ledger; do not retune per case after freezing.

Recommendation and escalation: recommend the simplest controller that meets
the required uncertainty-domain conditions. Robust tuning may retain the same
PI/PD structure; it is not automatically a more complex controller family.
Only escalate structure when simpler candidates cannot qualify. A finite grid
cannot prove all intermediate values without a mathematical bound.

For supported multiplicative uncertainty, expose the nominally internally
stable loop, its uncertainty weight W and sensitivity T; an applicable
small-gain certificate checks ||W*T||_infinity<1 under its declared perturbation
assumptions. MIMO uses the appropriate induced norm. Frequency samples alone
are not a continuum certificate. Keep certified bounds and scenario tests
separate, and preserve physical parameter correlations when specified.

ST verification and acceptance: verify H changes K and τ coherently, L affects
delay only, candidate remains fixed, and limits/fault paths persist. Accept all
declared feasible grid cases and separately document any continuous bound.
Fail on an unjustified uncertainty representation, retuning, or global
nonlinear claims. Correlated uncertainty may be used when physically justified;
independent variation is allowed when the plant contract justifies it.

<a id="wf-080"></a>

## WF-080 — Networked control

Source: [networked-control README](../../../examples/control/13-networked-fault-aware-and-diagnostic-control/78-networked-control/README.md).

Engineer inputs and trigger: define the remote/local requirements, clock
topology and uncertainty, epoch/sequence contract, timestamp evidence, hold
policy, local fallback and recovery acknowledgement. Tune proposes age,
timeout and transfer policy from those requirements; lesson values are
benchmarks, not universal limits. Trigger normal, drop, delay, reorder,
old-epoch and blocked-packet cases.

Automatic calculations/actions/results: calculate freshness with clock-offset
and uncertainty bounds when clocks are not shared; accept only newer
current-epoch packets,
reject stale/reordered packets, hold last command until timeout, transfer to
local PI with applied-output tracking, require acknowledgement and three fresh
packets before remote return, and expose network versus physical delay.

Manual controls: choose packet fault, epoch, acknowledgement, local/remote
mode, reset and abort.

Recommendation and escalation: recommend local PI when the network is not
required. Escalate to remote mode only with freshness/order/timeout and local
fallback contracts; escalate to fault-aware sensing for measurement faults.

ST verification and acceptance: verify sequence zero initialization, epoch
separation, age/order boundaries, .3 s fallback, bumpless transfer and no old
command restart. Accept normal thermal requirements and each packet policy.
Fail on packet age confused with heater delay, repeated packet counting, or
healthy-PV fallback commanding zero heat contrary to policy.

<a id="wf-081"></a>

## WF-081 — Fault-aware control

Source: [fault-aware README](../../../examples/control/13-networked-fault-aware-and-diagnostic-control/79-fault-aware-control/README.md).

Engineer inputs and trigger: define sensor quality, range/age evidence,
reference availability, disagreement policy, recovery acquisitions and Auto
edge. Tune proposes thresholds/timeouts from measurement uncertainty and safety
requirements; disagreement alone does not prove either sensor bad. Trigger
normal, stale, range, fresh-disagreement, both-invalid and recovery cases.

Automatic calculations/actions/results: classify each channel and the evidence
available for truth, select one valid
sensor with $I=u_a-P$ alignment, latch zero heat on fresh disagreement or no
valid sensor, require three fresh agreeing acquisitions and deliberate Auto
edge, and report timer, selected source, mode and continuity.

Manual controls: inject each fault, acknowledge, choose recovery edge, reset,
abort and inspect both channels.

Recommendation and escalation: recommend primary/monitor sensing when both
agree. Escalate to single-sensor fallback only for a clearly invalid/stale
channel; escalate to latched zero heat on unresolved disagreement. Never
recommend a naive higher/lower vote without a reference.

ST verification and acceptance: verify age/timestamp continuity, sustained timer
reset, disagreement priority, bumpless single-sensor transfer and three-sample
recovery. Accept normal thermal tests and fault-policy transitions separately.
Fail on model truth vote, disagreement auto-selection, missing latch or missing
new Auto edge.

<a id="wf-082"></a>

## WF-082 — Independent design

Source: [independent-design README](../../../examples/control/14-integrated-capstones/84-independent/README.md).

Engineer inputs and trigger: choose one taught plant, two meaningful taught
candidate controllers, one supported uncertainty, nominal/setpoint/load cases,
limits, sampling, validity, reset, metrics and a decision rule before results.
Trigger each candidate and uncertainty case from full reset.

Automatic calculations/actions/results: calculate plant feasibility, candidate
first commands, model residuals, requirement metrics and uncertainty ledger;
compare candidates without changing thresholds and produce qualified, none-
qualify or redesign result. Keep learner choices separate from outcomes.

Manual controls: choose plant, candidates, uncertainty, requirements, event,
mode/fault case, reset, stop and recommendation override.

Recommendation and escalation: recommend the simplest candidate meeting every
declared requirement. Escalate only when a failed requirement maps to a taught
additional method; if neither qualifies, recommend redesign or bounded manual
operation.

ST verification and acceptance: verify units, state ownership, validity,
limits, requested/applied values, reset and selected fallback in both
integrations. Accept only requirement-by-requirement nominal and uncertainty
evidence. Fail on copied settings without physical justification, threshold
changes after results, simulator-as-hardware claims, or global stability claims
from finite runs.

## Scope and conflict record

The 28 source-map links above were checked against the current 82-row map and
the complete mapped READMEs were read in bounded batches. Existing lesson
implementations and workflow metadata may expose only part of these contracts;
that is an implementation gap, not permission to weaken the workflow. In
particular, numerical ST calculations remain implementation/model support and
cannot substitute for Control Studio records, comparisons, validation or
manual review. No code, lesson README, native test or metadata file is changed
by this specification.
