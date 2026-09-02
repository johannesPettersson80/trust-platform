# Qualify a digital PID realization on the positioning table

Course position: Lesson 25 of 84. Previous lesson:
[Mechanical nonidealities](../24-servo-mechanical-nonidealities/README.md).
Next lesson: Positional, incremental, and combined PID forms (Lesson 26).

**Lesson objective:** Calculate and execute forward-Euler, backward-Euler, and
Tustin realizations at several controller intervals, then decide which cases
remain credible from actual sampling, held torque, tracking, quantization,
aliasing, precision, and computation-delay evidence.

This simulation does not qualify physical timing, drive latency, or an encoder.

**Equipment:** Encoder-controlled industrial positioning table.

## The problem

The equipment in this lesson is Encoder-controlled industrial positioning table.

The positioning table runs with a 10 ms plant step. Its controller updates
every one, two, or ten scans and holds the last torque between updates.

The gains remain 8 N m/rad, 2 N m/(rad s), and 3 N m s/rad with a 0.05 s
derivative filter. Only realization, interval, resolution, and delay change.

Continuous gains do not say which samples a computer uses, how it approximates
integration and differentiation, or when its calculation reaches the drive.

The discrete controller must therefore be qualified.

## What we need

We need to specify the actual sampled recurrence, held torque, elapsed interval, numerical precision, quantization, and computation delay that the controller executes. A continuous equation alone cannot establish which digital implementation remains credible.

## The new idea

A **continuous signal** exists at every modelled instant. A **sampled signal**
contains values only at controller updates.
The **controller sample interval ($\Delta t$)** is the elapsed time in seconds between
controller updates.

The **sampling frequency ($f_{s}$)** is the number of updates per second, measured
in hertz (Hz):

$f_{s} = 1 / dt$

The **Nyquist frequency ($f_{N}$)** is half the sampling frequency. Frequencies
above it cannot be distinguished uniquely:

$f_{N} = f_{s} / 2$

A **zero-order hold** retains the last motor torque between updates. Faster
plant simulation does not create new controller decisions during that hold.

The **setpoint ($SP$)** is desired angle, the **process value ($PV$)** is sampled
angle, and the **error ($e$)** is SP minus PV. Index **k** is the current update;
`k-1` is the previous update.

The **integral torque ($T_{I}$)** remembers error. Forward Euler adds the previous
error, backward Euler adds the current error, and Tustin uses the average of
previous and current error:

$forward: T_{I,k} = T_{I,k-1} + K_i \times dt \times e_{k-1}$

$backward: T_{I,k} = T_{I,k-1} + K_i \times dt \times e_{k}$

$Tustin: T_{I,k} = T_{I,k-1} + K_i \times dt / 2 \times (e_{k} + e_{k-1})$

The **filtered derivative torque ($T_{D}$)** is the measurement-rate damping term
in N m. A 0.05 s filter is realized with forward Euler, backward Euler, or the
Tustin trapezoidal mapping; all three recurrences are executable and inspectable.

The **quantization step ($q$)** is the 0.001 rad spacing between encoder values.

The **alias frequency ($f_{\mathrm{alias}}$)** is the false lower frequency produced when a
sampled disturbance exceeds $f_{N}$. For 8 Hz interference sampled at 10 Hz:

$f_{\mathrm{alias}} = \left|8 Hz - 1 \times 10 Hz\right| = 2 Hz$

The **computation delay ($T_{\mathrm{delay}}$)** is one update: calculated torque is applied
at the next update.

`LREAL` improves coefficient and state precision; it cannot prevent aliasing,
quantization, stale samples, or a poor interval.

## Prediction

The joint's 1 kg m^{2} inertia and 1 N m s/rad friction give a 1 s velocity time
constant. **Samples per time constant** is 1 s divided by $\Delta t$.

| $\Delta t$ | $f_{s}$ | $f_{N}$ | Samples per 1 s | Integral weight $K_i$ × $\Delta t$ | 8 Hz interference |
| ---: | ---: | ---: | ---: | ---: | --- |
| 0.01 s | 100 Hz | 50 Hz | 100 | 0.02 N m/rad | below Nyquist |
| 0.02 s | 50 Hz | 25 Hz | 50 | 0.04 N m/rad | below Nyquist |
| 0.10 s | 10 Hz | 5 Hz | 10 | 0.20 N m/rad | aliases to 2 Hz |

Calculate the 0.05 s derivative-filter coefficients before execution:

| $\Delta t$ | Forward coefficient $\Delta t$/τ | Backward denominator 1 + $\Delta t$/τ | Tustin retained-state coefficient | Tustin position-change coefficient |
| ---: | ---: | ---: | ---: | ---: |
| 0.01 s | 0.200 | 1.200 | 0.818 | 54.545 N m/rad |
| 0.02 s | 0.400 | 1.400 | 0.667 | 50.000 N m/rad |
| 0.10 s | 2.000 | 3.000 | 0.000 | 30.000 N m/rad |

Thus $dt = 0.10 s$, $f_{s} = 10 Hz$, $f_{N} = 5 Hz$, and the forward coefficient is
2.0. One-update delay is 0.01, 0.02, or 0.10 s.

Predict different torque recurrences, longer holds at slower updates, a 2 Hz
alias at 100 ms, and a one-update shift with delay.

## Implementation

The lesson executes the alternatives directly:

```st
CASE MethodSelection OF
    0: IntegralTorque := IntegralTorque + LREAL#2.0 * CycleTime * PreviousError;
    1: IntegralTorque := IntegralTorque + LREAL#2.0 * CycleTime * Error;
ELSE
    IntegralTorque := IntegralTorque
        + LREAL#0.5 * LREAL#2.0 * CycleTime * (Error + PreviousError);
END_CASE;

RequestedTorque := Controller.Update(
    Setpoint := Setpoint,
    ProcessValue := MeasuredPosition,
    CycleTime := ControllerCycleTime,
    MethodSelection := MethodSelection,
    DelayEnabled := DelayEnabled
);
```

Between updates `RequestedTorque` is unchanged, implementing the hold.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. At 20 ms compare forward Euler, backward Euler, and Tustin. Then hold Tustin and continuous parameters fixed while testing 10, 20, and 100 ms, followed by the one-update delay case. The declared event lasts 20 seconds. It restores Table-position setpoint to 0 rad afterward. The deterministic Simulation seed is 20260828.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Table-position setpoint (SP, rad); Quantized measured position (PV, rad); Held motor-torque request (MV, N m), including units, limits, mode, and quality. The initial operating point is PV = 0 rad, MV = 0 N m. The unresolved baseline problem is: The fastest Tustin run is not evidence that Euler alternatives, slower updates, quantized measurements, aliased interference, or delayed application remain acceptable.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept only a method and interval whose actual update timing, held output, tracking, torque, finite-state, quantization, alias, and delay evidence satisfy the declared requirements.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Setpoint step**, **Amplitude** = **0.5 rad**, and **Maximum duration (s)** = **20**. The affected signal is **Table-position setpoint**; it starts at 0 rad and must restore to 0 rad.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Quantized measured position -4 rad to 4 rad; Motor velocity -20 rad/s to 20 rad/s; Held motor-torque request -10 N m to 10 N m.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Table-position setpoint to 0 rad, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260828**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: actual_controller_update_interval_maximum <= 0.1 s (tolerance 0.000001 s); held_torque_changes_outside_update == 0 N m (tolerance 1e-9 N m); sampled_8_hz_alias_amplitude_at_100_ms >= 0.001 rad (tolerance 0.0001 rad); estimated_alias_frequency_minimum >= 1.9 Hz (tolerance 0.000001 Hz).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Plot SP, PV, held torque, and update pulses together. Compare error, overshoot,
settling, torque travel, interval, jitter, and hold duration.

**Maximum absolute position error** is the maximum, over the declared run, of
the absolute value of SP minus PV. Calculate it from the sampled trace rather
than substituting the final error or the largest signed error.

**Applied-torque total variation** is the sum of the absolute successive
changes in applied torque. **Final-window position span** and **torque span**
are the respective maximum-minus-minimum values over the declared final two
seconds.

Inspect the sampled measurement residual spectrum. At 100 ms it should show a
2 Hz alias from the configured 8 Hz interference.

A case settles only when PV remains within 0.020 rad of SP and final 2 s spans
are at most 0.010 rad PV and 0.500 N m torque. Otherwise report no settling time.
Classify that case as **not settled**; if its sampled response grows rather than
remaining bounded, classify it as **unstable** and stop the run.

Evidence measured 0.210 N m separation between 20 ms candidates, a 0.0020 rad
2 Hz alias at 100 ms, and first torque at 0.010 s or 0.020 s with delay.

![Digital choices change the implemented controller](figures/digital-realization.svg)

[Retained samples](figures/digital-realization-trace.csv) ·
[Decision summary](figures/digital-realization-summary.csv) ·
[Method-comparison samples](figures/digital-method-comparison-trace.csv) ·
[Update-timing samples](figures/digital-update-timing-trace.csv) ·
[Alias-spectrum samples](figures/digital-alias-spectrum-trace.csv)

Plots expose recurrences, update pulses, and holds. The
retained 100 ms samples give an estimated alias frequency of 2.00 Hz.

![](figures/digital-method-comparison.svg)

![](figures/digital-update-timing.svg)

![](figures/digital-alias-spectrum.svg)

Every tested case remained finite and bounded:

| Case | Settling time | Torque total variation | Longest hold | Actual interval |
| --- | ---: | ---: | ---: | ---: |
| Forward Euler, 20 ms | 11.73 s | 101.34 N m | 0.02 s | 0.02 s |
| Backward Euler, 20 ms | 11.74 s | 77.65 N m | 0.02 s | 0.02 s |
| Tustin, 20 ms | 11.73 s | 89.11 N m | 0.02 s | 0.02 s |
| Tustin, 10 ms | 11.73 s | 94.46 N m | 0.01 s | 0.01 s |
| Tustin, 100 ms | 11.61 s | 24.33 N m | 0.10 s | 0.10 s |
| Tustin, 10 ms plus delay | 11.73 s | 98.25 N m | 0.01 s | 0.01 s |

Lower movement at 100 ms is not automatically better: the command is held
longer and 8 Hz interference appears as 2 Hz. Judge timing, tracking, alias,
and delay together.

### Validation

Torque shall change only on update pulses and remain held between them. Actual
update spacing shall be 10, 20, or 100 ms as selected. PV shall remain on the
0.001 rad grid. The spectrum estimated from the retained 100 ms samples shall
place the dominant alias between 1.9 and 2.1 Hz with at least 0.001 rad
amplitude. The delayed case shall shift first torque by one complete update.

State, PV, velocity, and torque must remain finite; torque stays within ±10 N m.

## Conclusion

Each recurrence approximates the same continuous terms with different samples
and coefficients. Increasing $\Delta t$ lengthens holds and delays, and can make an
8 Hz encoder component look like 2 Hz.

Numerical precision cannot reconstruct unsampled measurements. Timing,
sampling, and resolution are controller behavior.

## Reason for the next lesson

Choose from bandwidth, noise, phase margin, timing, numerical range, and
closed-loop evidence. Requalify after timing, filtering, delay, or hardware changes.

Lesson 26 builds on this result by comparing positional, incremental, and
combined production PID forms without pretending that their retained states
are interchangeable.
