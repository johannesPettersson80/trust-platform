# Reshape a positioning-servo root locus

Course position: Lesson 46 of 84. Prerequisite: `control-crane-routh-stability`.
Next lesson: `control-flexible-drive-bode`.

**Equipment:** Belt-driven industrial positioning carriage.

**One clear objective:** prove why proportional gain alone fails and design and
validate one lead compensator from explicit pole requirements.

## The problem

This lesson controls a belt-driven industrial positioning carriage. The
carriage-position setpoint and measured position use millimetres. Requested and
applied motor-current commands use percent of the drive rating and may act in
either direction.

Two independent simulated copies use the same motor and carriage model, zero
initial position, one-millimetre move, 10-millisecond control interval, current
limits, and deterministic seed. One uses proportional control; the other uses
the lead compensator designed in this lesson. This is Simulation evidence, not
physical-plant evidence.

Lesson 45 used Routh-Hurwitz to establish a stable gain interval. It did not
show how individual poles move, which poles dominate the response, or whether
a stable gain is fast and well damped enough.

This servo permits no more than 5-percent overshoot, requires 2-percent settling
within 2.0 seconds, requires zero final position error, and permits no current
saturation. We need to see why proportional gain cannot satisfy those
requirements together before changing controller structure.

## What we need

We need more freedom than proportional gain alone provides to place the positioning carriage's dominant response while keeping the remaining modes acceptable. Desired pole location, added dynamics, and executable response must tell the same story.

## The new idea

An open-loop pole is a natural mode before feedback. An open-loop zero is a
frequency where the transfer-function numerator becomes zero. A root-locus
branch is the path followed by one closed-loop pole while one positive scalar
gain changes.

Branches start at open-loop poles and finish at open-loop zeros or at infinity.
A real-axis segment is part of the locus on the real number line. A breakaway
point is where branches leave that line. An asymptote describes a branch going
to infinity; its centroid is the point where the asymptotes intersect the real
axis. The angle condition decides whether a point lies on the locus. The
magnitude condition calculates the gain at that point.

Natural frequency describes the undamped speed of a pole pair. Damping ratio
describes its decay relative to that frequency. A constant-damping line marks
all complex poles with one damping ratio. Dominant poles are the slow poles
that shape most of the visible response; a non-dominant pole decays sufficiently
faster to have less influence.

A lead compensator adds a zero and a farther-left pole. It adds positive phase
and reshapes the locus toward faster, better-damped poles. A lag compensator
primarily improves low-frequency accuracy and is not the design used here.

## Prediction

The motor inertia integrates acceleration into velocity and position, while
viscous friction damps motion. After those physical quantities are reduced to
the declared local units, the plant is

$G(s) = \frac{1}{s(s+2)}$

Its open-loop poles are 0 and -2, with no finite zero. The real-axis locus lies
between those poles and breaks away at -1. Its centroid is -1, and two
asymptotes leave at plus and minus 90 degrees.

With proportional gain ($K$), the characteristic equation is

$s^{2}+2s+K=0$

For $K > 1$, the poles are $-1 \pm j \cdot \sqrt{K-1}$. Their real part remains -1,
so increasing gain cannot reduce the approximate 2-percent settling time below
about 4 seconds; it increases frequency while reducing damping. At $K = 4$,
predict poles `-1 +/- j*1.732`, natural frequency `2 rad/s`, damping ratio
`0.5`, and about 16.3-percent overshoot.

The required region needs a more negative real part. Place a lead zero at
-2.5, a farther-left pole at -15, and use gain 50:

$C(s)=50\frac{s+2.5}{s+15}$

The angle and magnitude conditions lead to

$s^{3}+17s^{2}+80s+125=0$

Independent roots are approximately `-3.235 +/- j*1.186` and `-10.530`. The
dominant-pair damping ratio is about 0.939. The complete closed-loop transfer
function also contains a zero, so damping ratio alone does not predict the
step overshoot. Evaluating that complete transfer function predicts about
3.2 percent overshoot, 1.36-second settling, zero final error, and an initial
50-percent requested current.

## Implementation

The actual Structured Text exposes both laws. The proportional path is gain
times position error. The lead path realizes the declared pole-zero state:

```st
ProportionalRequested := ProportionalGain
    * (ProportionalSetpoint - ProportionalPosition);

LeadError := LeadSetpoint - LeadPosition;
LeadRequested := LeadGain
    * (LeadError + (LeadZero - LeadPole) * LeadFilterState);
LeadFilterState := LeadFilterState
    + CycleTime * (-LeadPole * LeadFilterState + LeadError);
```

Requested current is retained before the identical -100-to-100-percent clamp;
applied current is the command reaching each simulated plant. The program does
not contain pole locations, overshoot, settling time, or an accepted winner.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Implement lead compensation with gain 50, zero 2.5 per second, and pole 15 per second. The declared event lasts 8 seconds. It restores Lead carriage setpoint to 0 mm afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Proportional carriage setpoint (SP, mm) and Lead carriage setpoint (SP, mm); Proportional carriage position (PV, mm) and Lead carriage position (PV, mm); Proportional applied motor current (MV, %) and Lead applied motor current (MV, %), including units, limits, mode, and quality. The initial operating point is Proportional carriage setpoint = 0 mm, Lead carriage setpoint = 0 mm, Proportional applied motor current = 0 %, Lead applied motor current = 0 %, Proportional carriage position = 0 mm, Lead carriage position = 0 mm. The unresolved baseline problem is: After root-locus breakaway, proportional gain changes frequency and damping but cannot move the pole-pair real part left of minus one.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept only when complete-transfer-function poles and executable response meet overshoot, settling, final-error, and current constraints.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **8**. The observed lesson signal is **Lead carriage setpoint**. Control Studio uses the existing Boolean project authority **P1.ExperimentEnabled** at **%MX96.0**; it does not inject a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to Lead carriage setpoint. Confirm the live values are current and the reviewed plan stays inside Proportional applied motor current -100 % to 100 %; Lead applied motor current -100 % to 100 %; Proportional carriage position -5 mm to 5 mm; Lead carriage position -5 mm to 5 mm.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **LeadCompensatorComparison** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Root-locus lead-compensator design**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

4. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: lead_percent_overshoot < 5 % (tolerance 0.1 %); lead_two_percent_settling_time <= 2 s (tolerance 0.01 s); lead_final_window_absolute_error <= 0.01 mm (tolerance 0.001 mm).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Change only controller structure and its derived parameters. Hold plant,
initial state, setpoint event, sample schedule, duration, limits, and seed equal.

Percent overshoot is peak position above setpoint divided by commanded position
change, times 100. The 2-percent settling time is the first time after which
position remains within plus/minus 2 percent of setpoint. Report `Not settled`
if that condition never holds. Final-window absolute error is calculated only
for a settled response. Applied-current travel is the sum of absolute changes
between successive applied-current samples, in percentage points.

Also retain peak requested and applied current, time at limit, calculated pole
identities, damping, metric windows, and stop reason.

### Validation

The independent numerical oracle and executable run give:

| Design | Overshoot | 2% settling | Final-window error | Saturated |
|---|---:|---:|---:|---|
| Proportional, K = 4 | 16.70% | 4.06 s | 0.0006 mm | No |
| Lead, 50(s+2.5)/(s+15) | 3.35% | 1.38 s | < 0.0001 mm | No |

The proportional case fails overshoot and settling. The lead case meets both,
reaches the final-error tolerance, and remains unsaturated. A faster design
that touches the current limit would be rejected. The calculated roots and
complete response—not only a dominant-pole approximation—must agree with the
retained run.

### Retained evidence

The first panel shows where each controller places the closed-loop poles. The
second panel shows what those pole locations mean for the real positioning
requirement.

![Servo pole placement and position response](figures/servo-root-locus-evidence.svg)

[Complete trace](figures/servo-root-locus-evidence-trace.csv) ·
[Decision summary](figures/servo-root-locus-evidence-summary.csv)

## Conclusion

Gain alone moves the uncompensated pair vertically after breakaway, leaving
real part -1. It trades damping for oscillation frequency but cannot make the
decay faster. The added zero and farther-left pole change the angle condition,
moving the dominant branches into the required region. The third pole is much
farther left and decays faster. The closed-loop zero explains why overshoot is
not given by the dominant-pair damping ratio alone.

## Reason for the next lesson

You can now use root locus to understand pole motion, test whether gain alone
can meet requirements, and design a compensator whose complete response is
verified. Root locus still does not expose resonance, noise amplification, or
gain and phase margins across frequency. Lesson 47 introduces Bode analysis on
a flexible drive where one step response can hide resonance.
