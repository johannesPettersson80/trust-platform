# Filtered derivative for an encoder servo

Course position: Lesson 23 of 84. Previous lesson:
[Derivative kick and setpoint weighting](../22-servo-derivative-kick/README.md).
Next lesson: Mechanical actuator nonidealities (Lesson 24).

**Lesson objective:** Select a derivative-filter cutoff from measured noise
attenuation and lag, then decide whether a second filter stage is justified.

This lesson uses seeded simulation noise. Its results are not physical-encoder
or physical-axis evidence.

**Equipment:** Encoder-based industrial positioning table.

## The problem

The equipment in this lesson is Encoder-based industrial positioning table.

The loaded positioning table retains the PID settings and derivative on
measurement established in Lessons 21 and 22. Its encoder now contains
repeatable measurement noise with a standard deviation of 0.002 radians. The
raw encoder remains visible while a separate controller-facing position can be
unfiltered, first-order filtered, or second-order filtered.

All three runs use the same 0.5-rad setpoint, 20 ms cycle, 4 N m payload torque,
plus-or-minus 10 N m drive limit, plant, initial state, and random seed.

Derivative on measurement removed command kick, but it still reacts to every
measured change. Encoder noise is not physical table motion, yet a finite
difference treats it as motion and creates rapidly changing torque. A filter
may reduce that activity, but it also delays the controller's view of real
position. We must measure both effects.

## What we need

We need to attenuate encoder noise before it becomes alternating motor torque while retaining enough real motion information for damping. Filter strength and number of stages must be selected from both torque-noise and lag evidence.

## The new idea

The **raw encoder position ($PV_{\mathrm{raw}}$)** is the noisy sensor measurement in radians
(rad). **Measurement noise ($n$)** is unwanted variation added to the true
position. Its **standard deviation (σ)** describes its typical size; σ is
0.002 rad here.

The **controller-facing position ($PV_{\mathrm{control}}$)** is the value passed to PID. It
equals $PV_{\mathrm{raw}}$ when filtering is bypassed.

The **cycle time ($\Delta t$)** is 0.02 seconds. A finite-difference derivative divides
the change between samples by $\Delta t$:

$estimated velocity = (PV_{\mathrm{control}},new - PV_{\mathrm{control}},old) / dt$

Dividing by a small $\Delta t$ makes fast measurement changes appear as large velocity.
Derivative gain then converts that false velocity into torque activity.

A **low-pass filter** lets slow position changes pass while attenuating fast
changes. The **filter time constant (τ)** controls its cutoff. A larger time
constant removes more fast motion but makes the filtered position respond more
slowly. **Filter order** is the number of identical stages applied in sequence:
one stage is first order; two cascaded stages are second order.

The **filter coefficient ($a$)** is dimensionless and is calculated by the shipped
filter from $\Delta t$ and τ:

$a = dt / (τ + 0.5 \times dt)$

For each stage, the **filtered output ($y_{\mathrm{f}}$)** moves a fraction a of the gap from
its previous value toward the current input:

$y_{\mathrm{f}},new = y_{\mathrm{f}},old + a \times (input - y_{\mathrm{f}},old)$

The second-order candidate applies that update twice. It attenuates more noise
but also adds more **phase lag**, meaning $PV_{\mathrm{control}}$ follows real motion later.
A real flexible mechanism can convert noisy torque into resonance excitation;
this rigid lesson measures torque activity, while Lesson 47 studies resonance.

The **torque-increment root-mean-square value ($RMS_{\Delta T}$)** measures scan-to-scan
applied-torque activity from 15 to 20 seconds. It is the square root of the mean
squared torque increments, measured in N m.

$\operatorname{RMS}\left(\Delta T\right) = \sqrt{\operatorname{mean}\left((T_{\mathrm{applied},k} - T_{\mathrm{applied},k-1})^{2}\right)}$

The **positive position-lag area ($A_{\mathrm{lag}}$)** sums positive $PV_{\mathrm{raw}}$ minus $PV_{\mathrm{control}}$
from 0 to 5 seconds and multiplies each sample by $\Delta t$. Its unit is radian-seconds
(rad s). More area means more delay in the controller-facing measurement.

$A_{\mathrm{lag}} = \sum \max\left(PV_{\mathrm{raw},k} - PV_{\mathrm{control},k}, 0\right) \times dt$

## Prediction

With $\Delta t$ equal to 0.02 s and τ equal to 0.05 s, the coefficient is:

$a = 0.02 / (0.05 + 0.5 \times 0.02) = 0.333$

If the raw measurement is 0.003 rad above the previous filtered output, one
first-order update moves by:

$change = 0.333 \times 0.003 rad = 0.001 rad$

The remaining fast change is attenuated. A second stage smooths the first-stage
output again. Predict progressively lower torque-increment RMS, but progressively
higher positive lag area.

## Implementation

The executable keeps the raw signal visible and changes only the selected path:

```st
ControllerPosition := FirstFilter.Update(Input := RawPosition, CycleTime := LREAL#0.02);
ControllerPosition := SecondFilter.Update(Input := RawPosition, CycleTime := LREAL#0.02);
Input.ProcessValue := ControllerPosition;
MotorTorque := Controller.Update(Input := Input);
```

Only one filter update is selected in a run; the two lines show the real
first- and second-order alternatives.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Run the first-order filter at 0.02, 0.05 and 0.10 s while changing only its time constant. Then compare first- and second-order filtering at the selected 0.05 s time constant. The declared event lasts 20 seconds. It restores Table-position setpoint to 0 rad afterward. The deterministic Simulation seed is 20260828.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Table-position setpoint (SP, rad); Raw encoder position (PV, rad); Applied motor torque (MV, N m), including units, limits, mode, and quality. The initial operating point is PV = 0 rad, MV = 4 N m. The unresolved baseline problem is: Derivative on the noisy raw encoder produces excessive scan-to-scan motor-torque changes.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Require torque-increment RMS at or below 0.250 N m and positive lag area at or below 0.020 rad s; among qualifying candidates choose the simplest filter and then the smallest time constant.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Setpoint step**, **Amplitude** = **0.5 rad**, and **Maximum duration (s)** = **20**. The affected signal is **Table-position setpoint**; it starts at 0 rad and must restore to 0 rad.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Raw encoder position -4 rad to 4 rad; Applied motor torque -10 N m to 10 N m.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Table-position setpoint to 0 rad, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260828**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: selected_filter_torque_increment_rms <= 0.25 N m (tolerance 0.001 N m); selected_filter_positive_position_lag_area <= 0.02 rad*s (tolerance 0.000001 rad*s); first_to_bypass_torque_increment_rms_ratio <= 0.8 (dimensionless) (tolerance 0.01 (dimensionless)); second_to_first_torque_increment_rms_ratio <= 0.8 (dimensionless) (tolerance 0.01 (dimensionless)).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Plot $PV_{\mathrm{raw}}$ and $PV_{\mathrm{control}}$ together, then align the applied-torque traces. Use
the same scale for all filter choices. Report $RMS_{\Delta T}$ to 0.001 N m and $A_{\mathrm{lag}}$ to
0.000001 rad s. Also calculate mean $PV_{\mathrm{control}}$ from 18 to 20 seconds.

Do not select the visually smoothest curve. Compare noise attenuation, added
lag, final tracking, and actuator limits together.

Before viewing the results, declare the selection rule: torque-increment RMS
must be at most 0.250 N m and positive lag area must be at most 0.020 rad s.
Among candidates that pass both limits, select the simplest filter with the
smallest time constant.

![Derivative filtering trades encoder-noise activity for lag](figures/filtered-derivative.svg)

[Retained samples](figures/filtered-derivative-trace.csv) ·
[Decision summary](figures/filtered-derivative-summary.csv)

The retained simulation produced:

| Filter | τ | Torque-increment RMS | Positive lag area |
| --- | ---: | ---: | ---: |
| Bypass | — | 0.875 N m | 0.009053 rad s |
| First order | 0.020 s | 0.466 N m | 0.010424 rad s |
| First order | 0.050 s | 0.192 N m | 0.017473 rad s |
| First order | 0.100 s | 0.097 N m | 0.031202 rad s |
| Second order | 0.050 s | 0.046 N m | 0.028451 rad s |

The 0.050 s first-order case is selected because it is the only tested candidate
that satisfies both predeclared limits. It removes 78% of the bypass torque
activity while keeping positive lag area below 0.020 rad s. The second stage
removes more activity, but fails the lag limit. That is an engineering tradeoff,
not an automatic upgrade.

### Validation

The selected filter must meet the predeclared 0.250 N m activity limit and
0.020 rad s lag limit. If several candidates qualify, select first order before
second order and then the smallest qualifying time constant. The 0.050 s
first-order candidate is the only tested candidate satisfying both limits.

For every case, mean $PV_{\mathrm{control}}$ from 18 to 20 seconds must remain within
0.020 rad of the 0.5-rad SP. Every value must be finite and applied torque must
remain within plus or minus 10 N m.

## Conclusion

Filtering reduces rapid measurement changes before PID differentiates them, so
the drive receives smaller scan-to-scan torque changes. Each filter stage also
requires time to follow real position, so the controller temporarily acts on an
older value. More filtering is therefore not automatically better.

The correct choice depends on sensor noise, sample time, required bandwidth,
plant resonance, and acceptable lag. A smooth plot alone is not a control-system
requirement.

## Reason for the next lesson

Use a derivative filter only after preserving the raw measurement and measuring
both attenuation and delay. Physical qualification also needs encoder spectrum,
drive current, structural resonance, and timing evidence.

The next lesson adds Coulomb friction, deadband, backlash, stiction, and motion
limits one at a time. Those mechanical effects cannot be repaired simply by
adding more filtering or retuning PID.
