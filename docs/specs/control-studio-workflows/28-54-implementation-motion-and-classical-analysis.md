# Control Studio workflows 28–54: implementation, motion and classical analysis

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

Status: required behavior specification, 2026-09-07.

This specification defines the required Studio behavior for authoritative
lessons 29–60 in docs/specs/control-course-teaching-specification.md. The
mapped READMEs and companion documents were read in full. A runnable project
or current UI may be incomplete; that is an implementation gap and remains
visible in the result. ST arithmetic, a native trace, or a screenshot cannot
substitute for a named Studio analysis.

Every workflow uses this order: find or supply a typed model and source record;
qualify it; recommend the simplest adequate controller or analysis; let the
learner choose or override; calculate or tune; verify declared cases; and
apply only after review. Results retain source, units, reset, timing,
parameters, uncertainty, evidence class, status and failure reason.

<a id="wf-028"></a>

## WF-028 — Measurement validity

Source: [Lesson 29 README](../../../examples/control/03-production-quality-digital-pid/42-invalid-stale-data/README.md).

Trigger/inputs: PV, quality, acquisition and controller timestamps, age and
interval limits, finite/range limits, recovery count and explicit enable edge.
The learner can inject bad quality, nonfinite/out-of-range PV, old/repeated
timestamps, negative age and overlong intervals.

Automatic result: calculate age, validity, fault latch, frozen I/filter state,
declared fallback, recovery count and readiness. For the heater benchmark,
recommend frozen state and zero heat with fresh samples plus deliberate enable;
a general loop uses its reviewed safe-output policy. Manual controls edit
validity limits, recovery conditions, acknowledgement and fault-test cases. Verify 0.7 s fails a 0.5 s
limit, old data does not count, three new samples recover, and a bad sample
resets. Accept only with fault priority and frozen state; fail if stale data
updates state or held Auto resumes.

<a id="wf-029"></a>

## WF-029 — Heater commissioning

Source: [Lesson 30 README](../../../examples/control/14-integrated-capstones/81-water-heater/README.md).

Trigger/inputs: validated heater model, gains, beta/filter, delay/history,
limits, modes and lifecycle policy; separate 35→38 °C and 30 kW-load trials.
Automatic result: calculate u_hold=(H(T-Ta)+D)/a, thermal windows, limits and
lifecycle verdicts. Recommend the simplest candidate passing both cases;
escalate capacity or model uncertainty when no candidate passes.

Manual controls: declare revision and requirements, reset to 35 °C/75%, run
each thermal case separately, then initialization, transfer, revision and
fault recovery. Accept timestamped units for every row; fail nominal 90% used
as measured proof, combined tests, or untested lifecycle behavior.

<a id="wf-030"></a>

## WF-030 — Actuator tracking

Source: [Lesson 31 README](../../../examples/control/01-feedback-foundations-and-temperature-control/08-heating-valve-rate-tracking/README.md).

Trigger/inputs: actuator model, measured stem, target, flow, temperature, PI
state, rate/lag/delay limits, reset and declared recovery requirements.
Automatically fit or select a tracking time Tt and rate model from the
actuator record, calculate bounded movement, trial integral, request and
I_next=I_trial+dt/Tt(x-u_r), and recommend the simplest tracking policy that
meets the thermal and actuator requirements. Expose candidate Tt, rate and
model uncertainty; escalate when no candidate separates rate windup from
saturation. Manual controls choose/override Tt, reset, target sequence and
tracking on/off, then compare matched runs. Accept only actual-stem evidence,
bounded movement and declared recovery; fail request-as-position or instant
zero-flow claims.

<a id="wf-031"></a>

## WF-031 — Valve characteristics

Source: [Lesson 32 README](../../../examples/control/09-industrial-control-structures/58-valve-characterization/README.md).

Trigger/inputs: valve/pipe model or measured calibration records, pressure
arrangement, actual stem, pressure, flow, temperature, bounded probes, rate,
lag, delay and steady-window requirements. Automatically plan an informative
staircase, wait for arrival/final-3 s range, fit the installed map and local
heat relation with uncertainty, and recommend the simplest adequate map.
Escalate to another pressure arrangement or experiment when residuals or
coverage are inadequate. Manual controls choose/override targets, pressure
mode, hold duration and acceptance window. Accept qualified actual
pressure/stem/flow fits; fail request-as-flow, transient-only calibration or
unreported pressure dependence.

<a id="wf-032"></a>

## WF-032 — Valve stiction

Source: [Lesson 33 README](../../../examples/control/09-industrial-control-structures/59-valve-stiction/README.md).

Trigger/inputs: valve/positioner model or measured response, request, accepted
target, stem, force/drive estimate, flow, temperature and pressure. Automatically
plan bounded increasing, decreasing and reversal probes, estimate command
deadband and breakaway/moving-friction ranges with uncertainty, and classify
ignored target versus accepted-but-stuck stem. Recommend path repair or
mechanical investigation only after discrimination; do not require a known
breakaway threshold. Manual controls choose probe amplitude, direction,
duration and override candidate classification. Accept only with accepted
target, stem and timing evidence; fail flow-only diagnosis or a probe that
cannot distinguish the causes.

<a id="wf-033"></a>

## WF-033 — Position control

Source: [Lesson 34 README](../../../examples/control/02-derivative-action-and-dynamic-response/32-proportional-position-stiffness/README.md).

Trigger/inputs: shaft model, angle, velocity, target, J/B, motion requirements,
limits and sample period. Automatically calculate position-loop gain needs
from peak, settling and crossing requirements, search admissible Kp and
choose a gain by the declared response, robustness and effort criteria; escalate to PD when no P candidate
passes. Manual controls choose/override Kp, target, reset and duration.
Verify J omega-dot=u-B omega, theta-dot=omega and velocity at first crossing
on matched resets. Accept declared peak/error/torque limits; fail first arrival
called settled or a visually fast torque violation.

<a id="wf-034"></a>

## WF-034 — PD control

Source: [Lesson 35 README](../../../examples/control/02-derivative-action-and-dynamic-response/33-derivative-motion-damping/README.md).

Trigger/inputs: shaft model, Kp candidate, measured velocity, motion/torque
requirements, noise and sample period. Automatically calculate damping need,
search Kd and derivative-on-measurement versus error alternatives, and
recommend the simplest Kd/filter combination that passes peak, settling,
noise and torque. Manual controls choose/override Kd, derivative source,
target and reset. Verify u=Kp(r-theta)-Kd omega, matched P/PD tests and a
synthetic kick. Fail continuous damping algebra used as sampled qualification.

<a id="wf-035"></a>

## WF-035 — PID load rejection

Source: [Lesson 36 README](../../../examples/control/02-derivative-action-and-dynamic-response/34-servo-pid-holding-torque/README.md).

Trigger/inputs: unloaded/loaded shaft model, load, motion requirements,
limits, timing and candidate P/PD/PID structures. Automatically calculate
load torque, steady error, I increment, tracking and search Ki/Kd/Kp candidates
that satisfy positioning, holding and saturation constraints. Recommend the
simplest candidate that rejects the load; escalate to capacity or load-model
review when none passes. Manual controls choose/override gains, load timing
and reset. Separate unloaded and loaded runs must apply load at time zero only
in the loaded case. Accept peak/error/holding evidence; fail reused I or
physical safe-hold claims after zero torque.

<a id="wf-036"></a>

## WF-036 — Derivative filtering

Source: [Lesson 37 README](../../../examples/control/02-derivative-action-and-dynamic-response/36-filtered-derivative/README.md).

Trigger/inputs: angle/noise record or sensor model, measured/previous angle,
velocity, Kd, sample period, motion/noise requirements and reset. Automatically
estimate raw derivative noise, calculate filter need and search filter time and
derivative parameters, then recommend the simplest filter meeting activity,
braking, peak and torque. Manual controls choose/override filter type/time,
noise probe and reset. Separate full resets use identical input conditions.
Accept activity reduction plus true-motion pass; fail replayed angle
trajectories or a filter selected from noise alone.

<a id="wf-037"></a>

## WF-037 — Discrete PID

Source: [Lesson 38 README](../../../examples/control/03-production-quality-digital-pid/37-discrete-integral-derivative/README.md).

Trigger/inputs: continuous model, sample period, elapsed-time policy, limits,
P/I/D requirements and state semantics. Automatically choose a supported
discretization, calculate e/P/trial-I/D/request/clamp/tracking in order, and
recommend a realization with explicit retained-state meanings. Manual controls
choose/override method, period, tracking and initialization, then inspect the
prescribed .40/.42/.50 rad benchmark and separate plant tests. Accept matching
state semantics and timing; fail reordered updates or hidden discretization.

<a id="wf-038"></a>

## WF-038 — Positional and incremental PID

Source: [Lesson 39 README](../../../examples/control/03-production-quality-digital-pid/38-positional-incremental-pid/README.md).

Trigger/inputs: continuous controller, supported discretization, mode/gain/
filter/limit changes, elapsed-time policy and state contract. Automatically
generate positional and supported incremental realizations, derive retained
request/tracking/gain-change state semantics, and recommend the simplest form
that preserves behavior. Manual controls choose/override realization, state
preload, period and limits; verify the 25→20 N m clipping and gain-change
benchmarks on a separate plant. Accept equivalent stateful outputs and clear
mode boundaries; fail clipped output used as previous unlimited request or
arithmetic-only equivalence.

<a id="wf-039"></a>

## WF-039 — Servo commissioning

Source: [Lesson 41 README](../../../examples/control/14-integrated-capstones/82-servo/README.md).

Trigger/inputs: validated shaft candidate, filter, realization, lifecycle,
load, reset, timing and limits. Automatic result evaluates I=u_applied-P-D,
unloaded peak/error, loaded hold and lifecycle/revision requirements. Recommend
the simplest integrated candidate; escalate to explicit brake design for
physical holding claims. Manual separate unloaded/loaded and lifecycle cases
must each retain reset/event time/units. Fail untested lifecycle rows or
mixed histories.

<a id="wf-040"></a>

## WF-040 — Volume balance

Source: [Lesson 43 README](../../../examples/control/01-feedback-foundations-and-temperature-control/14-conservation-and-accumulation/README.md).

Trigger/inputs: tank A=.5, h=.25, inlet/outlet flows and 1 s update. Automatic
result calculates A dh/dt=qin-qout, volume and level. Recommend constant-outflow
balance only for this case; escalate to gravity model otherwise. Manual .012
for 10 s then .010 must show .29 m within .001 m and level hold. Fail writing
prediction into sensor or claiming gravity.

<a id="wf-041"></a>

## WF-041 — Gravity outflow

Source: [Lesson 45 README](../../../examples/control/01-feedback-foundations-and-temperature-control/16-gravity-tank-nonlinearity/README.md).

Trigger/inputs: A=.5, c=.020, h=.25, qin=.010/.011, 1 s update and bounds.
Automatic result calculates qout=c sqrt(h), derivative and equilibrium
(qin/c)^2. Recommend nonlinear model in range; escalate on coefficient
mismatch. Manual 200 s .011 run must show .3025 m and drift <=.001 m; fail
negative-level sqrt repair or constant-outflow assumption.

<a id="wf-042"></a>

## WF-042 — Linearization

Source: [Lesson 46 README](../../../examples/control/01-feedback-foundations-and-temperature-control/17-gravity-tank-linearization/README.md).

Trigger/inputs: nonlinear model, operating point, input/output coordinates,
uncertainty, sample period and local-error requirement. Automatically compute
the Jacobian/local model at the selected point, gain/time constant, exact and
linear equilibria, residual and valid neighborhood. Recommend linearization
only inside the declared error; escalate nonlinear outside. Manual controls
choose/override operating point, perturbation, horizon and criterion. The
.011/.015 and .0025/.0625 cases are benchmarks, not product limits. Accept
matched reset comparisons; fail changing criterion after results.

<a id="wf-043"></a>

## WF-043 — Gain scheduling

Source: [Lesson 47 README](../../../examples/control/09-industrial-control-structures/56-gain-scheduling/README.md).

Trigger/inputs: nonlinear/local model, operating range, performance/robustness
requirements, candidate fixed controller and scheduling variables. Automatically
calculate local gains and schedule candidates, test whether one fixed design
passes first, and recommend scheduling only when fixed design fails or is
inadequate across the declared range. Manual controls choose/override fixed
gains, schedule breakpoints, interpolation and operating cases. Accept active
gain continuity and per-level evidence; fail hidden/discontinuous schedules or
scheduling adopted merely because it is available.

<a id="wf-044"></a>

## WF-044 — Laplace transforms

Source: [Lesson 48 README](../../../examples/control/01-feedback-foundations-and-temperature-control/18-transfer-function-from-physics/README.md).

Trigger/inputs: physical balance, deviations, operating point, initial state,
bias, gain and delay. Automatically show transform, derivative initial term,
zero-state transfer function, separate initial-condition response, closed-loop
response and delayed factor e^(-sL). Recommend the transfer function for its
zero-state open-loop purpose and retain the IC response alongside it; do not
reject TF analysis merely because IC is nonzero. A pure open-loop delay may shift the corresponding zero-state response in
time and contributes phase -omega*L; a delay-free closed-loop
response must never be shifted in place of recomputing its feedback equation.
Manual controls choose IC, delay and loop connection. Fail arbitrary IC
omission or delay-free closed-loop shifting.

<a id="wf-045"></a>

## WF-045 — Poles and zeros

Source: [Lesson 49 README](../../../examples/control/02-derivative-action-and-dynamic-response/19-poles-zeros-and-response/README.md).

Trigger/inputs: unreduced rational numerator/denominator, internal state model,
complex-plane plot and explicit learner-selected view. Automatic result retains
all unreduced poles/zeros and internal modes by default, identifies common
factors and uncertainty, and offers an explicit minimal input-output view with
a cancellation/hidden-mode warning. Recommend the simplest view only after
the internal model is reviewed; escalate hidden or unstable modes. Manual
controls choose unreduced versus minimal view and cancellation policy. Accept
both heater/shaft roots and deliberate cancellation evidence; fail default
cancellation, zero-crossing interpretation or silent unstable-mode deletion.

<a id="wf-046"></a>

## WF-046 — Characteristic equations

Source: [Lesson 50 README](../../../examples/control/07-classical-stability-and-frequency-design/43-characteristic-equation/README.md).

Trigger/inputs: connected shaft equations, retained controller states, J/B/
Kp/Kd, operating point and perturbation. Automatic result constructs the
complete characteristic equation, coefficients and roots. Recommend complete
lowest-order model; escalate unsupported hidden state/order. Manual J=B=1,
Kp=16, Kd=7 derivation and perturbation must match; fail unrelated transfer
denominator or omitted controller state.

<a id="wf-047"></a>

## WF-047 — Root locus

Source: [Lesson 51 README](../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/README.md).

Trigger/inputs: supplied or identified connected plant/controller model,
requirements, allowed gain range and all parameters except the chosen gain
held fixed. Automatically calculate pole paths, branch/breakaway information and
candidate gains for the general model. Obtain any claimed complete stability
region through WF-048/053/054 as applicable; a finite pole plot is not that proof. Manual controls choose gain parameter,
range, grid/refinement, candidate override and target-step verification.
Accept fixed-parameter invariants and matched responses; fail varying other
parameters or replacing the Tune root-locus feature with an ST root solver.

<a id="wf-048"></a>

## WF-048 — Routh–Hurwitz stability

Source: [Lesson 52 README](../../../examples/control/07-classical-stability-and-frequency-design/44-routh-hurwitz/README.md).

Trigger/inputs: the selected supplied OR identified plant model and compatible
controller representation, coefficient units, parameter ranges and uncertainty.
Manual complete-polynomial entry is an additional route, not a prerequisite. Automatically construct the
characteristic polynomial, show every Routh row/step/first-column sign, count
RHP poles, and calculate complete stable intervals/regions over the selected
parameter range. The J=B=1 shaft benchmark s²+s+Kp shows [1,Kp], [1,0], [Kp,0], Kp>0,
Kp=1 zero changes, Kp=-1 one, and Kp=0 excluded s(s+1) boundary; benchmarks
are not product limits.

Symbolic zero pivots and all-zero rows require exact special handling and the
auxiliary polynomial. A finite epsilon is a separate numerical-uncertainty
analysis and must report its bound; it cannot be presented as the symbolic
answer. Never divide by zero, treat zero as positive or silently perturb.
Manual controls choose model source, parameter range, precision and polynomial
route, then bounded-test a selected candidate. Accept complete intervals and
boundary status; fail gain tables, root-list substitution or unexplained
solver output. Degree-1/2 implementation limits remain a gap.

<a id="wf-049"></a>

## WF-049 — Periodic excitation

Source: [Lesson 53 README](../../../examples/control/05-experiments-identification-and-validation/22-excitation-for-dynamics/README.md).

Trigger/inputs: d=5–10 points, omega=.1/.2/.4 rad/s, manual bias 75%, PI off,
0.1 s sample, five cycles and 5P duration. Automatic result generates sine,
checks applied waveform, discards startup cycles and reports amplitude/phase
quality. Recommend slowest informative unclipped excitation; escalate when
cycles do not settle. Manual choose d/omega, retain all five cycles and use
final three. Accept bounded unclipped valid cycles; fail clipping, load change
or invented phase.

<a id="wf-050"></a>

## WF-050 — Frequency response

Source: [Lesson 54 README](../../../examples/control/07-classical-stability-and-frequency-design/46-bode-frequency-response/README.md).

Trigger/inputs: retained periodic cycles, actual applied command/PV,
frequency, bias, delay and units. Automatic result calculates amplitude ratio,
phase direction, Bode magnitude/phase and uncertainty; recommend simplest
adequate model and escalate low-quality/unwrap ambiguity. Manual select cycles
and inspect markers; omega=.2 must compare M=.141421 and phi=-.985398. Fail
startup-cycle or nominal-model substitution.

<a id="wf-051"></a>

## WF-051 — Sensitivity functions

Source: [Lesson 55 README](../../../examples/control/07-classical-stability-and-frequency-design/48-sensitivity-functions/README.md).

Trigger/inputs: C, G, disturbance path, target, physical load and measurement
noise separately. Automatic result calculates L=CG, S=1/(1+L), T=L/(1+L),
output/noise paths and activity. Recommend simplest controller meeting
disturbance attenuation without noise/actuator cost; escalate loop shaping.
Manual separate injections with same reset must show S/T identity and distinct
paths. Fail combined injections or magnitude-sum identity.

<a id="wf-052"></a>

## WF-052 — Loop shaping

Source: [Lesson 56 README](../../../examples/control/07-classical-stability-and-frequency-design/49-loop-shaping/README.md).

Trigger/inputs: baseline C/G, performance and robustness objectives, noise and
actuator limits, and admissible lead/lag structure. Automatically calculate
lead/lag settings Tz/Tp, complete CFG, crossover, phase, high-frequency gain,
and objective/constraint metrics; recommend the simplest settings meeting all
objectives. Manual controls choose/override structure, target bandwidth,
phase-margin/noise targets and settings, then run matched baseline/compensated
cases. Accept complete-loop tradeoff evidence; fail judging F alone or
changing requirements.

<a id="wf-053"></a>

## WF-053 — Nyquist stability

Source: [Lesson 57 README](../../../examples/control/07-classical-stability-and-frequency-design/47-nyquist-stability/README.md).

Trigger/inputs: complete directed loop/model, delay, frequency contour and
-1 reference. Automatically compute open-loop unstable-pole count P from the
known model; if P is genuinely unknown, report unavailable and require an
engineer-supplied count rather than defaulting to zero. Construct positive and
negative branches, origin indentation and high-frequency closure, show path
direction, signed N and Z=P+N. Clockwise encirclements are positive by this
contract. Manual controls choose contour bounds, refinement, orientation review
and model/count override. Accept complete path, direction and pole count; fail
coarse screenshots, quiet-record stability proof, omitted branch/detour or
silent P=0.

<a id="wf-054"></a>

## WF-054 — Sampled-data stability

Source: [Lesson 60 README](../../../examples/control/07-classical-stability-and-frequency-design/50-position-z-plane/README.md).

Trigger/inputs: supplied or identified continuous plant, controller, performance
and robustness requirements, CPU budget, allowable computation delay and
sample-period range. Automatically choose a period meeting performance,
robustness and CPU constraints, calculate exact held dynamics with
Ad=exp(Ah), Bd=integral(exp(A t)B dt), including a valid singular-A method,
and augment the state for computation delay before roots/unit-circle tests.
Recommend the simplest period/controller meeting all objectives; do not define
“shortest stable” as the objective. Manual controls choose/override period,
plant integration accuracy, delay and target, with the override retained.
Verify the benchmark h=.02/.6 cases, pending torque order, clipping and timing
guard. Accept only exact held/delayed results; fail Euler-only proof, immediate
new-torque application, zero fallback for unknown model, or changed plant
integration between periods.

## Disposition and read evidence

These contracts preserve the approved physics even where current projects or
generic Studio stages are incomplete. Current root-locus project/model mismatch,
Routh degree limitations, and unavailable staged analysis are gaps, not reasons
to weaken the requested behavior. All 27 mapped READMEs and the three relevant
companion documents were read from this checkout.

Authority hashes at review:

572161e48b32416e41ac17a0dec4b80ddbe2bbe41cb98b697dbc6a90428620c3
be833b18256654d2a11b7074c005aab573c77a3cec1dde609e36ae54e03a18f5
99e2859414806105fbcf57c2422a25d0b986fdc54e9fd8c3adf2f5f09f49c9e2
9729fe1884f64b9dfa792cbdc684234df297f6aae25fccdb7e9f8ff919c475f8

Mapped README read paths are the explicit source links in WF-028 through
WF-054 above. Recompute their hashes after source/bundle synchronization.

Mapped README paths above are the read-path ledger; recompute their hashes after source or bundle synchronization.
