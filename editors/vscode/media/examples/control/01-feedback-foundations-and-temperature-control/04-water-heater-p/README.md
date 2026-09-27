# 5. Proportional control

In this lesson, you will make the heater-power command increase with the difference between the temperature setpoint and measured water temperature. You will then observe why this correction alone cannot hold the temperature setpoint while heat is continually leaving the system.

## The problem

The cold-room assessment combined hysteresis and dwell in a controller whose output still had only two states. The temperature moved between switching boundaries. We now need a rule for a heater whose power can vary continuously.

An electric heater can use intermediate power. We will make a large temperature shortfall produce a large heater command and a small shortfall produce a small command.

## What we need

We need to:

1. calculate how far the measured temperature is below the temperature setpoint;
2. convert that temperature difference into a heater command; and
3. reduce the command as the water approaches the temperature setpoint.

## The new idea

**Proportional control** makes the correcting command proportional to the
current difference between the desired and measured value. For this heater,
a larger temperature shortfall produces more heating.

Water stores thermal energy. The heater adds energy, while the surroundings and incoming colder water remove energy. If the heater adds energy faster than it leaves, the water temperature rises. If energy leaves faster, the temperature falls.

The temperature setpoint is 35 °C. Let:

- $e$ be the temperature error, in degrees Celsius;
- $SP$ be the temperature setpoint, in degrees Celsius; and
- $PV$ be the measured temperature, in degrees Celsius.

The error is the temperature setpoint minus the measured temperature:

$e = SP - PV$

This subtraction measures the temperature shortfall. When the water is colder than desired, the error is positive. A positive error must produce positive heater power because increasing heater power raises the water temperature.

We want the correction to have two properties:

1. zero error produces zero correction; and
2. each additional degree of error adds the same amount of correction.

These properties give a straight line through zero, so doubling the error doubles the correction. Its slope is called the **proportional gain**, written $K_p$. Let $u$ be the heater command in percent. The rule is:

$u = K_p e$

The gain tells us how much the heater command changes for each degree of error. This lesson uses $K_p=2$ percentage points per degree Celsius. This is a teaching choice that keeps the simulated command within its available range; it is not a tuned value for physical equipment.

Two percentage points means, for example, increasing a command from 20% to 22%. Thus one extra degree of temperature shortfall adds two points to the heater command.

The heater command ranges from 0%, which adds no heater power, to 100%, which requests the simulator’s maximum heater power. The heater cannot remove heat, so a negative calculated command is applied as 0%. A calculated command above 100% is limited to 100%; this experiment stays below that upper limit.

Proportional control has an important limitation. At a steady hot temperature, the heater must replace the energy continually leaving the water. But when the measured temperature equals 35 °C, the error is zero and this rule commands no heater power. The water must therefore settle below 35 °C, where a remaining positive error produces enough power to balance the heat loss. This difference after the temperature has settled is the **steady-state error**.

## Prediction

The simulation starts with a temperature setpoint of 35 °C and a measured temperature of 20 °C:

$e = 35\,^{\circ}\mathrm{C} - 20\,^{\circ}\mathrm{C} = 15\,^{\circ}\mathrm{C}$

With $K_p=2$ percentage points per degree Celsius:

$u = 2 \times 15 = 30\%$

Apply the same rule at two more temperatures:

| Measured temperature | Error | Predicted heater command |
| ---: | ---: | ---: |
| 20 °C | 15 °C | 30% |
| 30 °C | 5 °C | 10% |
| 35 °C | 0 °C | 0% |

Before running, also predict:

1. how the command will change as the water warms; and
2. how the temperature, error, and command will respond when colder incoming water removes more energy after 60 s.

## Implementation

**ST practice.** Inspect the control-error calculation and the unlimited proportional request recorded for the plot, using the `LREAL` variables in `Main.st`:

```st
ErrorC := Setpoint - ProcessValue;
RequestedHeaterPower := ProportionalGain * ErrorC;
```

`*` multiplies gain by error. Keep `Setpoint` at 35.0 and `ProportionalGain` at 2.0, then check the three temperature cases in the prediction table. As an arithmetic exercise, write a separate limiting decision that applies 0% below the lower bound, 100% above the upper bound, and the request between them. Keep requested and applied power separate. P recalculates its output from the current error; it does not accumulate an integral contribution.

The runnable project uses `Controller : Control.PID` with proportional gain 2,
integral and derivative gains zero, zero bias and 0–100% output limits.
`Configure` supplies this complete parameter record, `Initialize` sets the
startup histories once, and `Update` executes every 0.1 s. Its returned output
is the only assignment to the applied heater command:

```st
AppliedHeaterPower := Controller.Update(Input := Input);
```

The multiplication above is a diagnostic calculation; it does not drive the
heater. The block runs in Manual with a zero command before the experiment,
after completion and on abort. Enabling the experiment selects Auto and the
same block executes P control. Its rate bounds allow the full 0–100% change in
one scan, so they do not slow this experiment.

The controller reads one measured temperature at `%IL0` and writes heater
power to `%QL0`. The native simulator applies the one-second heater delivery
delay and immediate extraction through `%QL8` before publishing that temperature.
The controller contains no simulated temperature calculation. The lesson's
activation and timed extraction sequence remain an explicit experiment;
site limits, input quality and recovery need application-specific qualification.

## Experiment in Control Studio

1. Open this lesson's project, start **Simulator**, and open **Control Studio → Monitor → Response**. Display temperature setpoint, measured temperature and error in °C, heater command in percent, and proportional gain.
2. Confirm temperature setpoint 35 °C, measured temperature 20 °C, error 15 °C, proportional gain 2 percentage points per degree Celsius, heater command 0%, automatic control disabled, and `P1.ExperimentEnabled` false.
3. Write down the predicted 30% initial command. Test will start and retain its own recording when you confirm the experiment.
4. Open **Control Studio → Test**. Select **Observe without excitation**, leave the disabled amplitude at **0**, and set the maximum duration to **120.1 s**. This gives time to observe warming and the response to increased heat loss. All event times start at **Confirm and start**.
5. Select **Review test**. Starting sets `P1.ExperimentEnabled` to true so that the proportional rule controls heater power; ending disables the rule and sets power to zero. Keep the 35 °C setpoint and gain of 2 unchanged during the run.
6. Select **Confirm and start**. Confirm the first command is about 30%, then watch the error and command decrease as the water warms.
7. At 60 s, the experiment introduces colder incoming water, increasing heat loss. Observe the decrease in PV and the resulting increases in control error and heater command.
8. At completion, wait for restoration confirmation and **Complete recording**, then use **Validate → Save result**. Expand **Saved results**, select that revision and choose **Open** to inspect the retained response. Confirm `P1.ExperimentEnabled` false, automatic control disabled, and heater command 0%. The one-second delivery delay can keep temperature rising briefly after the command reaches zero; the water then cools.

Select **Abort test** to stop early or if temperature leaves 0 to 80 °C or the heater command leaves 0 to 100%; make these range checks yourself. Confirm heater command 0%, or stop the simulator from the debug toolbar if it is unresponsive. For a fair repeat, restart at 20 °C with the same initial controller state and 60 s disturbance.

The program latches `SafetyAbort` when measured temperature leaves 0–80 °C and commands zero heating. A later valid reading or disabling and re-enabling the experiment does not clear that latch. Stop and restart Simulator to restore the declared 20 °C/0% initial state before repeating; distinguish the zero command from the native actuator response and the delayed change in water temperature.

## Evidence and decision

The retained [simulation readings](figures/p-gain-comparison.csv) for gain 2 show:

| Time | Measured temperature | Error | Heater command |
| --- | ---: | ---: | ---: |
| First controlled sample | 20.00 °C | 15.00 °C | 30.00% |
| Before the increased heat loss | 24.29 °C | 10.71 °C | 21.43% |
| End of the controlled run | 22.14 °C | 12.86 °C | 25.71% |

The first command matches the calculated 30%. As the water warms, both error and heater command decrease. When the heat loss increases, temperature falls, so error and heater command increase.

The controller increases heating when the water becomes colder, but the water still ends below 35 °C. The remaining shortfall produces the heater power needed to replace continuing heat loss. The rule responds in the right direction, yet does not achieve the temperature setpoint.

## Conclusion

Proportional control makes the correction larger when the error is larger and smaller when the error is smaller. The gain determines how strongly the command responds to each degree of error.

For this heater, proportional correction alone needs a remaining temperature error to produce the power that replaces continuing heat loss.

## Reason for the next lesson

At the temperature setpoint, proportional correction becomes zero even though the heater still needs power to replace heat loss.

[PI temperature control](../05-water-heater-pi/README.md) adds a stored correction that changes while error remains, forming a PI controller that can supply holding power at zero error.
