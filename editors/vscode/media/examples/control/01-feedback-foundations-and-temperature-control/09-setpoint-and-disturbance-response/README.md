# 9. Setpoint and disturbance response

In this lesson, you will learn why a setpoint step changes the PI output before the water temperature changes, while an unmeasured thermal load must first affect the measured temperature. You will calculate the initial response in each case and assess both on the same heater.

## The problem

The heater is hot and settled at SP=PV=35 °C with 75% nominal holding output. A new setpoint of 38 °C needs more power. So does an added 30 kW heat load while SP stays 35 °C. Their final nominal output is the same, but the controller begins correcting them at different points in the physical response.

## What we need

We need separate ways to judge following a new target and rejecting an
unwanted heat load. The comparison must show how each event reaches the
controller and whether its temperature response is acceptable.

## The new idea

**Setpoint response** is the reaction to a changed desired value.
**Disturbance rejection** is the response to an unwanted physical influence
while that desired value stays fixed. They enter different places in the loop:
the setpoint enters the controller, while the heat load acts on the water.

Here $SP$ is desired temperature, $PV$ is measured temperature and
$e=SP-PV$ is error, all in °C. On each update, before output limiting and
anti-windup tracking, this PI controller calculates

$$
I_{new}=I_{old}+K_i e\Delta t,\qquad u_{request}=K_p e+I_{new}.
$$

$I$ is the retained integral contribution in percentage points, $u_{request}$
is requested heater output in percent, $K_p=2$ points/°C,
$K_i=0.5$ points/(°C·s), and $\Delta t=0.1$ s is the update interval.
A setpoint step from 35 to 38 °C changes e immediately at the next controller update, before water temperature can move. With I=75%, the first request is $75+2(3)+0.5(3)(0.1)=81.15\%$.

The disturbance is an imposed 30 kW heat-extraction load. It can represent the
extra heating demand from colder incoming water at unchanged flow; the local
model represents that equivalent load directly rather than simulating inlet
mixing separately. The PI controller receives SP and vessel PV; it has no measurement of this added heat load. At its onset PV is still 35 °C, so e and the request remain 0 °C and 75% until the vessel cools. Feedback then sees positive error and increases heating. The known 1 s delivery delay further separates a changed command from its thermal effect.

For this configured heater, the equilibrium relation is $T_\infty=20+0.2u$, with temperature $T_\infty$ in °C and output $u$ in percent: each output point adds 0.2 °C above the 20 °C inlet. The nominal steady output for 38 °C is $(38-20)/0.2=90\%$. The heater is rated at 200 kW at 100%, so one command percentage point corresponds to 2 kW. Replacing the additional 30 kW therefore requires $30/2=15$ more percentage points, taking the nominal 75% holding command to 90%. These are equilibrium calculations, not recorded responses. Equal final outputs do not imply equal transient traces.

The distinction is causal. SP is an input to the controller, so the setpoint trial can change the error without waiting for the water. The load is an input to the process, and the controller has no load measurement, so its first opportunity to react is a changed PV. A command can therefore be correct at the instant a disturbance begins while the temperature is already about to move. The record should preserve that ordering instead of treating the first deviation as a controller defect.

Set these requirements before running either experiment:

| Trial | Requirement through 60 s |
| --- | --- |
| SP 35→38 °C | Peak ≤39 °C; $\vert PV-38\vert≤0.2$ °C by 30 s and thereafter |
| SP 35 °C, load +30 kW | Minimum ≥33 °C; $\vert PV-35\vert≤0.2$ °C by 30 s and thereafter |

Applied output must remain within 0–100% in both trials. The peak is the highest temperature reached; the minimum is the lowest. Settling requires the temperature to enter the stated error band and remain there until the end of the record.

## Prediction

For the setpoint trial, identify the first changing signals and calculate the first request. For the load trial, what happens at the instant the 30 kW load appears? Why can feedback not prevent the first PV deviation from an unmeasured disturbance? Predict which trial has a peak requirement and which has a minimum requirement.

## Implementation

The prepared project calls the existing `Control.PID` with the same PI settings
and back-calculation as Lesson 8. The block owns the retained history and heater
command. The displayed P, equivalent I and unlimited request are observations
of that history; they do not run a second controller. With fixed gains,
proportional weight one and no derivative or feedforward, they give the PI
calculation above. Use those same settings for both cases. For each isolated calculation, reset I to 75% and use SP/PV pairs 38/35 °C and 35/35 °C. Reset I to 75% again and check SP 35 °C with PV 34.9 °C; the request becomes $75+2(0.1)+0.5(0.1)(0.1)=75.205\%$. Keep disturbance magnitude out of the feedback expression: it belongs to the process experiment.

The prepared `TrialKind` selects `0` for the target trial or `1` for the
load trial. Set its initial value in [Main.st](src/Main.st) before starting
the simulator for that trial. Restart between trials to reset all state; a
value written to runtime memory before a restart would be lost. The
program holds the hot reset while preparing, then performs only that one
selected event. Normal completion and abort switch the actual PID to Manual at 0% and remove
extraction. The native plant supplies one measured temperature at `%IL0` and
the preceding applied heater output at `%IL8`; a new request therefore appears
one observation before its applied readback. The plant retains the 1 s heater
delivery history while extraction enters immediately.

## Experiment in Control Studio

1. Leave `TrialKind : INT := 0;` for the target trial, then start **Simulator** at SP=PV=35 °C, I=75%, P=0%, requested/applied output=75%, no added load, and the preceding 1 s command history at 75%. Keep Kp=2 percentage points/°C, Ki=0.5 percentage points/(°C·s), the 0.1 s update and 1 s tracking time from Lesson 8.
2. In **Control Studio → Monitor → Signals**, select SP and PV with I and requested/applied output. Add the thermal-load signal for the load trial. Open **Response** to compare the measured temperature with its target. Leave Error unselected in this overview: its near-zero values would compress the absolute-temperature scale.
3. In **Control Studio → Test**, click **Review test** for the independent 60 s sequence, then select **Confirm and start**. Test automatically records the reviewed sequence and establishes time zero, with SP changing 35→38 °C at time zero. Do not add a load. Record first error/output changes, peak PV, and settling.
4. After Test reaches terminal restoration, use **Validate → Save result** to retain the target response. Stop Simulator, change the `TrialKind` initializer to `1`, restart and re-establish the complete 35 °C/75% state, including I and command history. Run a second independent 60 s test with SP fixed at 35 °C and a 30 kW load introduced at time zero. Do not change SP. Again use **Review test**, then **Confirm and start**; Test automatically records the reviewed sequence. Record first PV and output changes, minimum PV, and settling; after terminal restoration, save the second result.
5. Abort outside 0–80 °C, outside 0–100% applied output, or on unusable measurements. Select **Abort test**, cancel the load, disable heating and confirm 0% output. Stop the simulator if it is unresponsive. At normal completion also restore these stopped conditions; after the delivery delay, the water cools.

## Reusing the project

The delivered project contains its Control library dependency, 100 ms task,
signal mappings and native simulation configuration. To adapt it, map the
measured temperature and applied-output feedback to your real sensors, and map
the heater and extraction commands to the corresponding outputs. Set process
units, bounds and actuator limits for that equipment. The example latches an
out-of-range temperature fault and removes both commands. The runtime rejects
nonfinite typed input before ST executes, so that fault needs the runtime stop
policy rather than this program flag. Real I/O also needs site-specific
quality/freshness supervision and an independent stop path. This nominal simulator uses 0.1 s per controller update; a deployment with
variable intervals must supply the actual elapsed time, covered later in the
sampling lesson. The simulated results do not qualify a physical heater.

## Evidence and decision

In the prepared nominal simulation, the maximum 30–60 s error is about 0.0751 °C
for the target trial and 0.2120 °C for the load trial. The target trial meets
the 0.2 °C requirement; the load trial narrowly fails it. Keep that failure for
the next diagnostic lesson. Do not change the gains or widen the requirement
to make this baseline pass.

The setpoint trial should show SP and error changing before PV. The disturbance trial should show load changing before PV, then error and controller output. Use the timestamps, not visual ordering alone. Applied output must remain 0–100% in both trials. A 39.5 °C peak fails even if the response reaches 38 °C quickly; a good setpoint trial does not establish disturbance rejection.

To inspect error separately, use **Signals** to hide SP, PV and the output
traces, select **Error**, and return to **Response**. **Samples** shows the
selected values with their times and quality. Assess the 30–60 s interval,
excluding the later stopped state; restore SP and PV to compare temperatures.

For each trial, write the event time, first changed signal, first controller response, PV extreme, first time inside the required band, and whether it stayed there to 60 s. If the band is never reached, write “not reached” rather than estimating a settling time beyond the record. The 30 kW event is intentionally separate from the 38 °C step; combining them would change the capacity question and invalidate the comparison.

## Conclusion

PI responds directly to a setpoint step through SP in the error calculation. It rejects an unmeasured load only after the process changes PV. Both responses require separate records and separate decisions.

## Reason for the next lesson

A failed requirement does not identify its cause. [Loop diagnosis](../../09-industrial-control-structures/60-loop-diagnosis/README.md) checks measurement, actuator, capacity, and disturbance evidence before any gain change.
