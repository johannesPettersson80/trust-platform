# Shape a precision motor-speed loop

Course position: Lesson 49 of 84. Prerequisite: `control-delayed-thermal-nyquist`.
Next lesson: `control-position-z-plane-sampling`.

**Equipment:** Precision motor and compliant load coupling.

**Lesson objective:** Compare proportional and loop-shaped motor-speed controllers against common frequency- and time-domain requirements.

## The problem

The equipment in this lesson is Precision motor and compliant load coupling.

A motor drives a precision load through a compliant coupling. The drive's
rated-torque command is measured in percent. A load-side encoder measures speed
in revolutions per minute (rpm). The experiment uses small speed deviations
about a 1000 rpm operating speed. A speed deviation of 1 rpm therefore means an
actual shaft speed of 1001 rpm.

The measured-speed channel contains identical deterministic encoder
interference in both cases. A separate true-speed channel lets you distinguish
encoder noise from physical shaft motion. Opposing load torque is a second
physical plant input; it is not an artificial change to the speed reading.

The two simulated motors have identical dynamics, initial state, setpoint,
load event, 10 millisecond timing, limits, interference, seed, and abort range.
Only the controller changes. This is **Simulation evidence, not physical-plant
evidence**. A physical drive also requires reviewed current, speed, vibration,
coupling, guarding, emergency-stop, and machine-motion limits.

Lesson 48 used Nyquist analysis to determine whether delayed loops were stable.
Lesson 47 calculated gain and phase margins near crossover. Those results do
not say how much load torque, encoder noise, model error, or controller action
passes through the precision-drive loop at each frequency.

The baseline proportional loop is stable, but it crosses near the compliant
shaft mode with little phase reserve. A stable loop can therefore amplify
speed motion and torque activity badly. We need to examine every important
closed-loop path before choosing it.

## What we need

We need the motor-speed loop to reject low-frequency load changes, avoid amplifying high-frequency noise, and retain adequate stability margins at the same time. Competing loop shapes must be compared against those common frequency- and time-domain requirements.

## The new idea

The plant response describes how motor torque changes load-side speed. After
that physical meaning is clear, call the plant response `G(s)`, where the
continuous frequency variable ($s$) represents dynamic response. The
controller response describes how measured speed error produces requested
torque; call it `C(s)`.

The open-loop response is the controller response followed by the plant
response around the feedback path. Call this loop response ($L$). Only now can
we write:

$L(s) = C(s)G(s)$

The sensitivity function describes the fraction of an output disturbance or
local model error that remains after feedback. Call it sensitivity ($S$). The
complementary-sensitivity function describes setpoint transmission and how
measurement noise reaches physical output. Call it complementary sensitivity
($T$). Both paths use the same feedback denominator:

$S(s) = 1 / (1 + L(s))$

$T(s) = L(s) / (1 + L(s))$

$S(s) + T(s) = 1$

The plant-input load-disturbance path is `G(s)S(s)`. Its magnitude is measured
in rpm of speed motion per percent of opposing load torque. The
measurement-noise to requested-torque path is the control-sensitivity function,
`C(s)S(s)`. Its magnitude is percent torque per rpm of encoder noise. Its minus
sign changes direction, not plotted magnitude.

Closed-loop bandwidth is the first frequency where the magnitude of
complementary sensitivity falls below one divided by the square root of two
times its low-frequency value. Maximum sensitivity ($M_{s}$) is the largest
qualified magnitude of sensitivity. Maximum complementary sensitivity ($M_{t}$)
is the largest qualified magnitude of complementary sensitivity. Roll-off is
the rate at which a response falls at high frequency.

For this stable, proper, minimum-phase loop, making sensitivity small over one
frequency band forces it to recover elsewhere. This redistribution is the
waterbed effect. Plants with unstable poles, nonminimum-phase zeros, delay, or
nonproper controllers require additional qualifications; do not treat the
statement as universal.

The local flexible-drive model has speed deviation as output and controller
torque minus opposing load torque as input:

$G(s) = 10 / (s^{2} + 0.6s + 1) rpm/\%$

The baseline positional controller requests 0.4 percent rated torque for each
rpm of speed error:

$C_{\mathrm{base}}(s) = 0.4 \%/rpm$

The shaped controller keeps the same zero-frequency gain. Its lead factor adds
phase near crossover. Its lag factor reduces gain around the flexible mode
relative to slow-load authority. Its low-pass factor rolls off high-frequency
controller response:

$C_{\mathrm{shape}}(s) = 0.4 \cdot ((1+s/0.3)/(1+s/3)) \cdot ((1+s/0.8)/(1+s/0.08)) \cdot (1/(1+s/8))$

This controller is proper and uses real filter states. It does not contain an
ideal derivative.

## Prediction

Calculate these values on the same 0.001 through 100 rad/s grid before running:

| Measure | Baseline | Shaped | What it means |
|---|---:|---:|---|
| $\left|S\right|$ at 0.01 rad/s | 0.200 | 0.201 | Similar slow-load rejection |
| $M_{s}$ | 3.197 | 1.724 | Less worst-case disturbance/model amplification |
| $M_{t}$ | 3.009 | 1.334 | Less worst-case tracking/noise amplification |
| Gain crossover | 2.187 rad/s | 1.706 rad/s | Lower shaped-loop crossover |
| Phase margin | 19.13 degrees | 44.11 degrees | More shaped-loop phase reserve |
| Bandwidth | 3.43 rad/s | 2.83 rad/s | Shaped response is intentionally slower |
| $\left|T\right|$ at 10 rad/s | 0.0420 | 0.0247 | Less high-frequency output noise |
| $\left|CS\right|$ at 10 rad/s | 0.4168 %/rpm | 0.2449 %/rpm | Less noise-driven torque activity |

At 0.01 rad/s, predict load-disturbance magnitudes of 2.000 and 2.013
rpm/%. Shaping should therefore preserve slow-load authority, reduce resonance
and noise activity, and pay for that improvement with lower bandwidth and some
slower transient load recovery.

## Implementation

The baseline controller directly implements its positional law:

```st
BaselineError := SpeedSetpoint - BaselineMeasuredSpeed;
BaselineRequestedTorque := ProportionalGain * BaselineError;
BaselineAppliedTorque := MAX(MIN(BaselineRequestedTorque, TorqueMaximum), TorqueMinimum);
```

The shaped path updates the actual lead, lag, and roll-off states at the real
cycle interval. The final filter state produces the requested torque:

```st
LeadState := LeadState + CycleInterval * (-LeadPole * LeadState + ShapedError);
LagState := LagState + CycleInterval * (-LagPole * LagState + LeadOutput);
RollOffState := RollOffState + CycleInterval * (-RollOffPole * RollOffState + LagOutput);
ShapedRequestedTorque := ProportionalGain * RollOffPole * RollOffState;
```

Requested and applied torque remain separately visible because limits can make
them differ. The full implementation in `src/Main.st` also exposes measured and
true speed, load torque, timing, and independent abort states.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Run the proper lead-lag-roll-off controller without changing any plant or experiment condition. The declared event lasts 120 seconds. It restores Baseline opposing load torque to 0 % afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Speed-deviation setpoint (SP, rpm); Baseline measured speed deviation (PV, rpm) and Shaped measured speed deviation (PV, rpm); Baseline applied torque (MV, %) and Shaped applied torque (MV, %), including units, limits, mode, and quality. The initial operating point is Speed-deviation setpoint = 0 rpm, Baseline applied torque = 0 %, Shaped applied torque = 0 %, Baseline measured speed deviation = 0 rpm, Shaped measured speed deviation = 0 rpm. The unresolved baseline problem is: The stable proportional loop crosses near the compliant mode with large sensitivity peaks, ringing, and encoder-noise torque activity.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept shaping only when common frequency and executable time evidence satisfy load, noise, robustness, bandwidth, effort, limit, and validity requirements together.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **120**. The observed lesson signal is **Baseline opposing load torque**. Control Studio uses the existing Boolean project authority **P1.ExperimentEnabled** at **%MX96.0**; it does not inject a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to Identical 10 rad/s, 0.02 rpm measured-speed interference on both encoder channels. Confirm the live values are current and the reviewed plan stays inside Baseline applied torque -20 % to 20 %; Shaped applied torque -20 % to 20 %; Baseline measured speed deviation -5 rpm to 5 rpm; Shaped measured speed deviation -5 rpm to 5 rpm.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **SensitivityShapingComparison** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **Sensitivity functions and loop shaping** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **SensitivityShapingComparison** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Sensitivity functions and loop shaping**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: shaped_maximum_sensitivity < 1.8 (dimensionless) (tolerance 0.003 (dimensionless)); shaped_maximum_complementary_sensitivity < 1.4 (dimensionless) (tolerance 0.003 (dimensionless)); shaped_phase_margin >= 40 deg (tolerance 0.08 deg); shaped_bandwidth_minimum >= 2.6 rad/s (tolerance 0.02 rad/s).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Change only the controller. Hold the plant, initial state, setpoint, load event,
sample schedule, duration, limits, interference, seed, and abort logic equal.

Retain complex loop response, sensitivity, complementary sensitivity, load and
control sensitivity, the $S+T$ residual, every crossover, bandwidth, $M_{s}$,
$M_{t}$, phase margin, model revision, controller revision, and evidence class.

For the time traces, setpoint overshoot is peak true-speed departure above the
predicted 0.8 rpm pre-load equilibrium. Settling time is the first time after
which true speed remains within 2 percent of that equilibrium. Load departure
is the largest absolute true-speed distance from the 0.6 rpm post-load
equilibrium. Torque total variation is the sum of consecutive requested-torque
changes and is reported in percentage points. Measure encoder-noise torque
travel over the final quiet window; do not call encoder movement physical shaft
movement.

### Validation

The shaped candidate shall keep low-frequency $\left|S\right|$ and $\left|GS\right|$ within 2 percent
of baseline, reduce $M_{s}$ below 1.8, reduce $M_{t}$ below 1.4, provide at least 40
degrees phase margin, reduce both $\left|T\right|$ and $\left|CS\right|$ at 10 rad/s by at least 35
percent, and keep bandwidth between 2.6 and 3.0 rad/s. Executable time evidence
shall show less overshoot and less requested-torque movement while leaving any
slower load recovery visible.

Reject nonfinite data, incomplete frequency coverage, multiple ambiguous
crossovers, an $S+T$ residual above `1e-9`, saturation, bad or stale quality,
timing outside tolerance, local-range departure, or an unsettled run. Report
`Not settled`, `Saturated`, `Aborted`, or `Invalid` instead of inventing a
performance number.

### Retained evidence

The frequency panel compares sensitivity ($S$) and complementary sensitivity
($T$) for both controllers. The speed traces show the physical benefit and the
slower load-recovery cost on equal simulated motors.

![Motor sensitivity and time response](figures/motor-sensitivity-evidence.svg)

[Complete trace](figures/motor-sensitivity-evidence-trace.csv) ·
[Decision summary](figures/motor-sensitivity-evidence-summary.csv)

![Motor speed response and control effort](figures/motor-sensitivity-response-evidence.svg)

[Complete response and effort trace](figures/motor-sensitivity-response-evidence-trace.csv)

## Conclusion

Large low-frequency loop gain makes sensitivity small and rejects slow load
changes. Because sensitivity plus complementary sensitivity equals one, both
cannot be made small at the same frequency. The baseline crosses near the
flexible mode with little phase reserve, so speed and torque are amplified.

The shaped controller redistributes gain, adds crossover phase, and rolls off
high-frequency action. This lowers the sensitivity peaks, ringing, and
noise-driven torque travel. Its lower bandwidth and slower load recovery are a
deliberate cost, not a hidden defect. Explain discrepancies using the discrete
filter realization, sample timing, finite windows, interference, limits, or
model mismatch rather than changing tolerances after seeing the result.

## Reason for the next lesson

You can now judge tracking, disturbance rejection, encoder-noise attenuation,
robustness, response speed, and actuator effort together. Stability or the
smallest sensitivity peak alone does not select a controller. Lesson 50 moves
the chosen continuous design into the **z-plane** and asks whether its sampled
implementation remains stable.
