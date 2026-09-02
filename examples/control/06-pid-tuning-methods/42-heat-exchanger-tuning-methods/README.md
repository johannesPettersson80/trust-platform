# Tune a heat exchanger deliberately

Course position: Lesson 42 of 84. Previous lesson:
[validate a heat-exchanger model on unseen conditions](../../05-experiments-identification-and-validation/41-heat-exchanger-validation-uncertainty/README.md).
Next lesson: [estimate critical behavior with a bounded relay test](../43-relay-autotuning/README.md).

**Equipment:** Plate heat exchanger with four independent simulated controller
copies.

**Lesson objective:** Tune proportional and integral action one term at a time, reproduce a historical critical-cycle PI tuning, and select the least aggressive candidate that satisfies the declared requirements.

## The problem

A steam valve controls the outlet temperature of a plate heat exchanger. Four
independent copies represent the same accepted local operating region. They
compare manual proportional-only, manual proportional-integral, critical
proportional-only, and historical proportional-integral control. Every copy is
simulated.

Each copy begins at 50 degrees Celsius with the valve at 20 percent. The local
heat-exchanger model has a gain of 0.5 degrees Celsius per valve percentage
point, a 20-second time constant, and a 3-second dead time. The controller runs
every 0.1 seconds. The valve is limited to 0 through 100 percent. The evidence
is Simulation evidence, not physical-plant evidence.

Lesson 41 showed that the model predicts this local operating region. A valid
model still does not decide how fast the loop should respond, how much
overshoot is acceptable, or how much valve movement is justified. This lesson
first makes those choices visible through manual tuning. It then compares them
with an intentionally aggressive historical rule.

## Derive what each controller term changes

The setpoint ($SP$) is the desired outlet temperature, in degrees Celsius. The
measured outlet temperature ($PV$) is the sensor value, also in degrees Celsius.
The control error ($e$) is setpoint ($SP$) minus measured outlet temperature ($PV$).
A positive error means the product is colder than desired.

\[
e=SP-PV.
\]

The operating-point bias is the valve position needed to hold 50 degrees
Celsius at zero error. Here it is 20 percent. The proportional gain
($K_p$) states how many requested valve percentage points are added for each
degree Celsius of present error. Proportional action is immediate. More gain
reduces offset and speeds the response, but a delayed process can begin another
correction before the earlier valve movement is visible at the sensor.

The integral contribution ($I$) represents accumulated past error as a valve
command, in percentage points. The integral gain ($K_i$) states how many valve
percentage points per degree Celsius per second are accumulated. Integral
action removes persistent error, but too much can cause overshoot, oscillation,
saturation, and difficult recovery. The integral time ($T_i$) is the proportional
gain divided by integral gain:

\[
T_i=\frac{K_p}{K_i}.
\]

The derivative gain ($K_d$) has units of valve percentage-point seconds per degree
Celsius. The derivative time ($T_d$) is derivative gain divided by proportional
gain. Derivative action can add damping by responding to measured rate, but it
also amplifies measurement noise. This slow thermal loop already meets its
requirements with proportional-integral control, so the lesson does not add
derivative action merely to make a three-term controller.

This controller uses the parallel positional form: bias, the present-error
contribution, the accumulated-error contribution, and the measured-rate
contribution are combined into one requested command. In symbolic form:

\[
MV_{\mathrm{requested}}=Bias+K_pe+I-K_d\frac{dPV}{dt},
\qquad \frac{dI}{dt}=K_ie.
\]

The applied manipulated variable ($MV_{\mathrm{applied}}$) is the command after the
controller's 0-to-100-percent limit. Saturation means the requested command
cannot be applied completely. Requested and applied values must therefore not
be confused.

Manual tuning changes one term at a time: establish bias, adjust proportional
action, add integral only when persistent error remains, and add derivative
only when measured damping evidence justifies its noise cost.

The ultimate gain ($K_u$) is the proportional-only gain that produces an
approximately constant-amplitude oscillation in one declared experiment. The
ultimate period ($P_u$) is the time between equivalent peaks. For this simulated
plant and operating point, the predicted critical values are $K_u = 22.24$
percentage points per degree Celsius and $P_u = 11.35$ seconds.

The Ziegler-Nichols historical proportional-integral rule calculates
$K_p = 0.45 \cdot K_u$, $T_i = P_u/1.2$, and $K_i = K_p/T_i$. Its
quarter-amplitude-decay objective expects one idealized deviation peak to
shrink toward one quarter of the preceding peak. This is aggressive and is not
a modern default; limits, noise, uncertainty, and actual requirements still
govern acceptance.

## Prediction

Manual proportional control uses a 20% bias and $K_p=2.0$. Let $\Delta y$ be
the final temperature rise. The plant gives
$\Delta y=K_{plant}\Delta MV=0.5(2e)=e$, while
$e=\Delta SP-\Delta y=2.5-\Delta y$. Solving
$\Delta y=2.5-\Delta y$ gives $\Delta y=1.25$ °C and $e=1.25$ °C. This derives
the proportional offset instead of merely observing it.

The first manual PI trial keeps $K_p = 2.0$ and uses $K_i = 0.05$, giving
$T_i = 40$ seconds. Predict that it will miss the 0.10-degree-Celsius recovery
requirement after the load. Change only integral gain to $K_i = 0.10$, giving
$T_i = 20$ seconds, for the second manual candidate.

Using $K_u=22.235$ and $P_u=11.35$ gives
$K_p=0.45K_u=10.006$, $T_i=P_u/1.2=9.458$ s, and
$K_i=K_p/T_i=1.058$ %/(°C·s). Predict a much faster
response and substantially more valve movement than manual PI.

## Implementation

The shipped Structured Text uses explicit proportional laws for manual P and
critical P. It uses one real `Control.PID` for manual PI and one real
`Control.PID` for historical PI. The essential manual configuration is:

```st
ManualPiParameters.ProportionalGain := LREAL#2.0;
ManualPiParameters.IntegralGain := LREAL#0.10;
ManualPiParameters.DerivativeGain := LREAL#0.0;
ManualPiParameters.Bias := LREAL#20.0;
ManualPiController.Configure(Parameters := ManualPiParameters);

ManualPiInput.Setpoint := Setpoint;
ManualPiInput.ProcessValue := ManualPiTemperature;
ManualPiInput.CycleTime := LREAL#0.1;
ManualPiApplied := ManualPiController.Update(Input := ManualPiInput);
```

The historical instance differs only in its declared proportional and integral
gains. Both use the same limits, anti-windup, cycle time, and initial state.

## Experiment in Control Studio

Control Studio exposes the four-candidate comparison. Test writes one reviewed
Boolean, `P1.ExperimentEnabled`, for 180.1 seconds. The project owns the common
50-to-52.5 °C setpoint and restores 50 °C. Native evidence owns the exact
100-second load event, the separately compiled $K_i=0.05$ trial, peak/period
calculation, and candidate decision. No candidate is applied. The deterministic
Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Common outlet-temperature setpoint (SP, degC); Manual proportional outlet temperature (PV, degC) and Manual PI outlet temperature (PV, degC); Manual proportional applied valve position (MV, %) and Manual PI applied valve position (MV, %), including units, limits, mode, and quality. The initial operating point is Common outlet-temperature setpoint = 50 degC, Manual proportional applied valve position = 20 %, Manual PI applied valve position = 20 %, Critical proportional applied valve position = 20 %, Historical PI applied valve position = 20 %, Manual proportional outlet temperature = 50 degC, Manual PI outlet temperature = 50 degC, Critical proportional outlet temperature = 50 degC. The unresolved baseline problem is: Proportional-only control retains offset, while the first deliberately slow integral setting does not recover from the common load inside the declared time.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Prefer the least aggressive candidate that meets final accuracy and overshoot requirements while retaining every failed trial and actuator-use trade-off.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **180.1**. The only writable signal is `P1.ExperimentEnabled`; it starts false and must restore false. Confirm common setpoint = 50 °C and every applied valve near 20%.
   - **Decision before action:** Before selecting **Review test**, record the 1.25 °C proportional offset and the derived manual/historical PI gains. Confirm every valve remains within 0–100% and every temperature within 40–60 °C. This is simulation-only; do not Apply.
   - **Successful evidence:** Select **Review test**, verify Boolean authority, timeout, false restoration, and stop conditions, then **Confirm and start**. Keep **Abort test** available. Observe all four responses under the common setpoint and the restored 50 °C setpoint.
   - **Recovery:** Abort for stale, invalid, nonfinite, mismatched, or out-of-range data. Confirm authority false, setpoint 50 °C, and safe valve outputs before retrying.
   - **Record:** Retain terminal state, restoration, all SP/PV/MV traces, gains, timing, noise, events, peak/period measures, response measures, valve travel, seed, and candidate state.

There is no shared Tune or disabled Validate stage in this lesson. The
project already contains the declared controllers; native executable evidence
performs the equivalent-condition comparison and leaves the preferred manual
PI candidate unapplied.

## Evidence and decision

### Measures and comparison

Define every decision measure before reading the result:

- Proportional late error is mean absolute setpoint-minus-temperature error
  from 80 to 100 seconds, in degrees Celsius.
- Rise time is the first 10-to-90-percent crossing duration, in seconds.
- Overshoot is peak temperature above the final setpoint divided by the
  setpoint change, in percent.
- Settling time is the elapsed time from the setpoint change until temperature
  enters and remains inside plus or minus 0.10 degrees Celsius.
- Load integral absolute error ($IAE$) is sampled absolute error accumulated from
  100 to 180 seconds, in degree-Celsius seconds.
- Cumulative valve movement is total variation: the sum of absolute successive
  applied-command changes, in percentage points. It can exceed 100 because it
  measures travel, not instantaneous valve position.

A run that does not meet the settling rule is reported as `Not settled`; its
last sample is not called steady-state error.

![Manual and historical heat-exchanger tuning](figures/manual-historical-tuning-comparison.svg)

The generated figure and its [complete retained trace](figures/manual-historical-tuning-comparison-trace.csv)
come from the executable simulation. The [decision summary](figures/manual-historical-tuning-comparison-summary.csv)
retains the exact gains, assumptions, results, and unapplied decisions.

### Validation

The executable simulation gives these decision results:

| Case | Qualified result |
|---|---|
| Manual P | 1.25 degrees Celsius late offset |
| Rejected manual PI, $K_i = 0.05$ | 0.34 degrees Celsius error at 180 seconds |
| Critical P | 14 alternating peaks; measured period 11.55 seconds; no accepted tuning |
| Manual PI, $K_i = 0.10$ | 0.08 degrees Celsius final error; 4.13 percent overshoot; 209.26 percentage points valve movement |
| Historical PI | 0.01 degrees Celsius final error; 3.00-second rise; 1,064.01 percentage points valve movement |

Both accepted PI candidates meet the final-error limit. Historical PI is much
faster, but its cumulative valve movement is more than five times the manual
candidate and therefore exceeds the declared requirement of at least twice the
manual movement. The critical case supplies at least six alternating peaks and
its measured period lies inside the required 9 to 14 seconds. The
manual candidate meets the no-more-than-5-percent overshoot criterion with
materially lower actuator activity, so it is preferred for this lesson. No
candidate is silently applied.

## Conclusion

The slow first manual integral setting proves that removing offset eventually
is not enough; recovery must meet a time requirement. Raising only integral
gain makes that causal change visible. The historical tuning responds quickly
because it inherits an aggressive objective from operation near the critical
oscillation. Its valve travel exposes the cost. One tuning formula is therefore
not a guarantee or a universal recommendation.

## Reason for the next lesson

Choose the least aggressive controller that satisfies declared accuracy,
response, robustness, and actuator requirements on retained evidence. The
critical gain and period belong to this plant, operating point, sampling,
limits, and experiment. Lesson 43 replaces sustained manual operation near
instability with a bounded relay experiment, while still requiring review and
leaving the resulting candidate unapplied.
