# Measure resonance with a Bode plot

Course position: Lesson 47 of 84. Prerequisite: `control-servo-root-locus`.
Next lesson: `control-delayed-thermal-nyquist`.

**Equipment:** Flexible-shaft motor and load.

**One clear objective:** measure the flexible-drive frequency response, expose
resonance, and compare qualified unshaped and resonance-shaped margins.

## The problem

The equipment in this lesson is Flexible-shaft motor and load.

This lesson studies an industrial motor connected to its load through a flexible
shaft. The motor-torque actuator is bidirectional. Requested and applied torque
are percentages of rated torque. An encoder measures load-side shaft speed in
revolutions per minute.

Three independent simulated drives have the same shaft, operating speed of
1000 revolutions per minute, initial state, limits, 10-millisecond sample
interval, and deterministic seed. Each receives the same 1-percent torque
amplitude at a different frequency. This is Simulation evidence, not physical-
plant evidence.

Lesson 46 used root locus to show how closed-loop poles move and to improve one
positioning transient. A single transient does not directly show which input
frequencies the plant amplifies, how much timing lag accumulates, or how close
an entire feedback loop comes to instability. A lightly damped shaft can store
damaging torsional energy near one frequency even when a coarse step plot looks
acceptable.

We therefore measure response over frequency before applying another
controller.

## What we need

We need to measure how the flexible shaft responds across frequency, locate its resonance, and judge stability margins before shaping the loop. Any added shaping must reduce the resonant risk without hiding poor excitation or violating time-domain requirements.

## The new idea

A sinusoidal steady state is the repeating response after the starting
transient has decayed. Angular frequency ($\omega$) states how quickly the sine
wave progresses and is measured in radians per second. One cycle equals two pi
radians.

Input amplitude is half the peak-to-peak applied-torque sine wave. Output
amplitude is half the peak-to-peak load-speed sine wave after its operating
speed has been removed. Magnitude is output amplitude divided by input
amplitude, here in revolutions per minute per percent torque. Phase is the
output's timing relative to the input, in degrees. Negative phase means that
load speed lags motor torque.

A logarithmic frequency axis gives equal space to equal frequency ratios. A
decade is a ten-to-one frequency interval. A decibel expresses an amplitude
ratio logarithmically. After magnitude has been defined, its decibel value is

$M_{\mathrm{dB}}=20\log_{10}($M$)$

A Bode magnitude plot shows magnitude against logarithmic frequency. A Bode
phase plot shows phase against the same frequency. Resonance is a frequency
region where stored energy makes the response much larger. Resonant frequency
is the frequency of that peak. Bandwidth is the frequency range over which the
closed loop can respond usefully.

The open-loop transfer is the plant and controller response multiplied before
feedback is closed. A gain crossover is a frequency where open-loop magnitude
equals one, or zero decibels. Phase margin is the additional phase lag required
at a gain crossover to reach minus 180 degrees. A phase crossover is a
frequency where phase reaches minus 180 degrees. Gain margin is the gain change
required there to reach magnitude one. Delay margin converts a positive phase
margin into the additional pure delay that would consume it.

Every crossover matters. When several exist, the least favourable valid margin
governs; a convenient crossing must not hide another one. If the required
crossing does not exist, its margin is reported as `Unavailable` or `Unbounded`
according to the calculation. Coherence is a zero-to-one data-quality measure
of how much output at a frequency is consistently related to the measured
input. Poor coherence makes an estimated response ineligible for margin work.

The requested motor torque ($u_{\mathrm{requested}}$) is the command calculated by the
experiment. The applied motor torque ($u$) is the limited command received by the
plant. The load-speed deviation ($y$) is measured speed minus the 1000-rpm
operating speed. The continuous frequency variable ($s$) describes the model's
dynamic response.

Shaft stiffness restores displacement, inertia resists acceleration, and
viscous damping removes stored motion. In local deviation units the declared
plant model is

$G(s)=\frac{y(s)}{u(s)}=\frac{1}{s^{2}+0.2s+4}$

Its static gain is 0.25 rpm/%, natural frequency is 2 rad/s, and damping ratio
is 0.05. At a chosen angular frequency, the magnitude and phase are

$\lvert G(j\omega) \rvert =\frac{1}{\sqrt{\left(4-\omega^{2}\right)^{2}+\left(0.2\omega\right)^{2}}}$

$\phi(\omega)=-\operatorname{atan2}\left(0.2\omega,4-\omega^{2}\right)$

These equations are valid for the local linear model around 1000 rpm. They do
not include backlash, extra shaft modes, drive-current dynamics, protection,
temperature, or installation flexibility.

## Prediction

Before running, calculate three responses:

| Probe frequency | Predicted magnitude | Predicted phase |
|---:|---:|---:|
| 0.5 rad/s | 0.267 rpm/% | -1.5 degrees |
| 2 rad/s | 2.500 rpm/% | -90.0 degrees |
| 8 rad/s | 0.0167 rpm/% | -178.5 degrees |

At 2 rad/s, the stiffness term and inertia term cancel in the real part of the
denominator. Only the small damping term remains, so the flexible mode amplifies
the 1-percent torque probe to approximately 2.5 rpm.

The baseline controller gain is 5. Its unshaped open loop has one gain
crossover near 2.99 rad/s and only about 6.9 degrees phase margin. The
resonance-shaped candidate is

$C_{\mathrm{shape}}(s) =5\frac{s^{2}+0.2s+4}{s^{2}+3.2s+4}$

For the nominal model, its open loop reduces to

$L_{\mathrm{shape}}(s)=\frac{5}{s^{2}+3.2s+4}$

Predict one gain crossover near 1.44 rad/s and about 112.6 degrees phase
margin. This apparent pole-zero cancellation is model-dependent, so the
candidate remains unapplied until perturbed-model validation is complete.

## Implementation

This is an open-loop measurement, not hidden feedback control. The actual
Structured Text advances one clock and creates three equal-amplitude probes:

```st
ExperimentElapsed := ExperimentElapsed + CycleTime;
LowRequested := ProbeAmplitude * SIN(LowFrequency * ExperimentElapsed);
ResonantRequested := ProbeAmplitude * SIN(ResonantFrequency * ExperimentElapsed);
HighRequested := ProbeAmplitude * SIN(HighFrequency * ExperimentElapsed);

LowApplied := MAX(MIN(LowRequested, LREAL#100.0), LREAL#-100.0);
```

The same clamp is applied independently to all three requests. Requested and
applied torque are retained separately. Structured Text contains no fitted
magnitude, phase, margin, ranking, or accepted answer; Control Studio derives
those results from retained samples and the qualified model.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Calculate an unapplied resonance-shaped candidate with the same controller gain and test its model dependence. The declared event lasts 120 seconds. It restores Low applied motor torque to 0 % afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Low-frequency load speed (PV, rpm) and Resonant load speed (PV, rpm); Low applied motor torque (MV, %) and Resonant applied motor torque (MV, %), including units, limits, mode, and quality. The initial operating point is Low applied motor torque = 0 %, Resonant applied motor torque = 0 %, High applied motor torque = 0 %, Low-frequency load speed = 1000 rpm, Resonant load speed = 1000 rpm, High-frequency load speed = 1000 rpm. The unresolved baseline problem is: The unshaped loop has a large resonant peak and approximately 6.9 degrees phase margin.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept for further simulation only when measured response, all crossings, margins, and perturbed-model checks meet the declared criteria.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **120**. The observed lesson signal is **Low applied motor torque**. Control Studio uses the existing Boolean project authority **P1.ExperimentEnabled** at **%MX96.0**; it does not inject a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to Low applied motor torque. Confirm the live values are current and the reviewed plan stays inside Low applied motor torque -100 % to 100 %; Resonant applied motor torque -100 % to 100 %; High applied motor torque -100 % to 100 %; Low-frequency load speed 990 rpm to 1010 rpm; Resonant load speed 990 rpm to 1010 rpm; High-frequency load speed 990 rpm to 1010 rpm.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **FrequencyProbeBank** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **Measured Bode response and stability margins** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **FrequencyProbeBank** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Measured Bode response and stability margins**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: resonant_frequency_response_magnitude >= 2.425 rpm/% (tolerance 0.075 rpm/%); unshaped_phase_margin >= 6.8 deg (tolerance 0.1 deg); shaped_phase_margin >= 112.4 deg (tolerance 0.2 deg).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Change only probe frequency. Hold plant coefficients, operating point,
amplitude, initial state, sample schedule, duration, limits, and seed equal.

For each run, fit the sinusoidal fundamental on the declared steady-state
window. Divide fitted output amplitude by fitted input amplitude for magnitude.
Subtract fitted input phase from fitted output phase for phase lag. Retain the
window, sample interval, amplitudes, phase, coherence, and quality decision.

The simulator records each applied command at the end of the interval over
which that command was held. Before comparing measured phase with the
continuous-time equation, reference the held command to the interval midpoint.
For the 10-millisecond interval, this removes one-half sample of known timestamp
alignment; it does not remove unknown plant delay. Retain both the raw sampled
phase and the corrected continuous-time comparison phase.

For each candidate, retain every gain crossover, the least favourable valid
phase margin, phase crossover, gain margin or explicit unavailable state, delay
margin, resonance peak and frequency, bandwidth, model revision, and evidence
class. Never select a favourable crossover while hiding another one.

### Validation

The executable simulation shall reproduce the three predicted magnitudes
within 3 percent and phases within 2 degrees. The independent analytical oracle
shall find the unshaped crossover near 2.99 rad/s with about 6.9 degrees phase
margin and the shaped crossover near 1.44 rad/s with about 112.6 degrees phase
margin.

Reducing damping must worsen the unshaped margin. Moving the natural frequency
must expose the error in exact nominal cancellation. Reject a candidate whose
perturbed model violates the declared margin or resonance requirement. Similar
final speed or a visually smooth short trace cannot override the frequency-
domain evidence.

### Retained evidence

The measured simulator points are shown against the independently calculated
model curve. Agreement supports using the model for design; the resonance peak
shows why a short step response was not enough.

![Flexible-drive Bode evidence](figures/flexible-drive-bode-evidence.svg)

[Complete trace](figures/flexible-drive-bode-evidence-trace.csv) ·
[Decision summary](figures/flexible-drive-bode-evidence-summary.csv)

![Unshaped and resonance-shaped controller Bode evidence](figures/flexible-drive-controller-bode-evidence.svg)

[Complete controller Bode trace](figures/flexible-drive-controller-bode-evidence-trace.csv)

## Conclusion

Below resonance, stiffness dominates and load speed follows torque with little
lag. At 2 rad/s, stiffness and inertia nearly cancel, leaving weak damping and a
large response with 90-degree lag. Above resonance, inertia dominates and phase
approaches minus 180 degrees.

The shaped denominator adds damping around the flexible mode. Its nominal
margin improvement is large, but cancellation is only as accurate as the model.
Windowing, unresolved transients, sampling, noise, and model error must explain
differences between calculated and measured curves; results must never be
adjusted silently.

## Reason for the next lesson

You can now design a bounded sine experiment, estimate magnitude and phase,
recognize resonance, and interpret qualified Bode margins on another flexible
plant. Bode crossings still do not count unstable poles or unambiguously show
encirclement of minus one. Lesson 48 introduces Nyquist analysis for a delayed
thermal process where that distinction matters.
