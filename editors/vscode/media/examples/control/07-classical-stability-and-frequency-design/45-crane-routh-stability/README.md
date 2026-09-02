# Prove an overhead-crane stability interval

Course position: Lesson 45 of 84. Prerequisite: `control-model-based-tuning`.
Next lesson: `control-servo-root-locus`.

**Equipment:** Overhead travelling crane trolley with suspended load.

**One clear objective:** derive the complete gain interval and predict stable,
marginal, and unstable crane responses before executable confirmation.

## The problem

The equipment in this lesson is Overhead travelling crane trolley with suspended load.

An overhead travelling crane moves a trolley while a suspended load introduces
oscillatory mechanical dynamics. The trolley-position setpoint and measured
trolley position use metres. Requested and applied motor force use percent of
the reviewed bidirectional drive range, from -100 to 100 percent.

The simulator contains three independent copies of the same local crane model.
They start from the same 0.1-metre trolley displacement and differ only in
proportional gain. This is Simulation evidence, not physical-plant evidence.

Lesson 44 ran selected tunings against nominal and perturbed process models.
Those finite experiments cannot prove stability for every gain between or
beyond the tested values. Deliberately searching for instability on a physical
crane would also be unsafe. We therefore need an algebraic stability test
before trying gains.

## What we need

We need to determine the complete trolley-gain interval that keeps every closed-loop mode stable before risking an executable crane response. Boundary and outside-range cases must then confirm what the calculation predicts.

## The new idea

The characteristic equation is the equation whose roots are the closed-loop
poles. A closed-loop pole is a natural response mode of the controlled system.
Its location is described with the complex frequency variable ($s$).

The left half-plane contains poles with negative real part; their natural
responses decay. The imaginary axis contains poles with zero real part. The
right half-plane contains poles with positive real part; their responses grow.
Asymptotic stability means every natural response decays. Marginal stability
means a mode persists without decay or growth. Instability means at least one
mode grows.

A polynomial coefficient multiplies one power of $s$. The Routh array
rearranges polynomial coefficients into rows. Sign changes down its first
column equal the number of right-half-plane poles, so we can count unstable
modes without first solving every root.

Two special cases matter. A zero first-column element is replaced temporarily
by a positive epsilon before taking the limit while counting signs. If a
complete zero row appears, form the auxiliary polynomial from the row above
and use its derivative to continue. A complete zero row indicates symmetric
roots and can expose an imaginary-axis boundary.

## Prediction

The linearised force balance and trolley/load dynamics produce an open-loop
denominator with one integrator and an oscillatory pair. Closing proportional
position feedback with positive proportional gain ($K$) gives

$p(s) = s^{3} + 2 \cdot s^{2} + 3 \cdot s + K$

Place the coefficients into the array. The $s^{3}$ row is $[1, 3]$. The $s^{2}$
row is $[2, K]$. The first element of the $s^{1}$ row is `(6 - K) / 2`. The
$s^{0}$ row contains $K$.

Every first-column element must be positive for asymptotic stability. Therefore
the complete interval is

$0 < K < 6$

For $K = 4$, the first column is $[1, 2, 1, 4]$: zero sign changes predict a
stable response. For $K = 6$, the zero row exposes the marginal boundary. The
polynomial factors into $(s + 2) \cdot (s^{2} + 3)$, giving one decaying pole and two
imaginary-axis poles. For $K = 8$, $[1, 2, -1, 8]$ has two sign changes and
predicts two unstable poles.

## Implementation

The actual Structured Text uses three explicit positional proportional paths:

```st
StableRequested := StableGain * (StableSetpoint - StablePosition);
MarginalRequested := MarginalGain * (MarginalSetpoint - MarginalPosition);
UnstableRequested := UnstableGain * (UnstableSetpoint - UnstablePosition);
```

The gains are 4, 6, and 8. Each request is limited into its applied motor force.
The program stops an individual case if position leaves the two-metre local
range or requested force exceeds the drive range. It contains no precomputed
pole, array, stability, or response-envelope answer.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Use Routh-Hurwitz and an independent root oracle before equivalent natural-response tests. The declared event lasts 40 seconds. It restores Stable trolley position to 0 m afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Stable trolley setpoint (SP, m) and Marginal trolley setpoint (SP, m); Stable trolley position (PV, m) and Marginal trolley position (PV, m); Stable applied motor force (MV, %) and Marginal applied motor force (MV, %), including units, limits, mode, and quality. The initial operating point is Stable trolley setpoint = 0 m, Marginal trolley setpoint = 0 m, Unstable trolley setpoint = 0 m, Stable applied motor force = 0 %, Marginal applied motor force = 0 %, Unstable applied motor force = 0 %, Stable trolley position = 0.1 m, Marginal trolley position = 0.1 m. The unresolved baseline problem is: A finite response at selected gains cannot prove the complete stability interval or expose a weakly excited unstable mode.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Use the Routh first-column signs and independent numerical roots as authority; use equivalent time traces only as confirmation.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **40**. The observed lesson signal is **Stable trolley position**. Control Studio uses the existing Boolean project authority **P1.ExperimentEnabled** at **%MX96.0**; it does not inject a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to Identical 0.1-metre initial displacement on each crane copy. Confirm the live values are current and the reviewed plan stays inside Stable applied motor force -100 % to 100 %; Marginal applied motor force -100 % to 100 %; Unstable applied motor force -100 % to 100 %; Stable trolley position -2 m to 2 m; Marginal trolley position -2 m to 2 m; Unstable trolley position -2 m to 2 m.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **RouthGainComparison** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Routh-Hurwitz stability criterion**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

4. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: stable_right_half_plane_poles == 0 (dimensionless) (tolerance 0 (dimensionless)); marginal_imaginary_axis_poles == 2 (dimensionless) (tolerance 0 (dimensionless)); unstable_right_half_plane_poles == 2 (dimensionless) (tolerance 0 (dimensionless)); stable_position_envelope_ratio < 0.25 (dimensionless) (tolerance 0.01 (dimensionless)).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Change only proportional gain. Hold the crane model, initial displacement,
zero setpoint, sample schedule, 40-second duration, limits, and seed equal.

Define the measures before seeing results:

- **First-column sign-change count** is the integer number of changes in the
  ordered Routh first column.
- **Position-envelope ratio** is the largest absolute position in the declared
  late window divided by the largest absolute position in the declared early
  window. Values below one show decay; values near one show persistence; values
  above one show growth.
- **Force travel** is actuator activity, calculated as the sum of absolute
  changes between consecutive applied-force samples, in percentage points.

Also retain peak position, peak requested and applied force, limit contact,
stop reason, polynomial, gain, windows, sample interval, and evidence class.

### Validation

First use an independent numerical root oracle. It must find no right-half-
plane roots for $K = 4$, two imaginary-axis roots for $K = 6$, and two right-
half-plane roots for $K = 8$. Then compare the time traces.

The stable trace must decay. The marginal trace must persist and be reported
as `Not settled`, never asymptotically stable. The unstable trace must grow or
reach its safe simulation abort. The time trace confirms the prediction; it
does not replace the Routh calculation or numerical root oracle. A short quiet
prefix or convenient final sample cannot override the pole classification.

Never test the unstable gain on a physical crane from this simulation result.
Physical work requires a separately reviewed plant model, topology, limits,
interlocks, operating procedure, authorised roles, and explicit approval.

### Retained evidence

The pole plot records the Routh-Hurwitz prediction beside the measured crane
motion. Use the decision summary for the engineering verdict and the complete
trace when you want to reproduce every plotted point.

![Crane pole locations and measured position](figures/crane-routh-evidence.svg)

[Complete trace](figures/crane-routh-evidence-trace.csv) ·
[Decision summary](figures/crane-routh-evidence-summary.csv)

## Conclusion

At gain 4, every first-column term is positive, so all natural modes decay. At
gain 6, the imaginary-axis pair keeps oscillating and cannot satisfy
asymptotic settling. At gain 8, the first-column sequence changes sign twice,
so two modes grow even if their early motion looks small. Initial conditions
and duration affect how clearly a mode appears in a plot, but not where the
closed-loop poles actually are.

## Reason for the next lesson

You can now establish a polynomial stability interval before hazardous trials
and distinguish stable, marginal, and unstable evidence. Routh-Hurwitz does not
show how poles move as gain changes or whether a stable response has acceptable
damping. Lesson 46 introduces root locus to show pole motion and why gain alone
may not meet a servo requirement.
