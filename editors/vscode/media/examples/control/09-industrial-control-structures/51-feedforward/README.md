# 55. Feedforward control

You will use a measured heat-load disturbance to calculate an immediate change in heater command. You will explain why feedback is still needed and test feedforward when power delivery is delayed or the load measurement is inaccurate.

## The problem

An extra heat extraction can cool the vessel before ordinary PI sees an
error. We want to reduce that initial temperature excursion when a separate
measurement can tell us about the load sooner.

The electric heater has $C=50\ \mathrm{kJ/°C}$, $H=10\ \mathrm{kW/°C}$ to 20 °C, 200 kW at 100%, and a one-second command-to-delivered-power delay. Its balance is

$$
50\dot T=2u(t-1)-10(T-20)-D,
$$

where $u$ is applied command in percentage points and positive $D$ is immediate extra heat extraction in kW. PI feedback waits for PV to fall; a measurement of $D$ can increase the command without waiting for temperature error. Delivered heat still arrives one second later.

## What we need

We need to begin correcting a measured heat disturbance before it creates
a large temperature error. The action must account for physical heat demand
while feedback remains available to correct measurement and model errors.

## The new idea

**Feedforward control** calculates an action from a measured disturbance or
reference before output error is needed to trigger it. Here a measured heat
extraction lets us request replacement power before temperature falls. PI
still corrects errors in that prediction and the actual delivery remains delayed.

Use a measured or calculated estimate of the extra heat-removal rate. For an inlet disturbance at unchanged mass flow, that estimate can come from mass flow times specific heat times the fall in inlet temperature from its nominal value. In this exercise the load estimate is supplied directly. Keep it separate from the simulator’s actual D when checking bias or a sensor fault.

At unchanged temperature, each command point supplies 2 kW. A measured 30 kW extraction therefore requires

$$
u_{\mathrm{FF}}=\frac{D_m}{2}=15\ \text{percentage points}.
$$

Here $D_m$ is the measured estimate in kW, distinct from actual extraction $D$.
The full request is $u_{\mathrm{PI}}+u_{\mathrm{FF}}$, then limits and tracking
are applied. At 35 °C with a 30 kW disturbance, total heating must be 180 kW,
or 90%. With correct feedforward, I can remain at 75%. If the estimate is
36 kW, feedforward requests 18 points and PI must eventually reduce its
contribution to 72%. The total is still 90%. The term is a command estimate,
not applied heat; the one-second delivery delay still permits an initial PV drop.

For a 30 kW step, if no extra power is delivered for the first second, the unavoidable initial drop from 35 °C is

$$
3\left(1-e^{-1/5}\right)=.5438\ ^\circ\mathrm C.
$$

An upstream load measurement one second ahead could shift the feedforward request earlier; that is a different timing case and must be identified as a paper comparison. If the load estimate is 36 kW, feedforward is 18 points, a 20% overestimate; PI must trim the residual.

## Prediction

Predict the immediate 15-point feedforward request, the one-second-delay initial drop of approximately .5438 °C if no upstream lead exists, and the nominal 90% total command. Predict that a 36 kW estimate gives 18 points and requires feedback correction. Predict that invalid load data should disable only feedforward while healthy PV PI continues; invalid PV follows Lesson 28.

## Implementation

Read measured estimate $D_m$ with validity and acquisition time. Calculate
$u_{\mathrm{FF}}=D_m/2$, form the unlimited full PI-plus-feedforward request,
then limit and track it:
$I_{next}=I_{trial}+(dt/T_t)(u_{applied}-u_{request})$.
Write these calculations in ST using separate named variables. The difference
includes feedforward because the actuator limits the complete request. Keep
actual extraction, its estimate, feedforward, P, I and requested/applied command distinct.

When load data becomes invalid or stale while PV is healthy, disable feedforward and preload the integral so $I=u_a-P-u_{\mathrm{FF,new}}$ preserves the current output. Do not restore a nonzero heat command after invalid PV; use Lesson 28's fault path. A measured load has no value if its timestamp is too old to represent the current extraction.

## Experiment in Control Studio

Start at SP=PV=35 °C, applied command 75%, integral output 75%, proportional output 0%, command history 75%, $dt=.1$ s, $\beta=1$, filter bypass, and the selected PI gains. Use tracking time 1 s. Declare requirements before comparing PI-only with PI plus feedforward: for a 30 kW step at time 0, minimum PV ≥33 °C and $|error|\le.2$ °C from 30–60 s, with command 0–100%. The calculated final command is a model check, not an additional temperature requirement.

The calculation reuses Lessons 11, 13, 25, 26, 27 and 28: energy balance, PI limits and lifecycle/measurement validity. Use a healthy PV; if PV itself is invalid, Lesson 28's zero-heat handling applies.

1. Reset the hot state and display true/model temperature, PV, $D$, load validity/time, feedforward, P, I, full/requested/applied power and delayed command.
2. Run PI-only and PI+feedforward as separate 60 s tests. At **Confirm and start** set $D=30$ kW immediately; keep SP=35 °C and all controller settings identical. Record time zero from that action.
3. Repeat a separate normal biased-load case with $D$ estimate 36 kW, then a load-sensor-invalid case with healthy PV. Keep actual extraction at 30 kW in each case. In the invalid case, make the load sensor invalid at 5 s and keep it invalid until the 60 s end; disable feedforward within one .1 s update. Require the transfer itself to change output by at most .1 percentage point, separately from the next ordinary PI update. In the invalid case verify feedforward off and output continuity; do not call it a PV-fault test.
4. Stop for temperature outside 0–80 °C, applied command outside 0–100%, invalid PV, invalid clock, or unresponsive monitor. At normal end set $D=0$, disable feedforward, command zero heat, wait for restoration and fully reset before each case.

## Evidence and decision

Compare minimum PV, 30–60 s error, final nominal output, first request timing and PI correction. PI-only and PI+FF must start from identical full resets. A feedforward trace cannot claim immediate cancellation when delivery delay is one second; compare its first-second drop with .5438 °C. This calculation assumes that the disturbance begins at an update; additional detection delay would increase the drop.

A 36 kW estimate demonstrates calibration uncertainty: the 18-point correction is not automatically correct. An invalid load sensor should cause a controlled transition to feedback-only, while invalid PV requires zero heat and fault recovery. Feedforward qualifies only if the fixed requirement is met and its uncertainty/timing benefit is demonstrated.

## Conclusion

Feedforward uses a measured cause to anticipate heater demand, while PI corrects temperature error and model mismatch. The heater balance fixes the sign and units, and the delivery delay sets a real lower bound on how early the water can respond.

## Reason for the next lesson

Feedforward can act from a measured disturbance, but it cannot anticipate an unmeasured one. In a jacketed vessel, the jacket temperature may reveal a heat disturbance before the vessel temperature does. That intermediate measurement provides another way to begin correction earlier.

[Cascade control](../52-cascade/README.md) introduces the jacket’s thermal store and closes an inner loop around its temperature, while an outer loop regulates the vessel.
