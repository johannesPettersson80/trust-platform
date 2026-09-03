# Water-heater proportional control

Course position: Lesson 4 of 84. Previous lesson:
[Hysteresis and dwell protection](../03-cold-storage-hysteresis-dwell/README.md).
Next lesson: [Operating-point bias](../05-water-heater-bias/README.md).

**Lesson objective:** Compare three proportional gains under identical
conditions, run the selected candidate on the simulated heater, and explain why
the best acceptable proportional setting still leaves steady-state error.

This lesson uses simulation. Its results are not physical-plant evidence.

## The problem

An electric recirculating water heater must deliver water at a chosen outlet
temperature. A sensor measures that temperature, a modulating element supplies
0–100 percent heat, and increased cold-water flow removes more heat.

The simulation starts at 20 °C with the heater off. Temperature must remain
within 0–80 °C; the safe state is manual mode with 0 percent heat. One perfectly
mixed thermal body and a fixed flow-load change replace the gradients, varying
flow, sensor error, and changing losses of a physical heater.

The previous cold-room controller deliberately cycled between fully on and off.
This heater can modulate continuously, so we now test whether output
proportional to temperature error provides tighter control.

## What we need

We need a continuously variable heater command that becomes stronger as the measured water moves farther from its desired temperature. We must then find how much response speed can be gained before overshoot, oscillation, or actuator effort becomes unacceptable.

## The new idea

The **setpoint ($SP$)** is the desired outlet-water temperature. The **process
value ($PV$)** is the outlet-water temperature measured by the sensor. Both are
measured in degrees Celsius (°C).

The **error ($e$)** is the setpoint minus the process value. A positive error
means the measured water is colder than desired. A negative error means it is
hotter than desired.

$e = SP - PV$

The **proportional gain ($K_p$)** states how much heater output is requested for
each degree Celsius of error. Its unit is percent heater power per degree
Celsius (%/°C).

The **requested manipulated variable ($MV_{\mathrm{requested}}$)** is the heater-power
request calculated before a physical output limit is applied. It is measured in
percent. The runnable project maps this requested manipulated variable
($MV_{\mathrm{requested}}$) to `%ML16`, so Monitor can show the controller's
unclamped request beside the applied heater output.

$MV_{\mathrm{requested}} = K_p e$

The **applied manipulated variable ($MV_{\mathrm{applied}}$)** is the heater-power command
after the **clamp** has limited the request to the heater's physical range.

$MV_{\mathrm{applied}} = \operatorname{clamp} \left(MV_{\mathrm{requested}}, 0\%, 100\%\right)$

Percent per degree Celsius multiplied by degrees Celsius gives percent heater
power. **Closed-loop feedback** means the sensor returns the latest process
value after every update; because heating raises temperature, positive error
must request positive power.

The heater can add heat but cannot actively remove it. Negative error therefore
produces 0 percent applied output rather than a negative heater command.

For this zero-bias controller with a 0–100 percent output range, the
**proportional band ($PB$)** is the error range that moves the unclamped heater
request from 0 to 100 percent. It is measured in degrees Celsius.

At the lower end, zero error gives `Kp × 0 °C = 0%`. At the upper end, solve
`100% = Kp × e` by dividing both sides by $K_p$, which gives
`e = 100% / Kp`. The distance between those two errors is therefore:

$PB = \frac{100\%}{K_p}$

The numerator is the 100-percent output span. Dividing percent by percent per
degree Celsius leaves degrees Celsius, which is why proportional band is a
temperature-error width.

**Damping** describes how repeated rises and falls die away. Because the
thermal process responds with lag, its temperature has not yet shown the full
effect of a previous heater command when the next command is calculated.
Excessive proportional gain can therefore cause repeated over-correction.

**Steady-state error** is the remaining difference after a response has
settled. This heater is a **self-regulating thermal process**: at constant input
it approaches a constant temperature, but holding that temperature above the
surroundings still requires nonzero output.

The **required steady heater output ($MV_{\mathrm{required}}$)** is the output that balances
heat loss at the desired operating condition. The **steady-state error ($e_{\mathrm{ss}}$)**
is the error required to produce that output with zero bias. When the controller
is not saturated and the response has settled:

$MV_{\mathrm{required}} = K_p e_{ss}$

In plain notation, this is `MV_required = Kp × e_ss`. To isolate $e_{ss}$,
divide both sides by $K_p$. The gain cancels on the right, which gives:

$e_{ss} = \frac{MV_{\mathrm{required}}}{K_p}$

In plain notation, the result is `e_ss = MV_required / Kp`.

This relationship explains the offset only after settling and only when the
requested output is not clamped. It does not prove that any final sample is an
equilibrium.

We also need measures that can reject a gain which lowers average error by
creating repeated movement. For samples $k$ in the declared 60-to-120-second
window, **cumulative travel** adds the magnitude of every change:

$\mathrm{travel} = \sum_k |\mathrm{value}_k - \mathrm{value}_{k-1}|$

The absolute value makes upward and downward motion add instead of cancel. For
PV, the result is cumulative process-value travel in degrees Celsius. For
applied MV, it is cumulative output travel in percentage points; repeated
motion can therefore total more than the instantaneous 0-to-100-percent span.

Before calling a response settled, inspect its final 10-second window. Define
the final PV and applied-output ranges as maximum minus minimum:

$PV_{\mathrm{range,final}} = PV_{\mathrm{max,final}} - PV_{\mathrm{min,final}}$

$MV_{\mathrm{range,final}} = MV_{\mathrm{max,final}} - MV_{\mathrm{min,final}}$

In plain notation these are `PV_range,final = PV_max,final - PV_min,final`
and `MV_range,final = MV_max,final - MV_min,final`.

The response is settled only if the first is at most 0.10 °C and the second is
at most 1.0 percentage point. Only then calculate final-window mean absolute
error:

$e_{\mathrm{final}} = \operatorname{mean}(|SP-PV|), \quad 110\,\mathrm{s} \le t \le 120\,\mathrm{s}$

In plain notation, `e_final = mean(|SP - PV|)` over 110 to 120 seconds.

Absolute error prevents values above and below SP from cancelling. The mean
puts candidates with the same number of retained samples on a common scale.
We call this a settled-error estimate, not proof of exact equilibrium.

**Validity prerequisite:** all retained timestamps must increase, all SP, PV,
requested MV, and applied MV values must be finite and synchronized, every run
must use the same plant, initial state, load event, sample interval, duration,
limits, and seed, and applied MV must remain between 0 and 100 percent. An
invalid or unrestored run is not ranked.

**Decision direction:** a candidate is eligible only if it settles, cumulative
PV travel is at most 3.0 °C, and cumulative applied-output travel is at most
20 percentage points. Among eligible candidates, select the smallest
final-window mean absolute error. The gain-40 run is an intentionally aggressive
diagnostic and shall be rejected if reduced damping prevents settling or
violates either travel limit. These simulation criteria are not physical heater
approval limits.

## Prediction

The test setpoint is 35 °C and the initial process value is 20 °C. The
initial error is therefore:

$e = 35\,{}^\circ\mathrm{C} - 20\,{}^\circ\mathrm{C} = 15\,{}^\circ\mathrm{C}$

With a proportional gain of 2 %/°C, the initial heater request is:

$MV_{\mathrm{requested}} = 2\,\frac{\%}{{}^\circ\mathrm{C}} \times 15\,{}^\circ\mathrm{C} = 30\%$

At 8 %/°C the request is `8 %/°C × 15 °C = 120%`, so the applied output is
clamped to 100 percent. For gain 2, `PB = 100% / (2 %/°C) = 50 °C`. For gain
8, `PB = 100% / (8 %/°C) = 12.5 °C`.

If the heater needs 50 percent steady output, the illustrative equilibrium
calculation gives `e_ss = 50% / (2 %/°C) = 25 °C` at gain 2 and
`e_ss = 50% / (8 %/°C) = 6.25 °C` at gain 8.
This is an illustrative equilibrium, not the plant's measured holding power.
Predict that gain 8 will reduce offset and that an intentionally excessive gain
of 40 %/°C will cause repeated temperature and output movement.

## Implementation

The executable uses `Control.PID` with integral and derivative gains disabled:

```st
Parameters.ProportionalGain := ProportionalGain;
Parameters.IntegralGain := LREAL#0.0;
Parameters.DerivativeGain := LREAL#0.0;
Controller.Configure(Parameters := Parameters);
RequestedManipulatedValue := ProportionalGain * (Setpoint - ProcessValue);
ManipulatedValue := Controller.Update(Input := Input);
```

`RequestedManipulatedValue` exposes the proportional calculation before its
limit. `ManipulatedValue` is the bounded applied output. `Control.PID` performs
the clamp. The program reconfigures the controller before every update. The
native three-gain evidence test uses the addressed gain setting to run each
candidate from a fresh initial state; the Control Studio procedure below runs
the shipped selected `Kp = 8 %/°C` candidate.

## Experiment in Control Studio

This procedure runs the shipped `Kp = 8 %/°C` candidate for one 120-second
Simulation experiment. Test runs the shipped Kp 8 candidate; the retained
three-gain comparison is generated separately by the lesson's native evidence
test from the same Structured Text, plant, initial state, load event, sample
interval, duration, limits, and seed. Control Studio owns the reviewed Boolean
`P1.ExperimentEnabled` authority and restores it disabled. The project derives
Auto mode and owns the scheduled
cold-water load event. The deterministic seed is 20260824.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Outlet-temperature setpoint (SP, degC); Outlet-water temperature (PV, degC); Requested heater power; and Applied heater power (MV, %), including units, limits, mode, and quality. Confirm SP = 35 °C, PV = 20 °C, MV = 0%, `Kp = 8 %/°C`, and Auto disabled. The unresolved baseline problem is: Low proportional gain leaves a large load-dependent outlet-temperature offset.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Compare moderate and intentionally aggressive proportional gains while preserving the identical plant, initial state, load event, limits, duration, sample time and seed; reject a gain that reduces offset by destroying damping.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **120.1**. The reviewed authority is **Experiment authority** (`P1.ExperimentEnabled`). The project derives Auto and owns the 35 °C setpoint, `Kp = 8 %/°C`, and 60-second load event.
   - **Decision before action:** Before selecting **Review test**, predict an initial 120% request clamped to 100%, then decreasing heat as PV rises; at 60 s the colder flow load should increase error and heater demand. Confirm the limits are Outlet-water temperature 0 °C to 80 °C and Applied heater power 0% to 100%.
   - **Successful evidence:** Select **Review test**, verify **P1.ExperimentEnabled**, the 120.1-second timeout, stop conditions, and false restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, Auto becomes enabled, and the terminal state reports completion with authority restored false and applied heater power restored to 0%.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test. Confirm Auto is false and applied heat is 0% before leaving the lesson.
   - **Record:** Compare requested and applied heat, PV, the load response, and restoration with your prediction. Retain the terminal state and seed **20260824**. Use the predeclared measures above when reading the retained three-gain comparison; do not present its table as a newly executed Control Studio run.

## Evidence and decision

### Measures and comparison

Apply the validity, settling, travel, and selection definitions declared before
the experiment. Compare desired and measured temperature in the upper plot,
then requested and applied heater power in the aligned lower plots. Requested
power may exceed the physical range; applied power may not.

![Water-heater responses for three proportional gains](figures/p-gain-comparison.svg)

| Proportional gain | Settled mean absolute error | Cumulative PV travel | Cumulative applied-output travel |
| ---: | ---: | ---: | ---: |
| 2 %/°C | 12.9 °C | 2.0 °C | 4.2 pp |
| 8 %/°C | 6.9 °C | 1.0 °C | 8.8 pp |
| 40 %/°C | Not settled | 22.7 °C | 908.6 pp |

The last gain-40 sample cannot prove equilibrium, so no steady-state error is
reported.

### Validation

Reproduce the predeclared validity, settling, travel, and selection checks from
the retained samples rather than trusting the rounded table.

The gains of 2 and 8 settle and satisfy both travel limits. The gain of 8 has
the smaller settled mean absolute error, so it is the best tested proportional
setting. The gain of 40 does not settle and violates both travel limits, so it
must be rejected.

The selected gain still leaves 6.9 °C steady-state error.

## Conclusion

At zero error this zero-bias controller requests zero power. The self-regulating
heater still loses heat, so the water must remain below SP to request the
holding power that balances that loss.

Increasing proportional gain reduces the required error, but the excessive
case shows why gain cannot be increased without limit. A useful setting must
balance temperature accuracy against repeated process movement and actuator
effort.

## Reason for the next lesson

This conclusion applies to a self-regulating plant that needs nonzero input at
the target; it is not a universal rule for integrating plants. The next lesson
measures the holding power, adds it as **operating-point bias**, and tests a
changed load.
