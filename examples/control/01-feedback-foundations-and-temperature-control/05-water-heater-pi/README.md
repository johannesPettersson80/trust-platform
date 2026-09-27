# 6. PI control

In this lesson, you will add integral action to remove the steady-state temperature error observed with proportional control. You will calculate how the integral contribution changes and explain how the heater can maintain a nonzero output when the measured temperature equals the setpoint.

## The problem

The proportional controller reduces its command as the water warms. At 35 °C its error is zero, so P commands 0%, even though heat continues to leave the vessel. The water therefore settles below the setpoint, where the remaining error creates the holding power.

## What we need

We need a heating contribution that can remain when the current error
reaches zero, so it can replace continuing heat loss. It must increase while
a positive error persists and decrease when the water is too warm.

## The new idea

**Proportional-integral control**, or **PI**, adds a stored contribution based
on accumulated error to the immediate proportional correction. The stored
contribution can keep supplying heat when the current error becomes zero.

Keep the proportional gain of 2 percentage points/°C. The setpoint is 35 °C and the initial measured temperature is 20 °C, so the error is 15 °C and the proportional command is 30%.

Now add a contribution that accumulates while the error remains. Choose an **integral gain** of 0.5 percentage points/(°C·s). With a constant 1 °C error, this adds 0.5 percentage points to the integral contribution each second. With a 15 °C error, it adds 7.5 points per second, or 0.75 point during a 0.1 s controller scan.

Start this contribution at zero. After the first update it is 0.75%, making the complete request 30.75%. If the error is still 15 °C on the next update, another 0.75 point gives an integral contribution of 1.50% and a request of 31.50%. Each increment is added to the previous value; the contribution does not start from zero on each scan.

The repeated calculation approximates the **integral of the error**. During one interval, assume the error is constant at its latest measured value. Multiplying error by elapsed time gives the area of that rectangle under the error-versus-time curve. Adding the areas accounts for both the size of an error and how long it persists.

Let $e=SP-PV$ be error in °C, $\Delta t$ the scan interval in seconds, and $K_i$ the integral gain in percentage points/(°C·s). The integral contribution $u_I$ is expressed in percent of the heater-command scale. Its change in one scan is

$$
\Delta u_I=K_i e\Delta t,
$$

and the updated contribution is

$$
u_{I,\mathrm{new}}=u_{I,\mathrm{old}}+\Delta u_I.
$$

This state is a number retained in the program, not thermal energy stored in the water. Multiplying $K_i$ by $e\Delta t$, whose units are °C·s, gives a change in percentage points.

Add the updated integral contribution to the proportional contribution $u_P=K_p e$ to obtain the complete requested output $u_r$:

$$
u_r=u_P+u_I=K_p e+u_I.
$$

As the positive error decreases, P decreases while I continues increasing in smaller increments. The total request need not increase throughout warming, because the two contributions change in opposite directions.

At zero error, P becomes zero and the next integral increment is zero. The accumulated contribution remains. For example, if I has reached 75% when the measured temperature reaches 35 °C, the next calculation gives P = 0%, no change in I, and a 75% heater request. Maintaining the temperature also requires this delivered heating to balance the actual heat loss.

A persistent positive error would keep increasing I. While additional heater power is available, the below-setpoint steady temperature observed with P alone therefore cannot persist. If the water rises above the setpoint, the error becomes negative and I decreases. A brief crossing of the setpoint is not the same as settling there.

## Prediction

Calculate the first two updates for SP 35 °C, PV 20 °C and $u_I=0$. Then predict what happens to P, each new integral increment, and total request as the water warms. At PV=35 °C, does the integral state disappear? At PV=36 °C, what is the next increment? After a heat-load increase, must the eventual holding command be larger or smaller?

## Implementation

**Arithmetic practice.** Use `LREAL` values to work through the accumulation
calculation below. This explains PI; the runnable project delegates its
controller state and output to `Control.PID`.

```st
Error := Setpoint - ProcessValue;
ProportionalContribution := ProportionalGain * Error;
IntegralIncrement := IntegralGain * Error * CycleTime;
IntegralContribution := IntegralContribution + IntegralIncrement;
RequestedHeaterPower := ProportionalContribution + IntegralContribution;
```

Keep `ProportionalGain := 2.0`, `IntegralGain := 0.5` and `CycleTime := 0.1` for this fixed 0.1 s interval. The integral assignment reads the previous value on its right-hand side and stores the updated value for the next calculation. Initialize I once before the sequence. Check fixed readings: after two 15 °C updates the state is 1.50%; at zero error it is still 1.50%; at −1 °C the next increment is −0.05 point, leaving 1.45%, while P is −2% and the request is −0.55%, applied as 0%. These are uncorrected arithmetic values. The negative request introduces an output limit; the real library also corrects its retained state when a limit acts, a behavior examined in the following lessons.

The runnable `Main.st` instantiates `Controller : Control.PID` with the same
Kp and Ki, zero derivative gain, setpoint weight one and 0–100% output limits.
It calls `Configure`, initializes the actual block at the start of each warming
experiment, and obtains applied power only from `Controller.Update` every
0.1 s. Manual commands zero before enabling and after completion or abort.

The block stores nominal output and previous measurements. At the cold start,
the initialization records the measured SP/PV of 35/20 °C, a nominal demand of
30% corresponding to P with zero I, and the previous applied output of 0%.
The first update adds 0.75 point and applies 30.75%. This is the intended
startup step for this exercise; it does not initialize the block again on
every enabled scan.

The plotted `NominalHeaterPower` comes from the actual block's `GetState`
snapshot after its internal limit correction. Its `NominalOutput` field is
not a pre-limit request. With this fixed gain, weight one and no derivative or
feedforward, the displayed integral contribution is that retained nominal
output minus P. This diagnostic calculation does not maintain a second
integrator or drive the heater. The warming and load experiment keeps limits
inactive, so nominal and applied output agree throughout the comparison.

From this lesson's directory, check the runnable project with
`trust-runtime check --project .`. Run the separate arithmetic exercises with
`trust-dev test --project tests`. They retain the worked calculations above;
the actual controller and plant are exercised by the Control Studio experiment.

The measured water temperature arrives directly at `%IL0`; the controller
writes heater power to `%QL0`. The native simulator applies the heater's
one-second delivery delay independently of the extraction command on `%QL8`.
There is no simulated temperature calculation in the controller program.

## Experiment in Control Studio

1. Start **Simulator** for the heater project and open **Control Studio → Identify**. Choose **Project experiment** from **Method**, then use **Signals** to select SP, PV and error in °C, plus P, I, retained nominal controller output and applied output in percent.
2. Confirm SP 35 °C, PV 20 °C, zero I, output 0%, automatic control disabled, and the same nominal heat-loss condition as the preceding proportional-control lesson.
3. Before running, require PV to stay within 34.8–35.2 °C during the final 10 s before the 60 s heat-load change and during the final 10 s before shutdown. This ±0.2 °C band is the accuracy requirement for this comparison. Select **Identify → Method → Project experiment** with its declared 120.1 s duration, allowing time to observe warming and recovery. Select **Review project test**, then **Confirm and start**. Test starts its own recording and enables PI with zero I. At 60 s the experiment applies the same heat-load event used in Lesson 5. All event times begin at **Confirm and start**.
4. Watch P decrease while I accumulates. Assess both stated temperature windows. Record the highest temperature separately to identify any overshoot, meaning a temperature above the setpoint.
5. At completion, wait for **Project test completed. Outputs restored.** Inspect the complete recording on Identify, using **Signals** to select the quantities you need. Completion or abort disables control and clears I; confirm applied output 0%. The one-second delivery delay can keep temperature rising briefly after the command reaches zero; the water then cools. Restart at 20 °C with zero I and the original heat-load condition before repeating.

Abort if PV leaves 0–80 °C, applied output leaves 0–100%, or readings stop updating. Confirm 0% output, or stop the simulator if it is unresponsive. These are exercise limits.

The program latches `SafetyAbort` when measured temperature leaves 0–80 °C and commands zero heating. A later valid reading or disabling and re-enabling the experiment does not clear that latch. Stop and restart Simulator to restore the declared 20 °C/0% initial state before repeating; distinguish the zero command from the native actuator response and the delayed change in water temperature.

## Evidence and decision

The uncorrected arithmetic check should show I values 0.75%, 1.50%, 1.50%, then 1.45% for the stated fixed readings. In the executed warming trace, P should fall as error falls, while the equivalent I derived from the actual block should continue increasing as long as error is positive. Near 35 °C, P should be near zero while I supplies the continuing heating.

Compare the same 60 s relative heat-load event with Lesson 5. A crossing of 35 °C does not establish settling. Accept the PI result only if the stated final temperature band is met in the specified windows and the output remains applied within 0–100%. After the load event, the required holding contribution should increase because more heat must be replaced.

## Conclusion

Integral action accumulates error over time. Its contribution can remain nonzero at zero error, so PI can supply the power needed to balance continuing heat loss and remove the proportional controller's steady-state temperature error.

## Reason for the next lesson

This experiment kept the demand within the heater's available range. If a
controller does not account for an actuator limit, integral demand can keep
accumulating while the heater is already at 100%. [Integral windup](../06-water-heater-saturation-windup/README.md)
deliberately creates that mismatch, separates requested and applied output,
and shows why the retained demand delays recovery.
