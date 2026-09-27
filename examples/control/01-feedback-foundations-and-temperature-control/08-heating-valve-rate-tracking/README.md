# 30. Actuator tracking

You will control vessel temperature with a moving hot-water valve. You will explain how limited travel speed can cause integral windup even while the requested position stays within 0–100%, and correct the integral state using measured stem position.

## The problem

A PI controller can request a new valve position faster than the valve can move. The request may remain inside the available 0–100% position range while the measured stem is still travelling. During that interval, ordinary integration can continue adding correction because the process value has not responded yet. When the temperature setpoint is reduced, the stored correction may keep the valve farther open than the new setpoint requires.

This is a rate-limited actuator problem. The question is whether the controller's stored integral state represents the position the actuator can actually deliver.

## What we need

We need the controller's stored demand to reflect the valve position
actually achieved, even while a valid target is ahead of the moving stem.
That lets us address windup caused by limited movement speed.

## The new idea

**Actuator tracking** uses what the actuator has actually achieved to keep
controller state consistent with it. Here we track measured valve position.
A target inside 0–100% can still be far ahead of the moving stem, so tracking
only that target would miss the discrepancy that can drive windup.

Use this fixed teaching plant:

- vessel thermal capacitance C=50 kJ/°C;
- the original ambient and fixed cold-inlet loss 10(T−20) kW;
- hot inlet temperature 60 °C;
- rounded water heat capacity cp=4 kJ/(kg·°C);
- hot-water mass flow q in kg/s;
- regulated valve pressure drop 100 kPa;
- qsteady=2(x/100) kg/s for valve position x in percent;
- flow lag .5 s and heat-delivery delay 1 s;
- valve position rate vmax=.5 percentage point/s; and
- controller update dt=.1 s.

The hot-water valve replaces the electric heating input. Equal additional outflow keeps vessel mass constant. Specific heat is energy per kilogram per degree, and C=50 kJ/°C means a 50 kJ change in stored energy per degree. One kW is one kJ/s.

The vessel starts at T=35 °C, x=75%, q=1.5 kg/s, and delayed-flow history 1.5 kg/s. At this point the thermal energy carried in by hot water minus that carried out with vessel water gives 4(1.5)(60−35)=150 kW, balancing 10(35−20)=150 kW. The model is:

$$
50\frac{dT}{dt}=4q(t-1)(60-T)-10(T-20).
$$

The flow/temperature product comes from the difference between thermal energy carried by incoming hot water and outgoing vessel water when vessel mass is constant and well mixed. It is not an electric heater rating and is not globally a constant 2 kW per command point. At T=35, a one-point position change changes steady flow by .02 kg/s and therefore adds about 2 kW of heat locally.

Let x be actual stem position in percent, ur the PI position request, and ut the position target after limiting ur to 0–100%. Flow follows physical stem position; the controller reads its measured value. The actuator prediction is

$$
x_{next}=x+\operatorname{clamp}(u_t-x,-v_{max}\Delta t,+v_{max}\Delta t).
$$

Here clamp restricts its first argument between the next two arguments. The position difference and each rate bound are in percentage points. With vmax=0.5 point/s and Δt=0.1 s, one interval permits at most 0.05 point of movement. A fixed target change from 75% to 85% takes 20 s; after 1 s the predicted stem is 75.5%. A 0.5 point/s rate corresponds to a 200 s full stroke.

The supplied flow approximation is $0.5\,dq/dt+q=2x/100$, with q in kg/s. It represents flow lag after the stem changes; a separate flow-response test would establish its time constant. The 1 s delivery delay then separates that flow from its heating effect in the vessel. These are local teaching approximations to inspect in the next valve experiment.

At full valve position x=100%, q=2 kg/s. The nominal equilibrium solves 4(2)(60−T)=10(T−20), giving T=37.7778 °C. A temperature setpoint of 35.5 °C requires heat loss 155 kW and flow q=155/[4(60−35.5)]=1.58163265 kg/s, corresponding to x=79.08163%. This is feasible, but flow lag and delivery delay mean temperature does not jump to that equilibrium.

Use PI with Kp=10 percentage points/°C, Ki=2 percentage points/(°C·s), integral state I initially 75%, tracking time Tt=1 s, and output position 0–100%. The requested position is x_request=Kp e+I, where e=SP−PV. For this experiment, valid temperature and stem measurements represent the corresponding simulated physical values. For SP35→35.5, the first trial integral is 75.1, P is 5, and request is 80.1%. The current measured stem is still 75%; the next physical movement is limited to .05 point.

Tracking corrects the integral toward the request the actuator can deliver. The integral contribution consistent with the measured stem is I=x−P, where x now denotes actual measured position. Subtracting the trial integral gives x−P−I_trial = x−u_r. A tracking update is:

$$
I_{next}=I_{trial}+\frac{\Delta t}{T_t}(x-u_r).
$$

For the first trial, I_trial=75.1, x=75, u_r=80.1, so the correction is .1(75−80.1)=−.51 and I_next=74.59. Tracking uses the current measured position, not a limited target or predicted future position. It does not guarantee a better temperature response; it corrects the stored contribution toward the value consistent with actual position; gradual tracking need not eliminate the mismatch immediately.

## Prediction

Before running, calculate:

1. the 150 kW initial heat balance;
2. the equilibrium at 100% valve;
3. the flow and valve position required for 35.5 °C;
4. the first PI request and tracking correction after SP becomes 35.5 °C;
5. the measured stem after 1 s and the time to move 75→85%; and
6. why a position target of 0% does not immediately stop hot-water flow.

Predict how tracking should affect the integral after the setpoint returns to 35 °C, but do not claim that it guarantees a lower peak before observing the records.

## Implementation

Read measured temperature, stem position and flow as inputs. Write the PI position request and tracking correction with `REAL` quantities in the position scale:

```st
ErrorC := SetpointC - MeasuredTemperatureC;
ProportionalPositionPct := KpPositionPctPerC * ErrorC;
TrialIntegralPositionPct := IntegralPositionPct
    + KiPositionPctPerCs * ErrorC * ElapsedS;
RequestedPositionPct := ProportionalPositionPct + TrialIntegralPositionPct;
```

Insert the known 0–100% limiter to form `TargetPositionPct`, then write

```st
TrackingCorrectionPct := ElapsedS / TrackingTimeS
    * (MeasuredStemPct - RequestedPositionPct);
IntegralPositionPct := TrialIntegralPositionPct + TrackingCorrectionPct;
```

Use current measured position for tracking. The actuator model predicts movement over the following interval; that prediction must not overwrite `MeasuredStemPct`. Check the first-update values 80.1%, −0.51 point and 74.59% before running. The ordinary-integration comparison uses zero tracking correction with every other parameter unchanged.

## Experiment in Control Studio

1. Stop and restart Simulator. Confirm T=35 °C, SP=35 °C, measured stem=75%, requested stem=75%, I=75%, P=0, q=1.5 kg/s, delayed-flow history=1.5 kg/s, D=0, regulated pressure drop within 100±1 kPa, valid data, and dt=.1 s.
2. At Confirm and start, set SP=35.5 °C and define time zero. At 10 s return SP to 35 °C. Record for 70 s with tracking disabled for one run and enabled for the matched run. Keep all plant values, noise, reset, and timing identical.
3. In **Control Studio → Monitor → Response**, display SP, T, error, requested position, measured stem, flow, delayed flow, net heat, P, I, and elapsed time with their units. Inspect the interval when request remains within 0–100 but measured stem cannot yet reach it.
4. Stop if T leaves 33–37 °C, limited target or measured stem leaves 0–100%, flow leaves 0–2.1 kg/s, pressure leaves 100±1 kPa, data become invalid, or the clock stops. A closing request of 0% does not stop flow immediately; observe decreasing stem/flow through the rate and delay. Stop Simulator if it is unresponsive.
5. End the virtual exercise, cancel the command, and restore the complete initial state. Restart before every repeat; do not rely on retained history.

## Evidence and decision

Compare matched tracking-off and tracking-on records using integral state, request, measured stem, flow, temperature peak, post-return request, and recovery toward 35±.2 °C. Verify that tracking uses current measured position and that the valve moves at no more than .5 point/s. A lower integral value does not by itself prove better temperature control, and a slower temperature response does not by itself prove windup. Record the first time after the setpoint returns that temperature enters 35±0.2 °C and stays there through 70 s, or report that it did not settle in the record. Compare that recovery time and peak temperature separately. Verify that the unlimited request stayed within 0–100% during the interval used to demonstrate rate-induced windup; otherwise amplitude saturation also contributed.

## Conclusion

The moving valve changes position at a bounded rate, while its flow and heat effect also have lag and delay. PI tracking keeps stored correction related to the measured stem instead of a request the valve has not yet delivered.

## Reason for the next lesson

Tracking now uses actual stem position, but the heating calculation still assumes that a given position produces the stated flow. The same valve can deliver a different flow when the pressure across it changes with the installed piping.

[Valve characteristics](../../09-industrial-control-structures/58-valve-characterization/README.md) checks the position, pressure and flow relationship before we treat position as delivered heating.
