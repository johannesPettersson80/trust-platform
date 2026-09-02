# Model-qualified PI tuning for three real processes

Course position: Lesson 44 of 84. Prerequisite: qualified feedback-relay autotuning.
Next: closed-loop poles and Routh-Hurwitz stability.

Equipment: Recirculating water heater, buffer tank, and continuous belt dryer.

**Lesson objective:** Calculate and validate Fast, Balanced, and Robust PI candidates from eligible models while keeping every candidate unapplied.

## The problem

The equipment in this lesson is Recirculating water heater, buffer tank, and continuous belt dryer.

This lesson controls three simulated pieces of industrial equipment with the
same shipped `Control.PID` function block: a recirculating water heater, a
buffer tank, and a continuous belt dryer. The heater and dryer settle after a
fixed input. The tank level keeps changing while inlet and outlet flow differ.

The controlled quantities are heater temperature in degrees Celsius, tank
level in percent, and dryer product dryness in percent. Their manipulated
variables are heater power, inlet-valve position, and dryer heater command,
each limited from 0 to 100 percent. This is Simulation evidence, not physical
plant evidence.

Lesson 43 estimated one critical point with a feedback relay. That evidence is
useful, but it does not tell us how deliberately choosing a slower response can
increase tolerance to model error. A validated process model lets us make that
speed-versus-robustness choice explicitly. It also prevents us from applying a
self-regulating formula blindly to an integrating tank.

## What we need

We need to convert each eligible process model into several explicit speed-versus-robustness choices and validate them under common conditions. A calculated candidate remains unapplied until its model range, actuator use, and uncertain response are acceptable.

## The new idea

The desired closed-loop time ($\lambda$) is the response-speed target in seconds.
A smaller value asks for a faster response and usually leaves less tolerance
for modelling error. A larger value asks for a slower, more robust response.

Self-regulating plant gain ($K$) is the steady output change divided by the
steady input change. The dominant time constant ($T$), in seconds,
describes the main response lag. Effective dead time ($L$), in seconds, describes
delay before the useful response appears. After defining those quantities, the
SIMC proportional gain is

$K_p = T / (K \cdot (\lambda + L))$

The integral time ($T_i$) is the time in seconds used to scale integral action.
For these first-order self-regulating models, $T_i = T$. The parallel integral
gain ($K_i$) used by `Control.PID` is then

$K_i = K_p / T_i$

An integrating process never reaches a new steady output while net input stays
nonzero. Its integrating rate gain (Kprime) is output change per unit input per
second, not a steady-state gain. For the tank, the eligible equations are

$K_p = 1 / (Kprime \cdot (\lambda + L))$

$T_i = 4 \cdot (\lambda + L)$

and again $K_i = K_p / T_i$.

SIMC means internal-model-control-inspired tuning of an ordinary feedback PI
controller. It is not model-predictive control: no online optimizer predicts a
future input sequence here.

## Prediction

The water heater model has $K = 0.20 {}^\circ\mathrm{C}/\%$, $T = 5.0 s$, and $L = 1.0 s$.
Its Fast, Balanced, and Robust numeric targets in seconds are 1.67, 3.33, and
6.67. The resulting `(Kp, Ki)` pairs are $(9.375, 1.875)$,
$(5.769, 1.154)$, and $(3.261, 0.652)$.

The buffer tank has $Kprime = 0.05 level-\%/(valve-\% \cdot s)$ and effective dead time
$L = 0.5 s$. That effective delay comes from the already taught reduction of
the physical 1.0-second valve lag. Fast, Balanced, and Robust use lambda values of 0.5,
1.0, and 2.0 seconds. Their pairs are $(20.000, 5.000)$, $(13.333, 2.222)$,
and $(8.000, 0.800)$.

The dryer has $K = 0.40 dryness-\%/heater-\%$, $T = 20.0 s$, and $L = 8.0 s$.
Its lambda values are 8.0, 16.0, and 32.0 seconds. Its pairs are
$(3.125, 0.156)$, $(2.083, 0.104)$, and $(1.250, 0.063)$.

Predict that Fast will normally have the shortest 10-to-90-percent rise time,
while Robust will move the actuator more gently and tolerate model error
better. This is a prediction to test, not a winner embedded in the PLC.

## Implementation

The shipped Structured Text uses three instances of the same controller. This
excerpt shows the heater path; tank and dryer use their own gains, bias,
setpoint, process value, requested output, and applied output.

```st
HeaterParameters.ProportionalGain := HeaterKp;
HeaterParameters.IntegralGain := HeaterKi;
HeaterController.Configure(Parameters := HeaterParameters);

HeaterInput.Setpoint := HeaterSetpoint;
HeaterInput.ProcessValue := HeaterTemperature;
HeaterInput.Mode := PIDMode#Auto;
HeaterInput.CycleTime := LREAL#0.1;
HeaterRequested := HeaterController.Update(Input := HeaterInput);
HeaterApplied := MAX(MIN(HeaterRequested, LREAL#100.0), LREAL#0.0);
```

Requested output is the bounded controller request. Applied output is the
command delivered to the simulated actuator. They are retained separately so
later actuator limitations cannot be hidden.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Calculate model-eligible SIMC PI candidates for three declared response targets. The declared event lasts 220 seconds. It restores Heater temperature setpoint to 20 degC afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Heater temperature setpoint (SP, degC) and Tank level setpoint (SP, %); Heater temperature (PV, degC) and Tank level (PV, %); Heater applied power (MV, %) and Tank applied inlet valve (MV, %), including units, limits, mode, and quality. The initial operating point is Heater applied power = 20 %, Tank applied inlet valve = 50 %, Dryer applied heater command = 30 %, Heater temperature = 20 degC, Tank level = 50 %, Dryer dryness = 40 %. The unresolved baseline problem is: A critical-point estimate does not expose a deliberate closed-loop speed-versus-robustness target or distinguish self-regulating from integrating tuning equations.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Retain all model-qualified candidates and validation evidence for review; do not select or apply a winner in the PLC program.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Setpoint step**, **Amplitude** = **5 degC**, and **Maximum duration (s)** = **220**. The affected signal is **Heater temperature setpoint**; it starts at 20 degC and must restore to 20 degC. Change only the response target; keep the same eligible plant, initial state, events, sample interval, and limits for Fast, Balanced, and Robust candidates.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Heater applied power 0 % to 100 %; Tank applied inlet valve 0 % to 100 %; Dryer applied heater command 0 % to 100 %; Tank level 0 % to 100 %.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Heater temperature setpoint to 20 degC, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** Continue only when Identify shows an accepted model. Set **Design method** = **SIMC** and set **Response** = **Balanced** for the first candidate.
   - **Decision before action:** Calculate the expected gains and predict the response/robustness trade-off before selecting **Generate candidate**.
   - **Successful evidence:** Generate the unapplied candidate and compare Response, controller output, Bode evidence, and margins using the same accepted model.
   - **Recovery:** If generation is disabled, return to Identify and accept a qualified independent model. If the method is rejected, retain the reason instead of substituting another formula.
   - **Record:** Save the candidate settings and explain why the trade-off is eligible or rejected without applying it.

4. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** Continue only after Tune shows an unapplied candidate, then select **Run required validation**.
   - **Decision before action:** Open **Requirement details** and use the predeclared comparisons: heater_final_window_absolute_error <= 0.25 degC (tolerance 0.01 degC); tank_final_window_absolute_error <= 0.25 % (tolerance 0.01 %); dryer_final_window_absolute_error <= 0.25 % (tolerance 0.01 %).
   - **Successful evidence:** Inspect Response, constraints, and robustness using the same retained model and conditions. Every requirement reports Pass, Fail, or Inconclusive with value, unit, limit, source, and reason.
   - **Recovery:** If validation is disabled, stale, or failed, return to the named prerequisite stage, correct the cause, and retain both rejected and replacement evidence; do not open **Review application** for a failed result.
   - **Record:** Select **Save result** only after source, units, tolerances, and restoration agree, then state the final disposition and failed requirement if any.

## Evidence and decision

### Measures and comparison

Define the measures before looking at the results:

- **Rise time** is the time from the first 10-percent crossing to the first
  90-percent crossing after the setpoint step, in seconds.
- **Settling** requires process value to remain within the declared band for
  the declared dwell. If it does not, report `Not settled`; do not use a final
  sample as steady-state evidence.
- **Final-window absolute error** is the mean absolute setpoint-minus-process-
  value difference from 210 through 220 seconds.
- **Cumulative applied-output movement** is actuator activity. Calculate the
  sum of absolute changes between consecutive applied-output samples and
  report it in percentage points.

Compare the nominal model, 20 percent more plant gain, and 50 percent more
effective delay. A response that is unstable, saturated, aborted, or Not
settled is ineligible for ranking by steady-state error.

![SIMC response targets on three process types](figures/model-based-tuning-comparison.svg)

The figure normalizes each process-value change so speed can be compared
without pretending the plants share physical units. The [complete nominal
trace](figures/model-based-tuning-comparison-trace.csv) retains every process
response and applied actuator command. The [decision summary](figures/model-based-tuning-comparison-summary.csv)
retains gains, rise times, uncertainty cases, and `applied: false`.

### Validation

First verify finite signals, limits, quality, and settling eligibility. Then
compare rise time, overshoot, final-window error, and cumulative applied-output
movement. Validate the load event separately from the setpoint step. No
candidate may pass merely because one metric looks good.

The test suite executes all nine nominal controller/plant combinations and the
gain and delay variations. It verifies bounded outputs and zero late error to
the declared tolerance. The selected settings remain unapplied.

## Conclusion

Fast uses more feedback gain because its desired closed-loop time is shorter.
Robust uses less gain because its larger response target allows more time for
unmodelled lag and delay. The integrating tank needs a rate-gain formula:
treating its rate gain as a static gain would produce a dimensionally wrong
controller. The load tests also show why setpoint speed alone is not a complete
tuning decision.

## Reason for the next lesson

You can now decide whether a validated self-regulating or integrating model is
eligible for SIMC PI tuning, calculate transparent response-target candidates,
and test them without silently applying gains. A model-derived tuning is still
not proof of stability under every uncertainty. Lesson 45 introduces explicit
stability reasoning before any physical application can be considered.
