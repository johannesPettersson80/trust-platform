# Linearize a gravity-drained tank

Course position: Lesson 34 of 84. Previous lesson: [Conservation-law
models](../33-tank-heater-conservation/README.md). Next lesson: poles, zeros, and
real plant classes.

**Lesson objective:** Derive a first-order transfer function for a
gravity-drained tank at three operating points and use executable error evidence
to decide where each local model is valid.

This open-loop workshop is simulation evidence, not physical-plant evidence.

## The problem

**Equipment:** Gravity-drained water tank with variable-speed inlet pump.

A variable-speed pump supplies a vertical tank. Liquid leaves through a fixed
outlet under gravity. We measure tank level in centimetres, perform the physical
balance in metres, and command inlet flow in cubic metres per second.

The tank area is constant, the liquid properties do not change, and the outlet
coefficient is fixed. There is no overflow, sensor dynamic, pump dynamic,
disturbance, or controller in this lesson.

Lesson 33 prescribed inlet and outlet flow independently, giving a constant-gain
integrator. Here, gravity makes outlet flow depend on level. The same inlet-flow
change produces a different equilibrium and response speed at low and high
level. One linear model cannot therefore be assumed valid across the tank.

## What we need

We need a simple local relationship between pump-flow changes and level changes around a declared operating point. Its valid range must be measured because gravity outflow changes with level and one local approximation cannot represent the complete tank.

## The new idea

Tank-volume accumulation equals inlet flow minus gravity outlet flow. Define
tank area ($A$) in square metres, level ($h$) in metres, inlet flow ($q$) in cubic
metres per second, and outlet coefficient ($c$) in square-metre-root-metre per
second. The physical balance is:

$A \cdot dh/dt = q - c \cdot \sqrt{h}$

The square root of level makes the relation **nonlinear**: doubling level does
not double outlet flow.

An **operating point** is a constant inlet flow (q0) and constant level ($h_{0}$)
where there is zero accumulation. At that condition:

$q_0=c\sqrt{h_0}$

Define level deviation ($\Delta h$) as actual level minus operating-point level.
Define inlet-flow deviation ($\Delta q$) as actual flow minus operating-point
flow. A **first-order Taylor approximation** replaces the curved outlet-flow
relationship with its tangent near that operating point:

$\frac{d\Delta h}{dt}=-\frac{c}{2A\sqrt{h_0}}\Delta h+\frac{1}{A}\Delta q$

The **Laplace transform** represents this linear differential relation, with
zero initial deviation, as an algebraic input-output relation. The resulting
**transfer function** is:

$DeltaH(s) / DeltaQ(s) = K / (\tau \cdot s + 1)$

The static gain ($K$) is steady output-deviation divided by input-deviation. The
time constant ($\tau$) describes local response speed. Model **order** counts its
independent dynamic modes. A **pole** locates a mode, a **zero** shapes the
input-output response, and **dead time** delays it. Here:

$K=2\sqrt{h_0}/c$ and $\tau=2A\sqrt{h_0}/c$

This first-order model has one pole at `-1/tau`, no finite zero, and no dead
time. We do not invent an FOPDT delay merely because that model is familiar.

## Prediction

Use $A = 0.5 m_{2}$ and $c = 0.02 m_{2} \cdot \sqrt{m}/s$. At low, middle, and high levels
of **0.25, 0.50, and 0.81 metres**, calculate equilibrium inlet flows of
**0.010000, 0.014142, and 0.018000 cubic metres per second**.

The local gains are **50.000, 70.711, and 90.000 m/(m3/s)**. The local time
constants are **25.000, 35.355, and 45.000 seconds**. A `+0.001 m3/s` step
therefore predicts level rises of **5.000, 7.071, and 9.000 centimetres**.

The exact nonlinear equilibria rise by 5.250, about 7.321, and 9.250 cm. Each
small-step tangent model should therefore finish with about **0.250 centimetres
small-step error**. At the low operating point, a larger `+0.005 m3/s` step
predicts 25 cm locally but 31.25 cm nonlinearly: 6.25 cm error.

## Implementation

There is no controller under test. The actual Structured Text derives each
tangent model and advances it at the 100 ms sample interval:

```st
LowEquilibriumFlow := OutletCoefficient * SQRT(LowOperatingLevelMetres);
LowGain := LREAL#2.0 * SQRT(LowOperatingLevelMetres) / OutletCoefficient;
LowTimeConstant :=
    LREAL#2.0 * TankAreaSquareMetres * SQRT(LowOperatingLevelMetres)
    / OutletCoefficient;
LowPole := -LREAL#1.0 / LowTimeConstant;
LowLocalDeviationMetres := LowLocalDeviationMetres
    + SampleTimeSeconds
    * ((-LowLocalDeviationMetres + LowGain * FlowDeviation) / LowTimeConstant);
```

Three simulator plants independently execute the nonlinear gravity balance
from the same physical parameters and operating points.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Run three derived local models beside three nonlinear tanks, then execute a separately restored large-step boundary case. The declared event lasts 300 seconds. It restores Inlet-flow deviation to 0 m3/s afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Low-tank level (PV, m) and Middle-tank level (PV, m); Low-tank inlet flow (MV, m3/s) and Middle-tank inlet flow (MV, m3/s), including units, limits, mode, and quality. The initial operating point is Inlet-flow deviation = 0 m3/s, Low-tank inlet flow = 0.01 m3/s, Low-tank level = 0.25 m, Middle-tank inlet flow = 0.0141421356 m3/s, Middle-tank level = 0.5 m, High-tank inlet flow = 0.018 m3/s, High-tank level = 0.81 m. The unresolved baseline problem is: A tangent model derived at low level cannot represent the nonlinear tank over its complete operating range.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Use a local model only with its operating point, declared range, and validated error evidence.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Manual output step**, **Amplitude** = **0.001 m3/s**, and **Maximum duration (s)** = **300**. The affected signal is **Inlet-flow deviation**; it starts at 0 m3/s and must restore to 0 m3/s.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Low-tank level 0 m to 1 m; Middle-tank level 0 m to 1 m; High-tank level 0 m to 1.2 m. Use this run only to test the declared local approximation; do not Identify, Tune, or Apply.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Inlet-flow deviation to 0 m3/s, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: low_small_step_local_error <= 0.3 cm (tolerance 0.01 cm); middle_small_step_local_error <= 0.3 cm (tolerance 0.01 cm); high_small_step_local_error <= 0.3 cm (tolerance 0.01 cm); low_large_step_extrapolation_error >= 5 cm (tolerance 0.01 cm).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

**Local-model prediction error** is the absolute difference between nonlinear
level and local-model level at the same timestamp, in centimetres. Equilibrium
error is valid only after both traces satisfy a declared late-slope limit.

Before viewing results, declare the `+0.001 m3/s` case valid when final error is
at most **0.30 centimetres** at all three operating points. Label the
`+0.005 m3/s` case outside the small-signal validity case; its expected error is
more than 5 cm.

![Local gravity-tank models are accurate only near their operating points](figures/gravity-tank-linearization.svg)

[Retained run](figures/gravity-tank-linearization-run.csv) ·
[Plot data](figures/gravity-tank-linearization-trace.csv) ·
[Decision summary](figures/gravity-tank-linearization-summary.csv)

### Validation

Independent arithmetic must reproduce every equilibrium flow, gain, time
constant, and pole. The nonlinear tanks must hold their operating points before
the step. The executable results were:

| Case | Absolute nonlinear-versus-local error |
|---|---:|
| Low, small step | 0.250 cm |
| Middle, small step | 0.249 cm |
| High, small step | 0.245 cm |
| Low, large step | 6.241 cm — outside validity |

The measured large-step error was **6.241 centimetres**, outside the declared
small-signal validity case.

The complete small-step transients and qualified equilibria must agree within
the declared limit. The large-step divergence is expected model-boundary
evidence, not a failed solver.

## Conclusion

At higher level, the tangent of the square-root outlet curve gives larger local
gain and a longer time constant. Linearization preserves that nearby tangent,
not the entire curved relationship. The pole describes one local exponential
mode; it does not make the physical tank globally linear.

These three local plant models are **not gain scheduling**. Gain scheduling
would change a controller according to operating region; no controller exists
here.

## Reason for the next lesson

A transfer function is a compact local input-output model derived under stated
assumptions and zero initial deviation. Always retain its operating point,
input and output ranges, and validation evidence. Lesson 35 uses poles and
zeros to predict qualitative behavior across several real plant classes.
