# Positional, incremental, and combined PID realization

Course position: Lesson 26 of 84. Previous lesson:
[Digital realization](../servo-digital-realization/README.md). Next lesson:
Initialization and operating modes.

**Lesson objective:** Explain and verify why Control.PID uses positional
realization without integral action and incremental realization with integral
action.

This lesson uses simulation. Its results are not physical-plant evidence.

**Equipment:** Industrial positioning table and recirculating water heater.

## The problem

The equipment in this lesson is Industrial positioning table and recirculating water heater.

The encoder-controlled positioning table needs proportional control, while the
recirculating water heater needs proportional-integral control. Both use the
same shipped `Control.PID` function block. The choice examined here is how that
block updates its output state; it is not a choice between two product
controllers.

Lesson 25 showed that a digital controller executes equations at individual
updates and retains values between them. We cannot safely reuse one update
equation for P, PI, PD, and PID without checking what happens when error or a
gain remains constant.

## What we need

We need the controller to choose a recurrence that behaves correctly both with and without stored integral action. The learner must see why an unchanged-error proportional controller cannot rely on an output-increment equation that produces no new increment.

## The new idea

The **setpoint ($SP$)** is the desired table position in radians or desired water
temperature in degrees Celsius. The **process value ($PV$)** is the corresponding
measured position or temperature. The **error ($e$)** is the setpoint minus the
process value, in the same physical unit as that setpoint.

The **controller output ($u$)** is the actuator request: motor torque in newton
metres for the table or heater power in percent for the heater. The **sample
index ($k$)** identifies the current controller update. The current controller
output is `u[k]`; the retained output from the preceding update is `u[k-1]`.

The **current error ($e_{k}$)** is the current setpoint minus the current process
value. The **previous error ($e_{k-1}$)** is the error retained from the preceding
update. The **error change (Δe[k])** is the current error minus the previous
error.

$\Delta e_{k} = e_{k} - e_{k-1}$

The **sample interval ($T_{s}$)** is 0.02 seconds between updates. The **output bias
($u_{0}$)** is the actuator request at zero error when integral action is disabled.
The **proportional gain ($K_p$)** states how much output is requested per unit of
error. The **integral gain ($K_i$)** states how much output is added per unit of
error per second.

The **positional form** calculates the complete output from the present terms.
For the P controller in this lesson:

$u_{k} = u_{0} + K_p \times e_{k}$

The **incremental form**, also called the **velocity form**, adds calculated
increments to the previous output. For the PI controller in this lesson:

$u_{k} = u_{k-1} + K_p \times \Delta e_{k} + K_i \times T_{s} \times e_{k}$

At constant nonzero error, $e_{k} = e_{k-1}$, so $\Delta e_{k} = 0$. An incremental P
calculation with $K_i = 0$ then adds nothing. It can remain stuck at an arbitrary
retained output even when $K_p$ changes. Positional P instead recalculates the
complete output immediately.

When $K_i$ is nonzero, the integral increment keeps changing output while error
remains. Retaining the previous output also makes tracking, clamping
anti-windup, and bumpless gain changes practical for PI and PID.

The **combined form** is the shipped design: `Control.PID` uses positional form
when $K_i = 0$ and incremental form when $K_i$ is nonzero. There is no form switch
for the operator to set incorrectly. Positional and incremental describe the
update realization. Parallel, ideal, and series describe PID parameter forms;
they are a different subject and are not interchangeable names.

## Prediction

Hold table error at 0.5 rad. At 4 N m/rad, positional P requests:

$u = 4 N m/rad \times 0.5 rad = 2 N m$

Changing gain to 8 N m/rad predicts 4 N m. The rejected incremental-P
calculation starts at an arbitrary 1 N m. Because its error change is zero, it
adds zero and remains at 1 N m after the gain change.

For the heater, error is 15 °C, integral gain is 0.8 %/(°C s), and the sample
interval is 0.02 s. The expected integral increment is:

$K_i \times T_{s} \times e = 0.8 \%/({}^\circ\mathrm{C} s) \times 0.02 s \times 15 {}^\circ\mathrm{C} = 0.24\%$

Predict an immediate 2 N m positional-P gain response, no rejected
incremental-P response, and no PI output jump beyond the expected 0.24 percent
integral increment.

## Implementation

The actual lesson sets zero integral gain for the table and nonzero integral
gain for the heater, then invokes the same product function block:

```st
PositionParameters.IntegralGain := LREAL#0.0;
HeaterParameters.IntegralGain := HeaterIntegralGain;
PositionTorque := PositionController.Update(Input := PositionInput);
HeaterPower := HeaterController.Update(Input := HeaterInput);
```

The rejected incremental-P calculation is retained only as a diagnostic. It is
never connected to a plant output and is not another controller implementation.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Run the same gain change through Control.PID with zero integral gain on the positioning table, then repeat the unchanged-signal gain change with nonzero integral gain on the heater. The declared event lasts 30 seconds. It restores Table-position setpoint to 0 rad afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Table-position setpoint (SP, rad) and Heater-temperature setpoint (SP, degC); Measured table position (PV, rad) and Outlet-water temperature (PV, degC); Position motor torque (MV, N m) and Applied heater power (MV, %), including units, limits, mode, and quality. The initial operating point is Measured table position = 0 rad, Position motor torque = 0 N m, Outlet-water temperature = 20 degC, Applied heater power = 30 %. The unresolved baseline problem is: At unchanged nonzero error, the rejected incremental-P recurrence calculates zero output increment and remains at its arbitrary retained output after a proportional-gain change.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Let the shipped controller select positional P when integral gain is zero and incremental PI when integral gain is nonzero; do not expose a user form switch.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **30**. The observed lesson signal is **Table-position setpoint**. Control Studio observes the project's existing timed sequence and does not write a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to Proportional gain changes from 4 to 8 while error and plant feedback are held unchanged for one update. Confirm the live values are current and the reviewed plan stays inside Measured table position -4 rad to 4 rad; Position motor torque -10 N m to 10 N m; Outlet-water temperature 0 degC to 80 degC; Applied heater power 0 % to 100 %.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: positional_p_immediate_gain_change == 2 N m (tolerance 1e-9 N m); rejected_incremental_p_prediction_error >= 3 N m (tolerance 1e-9 N m); pi_gain_change_discontinuity_after_integral_increment <= 1e-9 % (tolerance 1e-9 %).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

The **immediate gain-change output difference** is the output after the gain
change minus the output before it, over exactly one controller update. The
**rejected incremental-P state error** is the absolute positional prediction
minus the rejected retained output at that update. The **PI output
discontinuity** is the observed PI output difference minus the expected
integral increment.

| Observation | Executable result | Required interpretation |
| --- | ---: | --- |
| Positional P torque | 2.000 → 4.000 N m | responds to changed $K_p$ |
| Rejected incremental-P output | 1.000 → 1.000 N m | stuck at arbitrary state |
| PI discontinuity after expected increment | 0.000% | retains prior output |

![Valid realization follows controller structure](figures/pid-forms.svg)

[Retained run](figures/pid-forms-run.csv) ·
[Plot data](figures/pid-forms-trace.csv) ·
[Decision summary](figures/pid-forms-summary.csv)

These measures are invalid if saturation or rate limiting is active; report the
limit state instead of attributing a bounded output to the unrestricted law.

### Validation

Repeat from identical initial state. Require the positional P output difference
to equal 2 N m, its prediction error to be within 0.000000001 N m, the rejected
state error to be at least 3 N m, and the absolute PI discontinuity after the
expected integral increment to be at most 0.000000001 percent. Then let both
simulator plants advance for 5 seconds. The retained run must contain all 250
updates for each plant, and must show finite signals, the selected realization,
and the applied limit state on every update.

## Conclusion

The two realizations are related equations, but they do not preserve the same
behavior for every controller type and state transition. Pure incremental P has
no term that corrects an arbitrary retained output at constant error. PI has a
nonzero integral increment and benefits from retaining output for practical
tracking and anti-windup behavior.

The valid engineering choice is therefore automatic: positional for P or PD,
incremental for PI or PID.

## Reason for the next lesson

Use `Control.PID` for all four controller types and let $K_i$ select its internal
realization. Do not add a user-selectable form parameter. Initialization,
manual, tracking, disabled, and transition behavior are still unresolved;
Lesson 27 tests them on a booster pump.
