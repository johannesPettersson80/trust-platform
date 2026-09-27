# 40. Volume balance

You will use the difference between inflow and outflow to predict tank level. You will explain why equal flows hold the present level rather than restoring a previous one.

## The problem

The servo used motor torque to change angular motion. For this tank, the available action is inlet flow and the measurement is liquid level, so we first need the corresponding storage balance. A tank level changes when inlet and outlet flow differ. Holding the outlet fixed isolates accumulation before gravity-driven outflow is introduced.

## What we need

We need to predict how the difference between inlet and outlet flow changes
stored water and level. The model must explain both a rising level and why
equal flows hold the level already attained.

## The new idea

A **volume balance** calculates how stored liquid changes from inflow minus
outflow. We use it to turn known flows into a prediction of water level.

Let A be the constant horizontal tank area, h the water depth, V the stored volume, and qin/qout the inlet/outlet volume flows. Use A=0.5 m², h0=0.25 m and qin=qout=0.010 m³/s. Assume constant water density, no evaporation and no additional leaks or overflow in the test range. Measure area from tank dimensions and flow from collected volume divided by elapsed time. Flow is volume per time and V=Ah. An outlet pump maintains qout at .010 m³/s in this introductory model. This assumed constant outflow isolates volume accumulation; the next lesson replaces it with passive gravity-driven outflow.

Over an interval Δt, inflow adds qin·Δt of volume and outflow removes qout·Δt. Conservation therefore gives ΔV=(qin−qout)Δt. Since ΔV=A·Δh, dividing by the interval and taking its limit gives

$$
A\frac{dh}{dt}=q_{in}-q_{out}.
$$

Units are m²·m/s=m³/s. A step qin=.012 gives net .002 m³/s and dh/dt=.004 m/s. After 10 s, added volume is .020 m³=20 L and level is .29 m. Returning both flows to .010 makes dh/dt zero at .29 m; equal flows do not remove stored water. A zero inlet while outflow continues lowers level.

For the specified model comparison, require the measured level at 10 s to be within 0.001 m of the predicted 0.29 m. Assume measurement uncertainty no greater than 0.0005 m for this exercise; otherwise the record cannot support that accuracy decision.

## Prediction

Calculate the .012 step rate, ten-second level, added litres, and level after restoring equal flow. State why outflow regulation is needed.

## Implementation

Open src/Main.st. Keep LevelM model state separate from the read-only measured level. Initialize the model at 0.25 m and establish that physical level separately in Simulator. Write these updates with a 1 s interval:

```st
LevelRateMPerS := (InletFlowM3PerS - OutletFlowM3PerS) / TankAreaM2;
ModelLevelM := ModelLevelM + LevelRateMPerS * ElapsedS;
```

Each model update advances one second from its previous state; the initial value belongs to time zero before the first update. Reject invalid flow/level and enforce model bounds 0–1 m. Do not write a prediction into a sensor variable.

## Experiment in Control Studio

1. Restart at level .25, qin=.010, qout=.010, valid clock, and disabled control.
2. In **Control Studio → Monitor → Response**, display measured/model level in m, flows in m³/s, level rate in m/s and elapsed time in seconds.
3. At Confirm and start set time zero, apply qin=0.012 m³/s and restore it to 0.010 m³/s at 10 s. Record until 10.1 s so the restored command and 10 s level can be inspected.
4. Stop on level 0–1 violation, invalid data, or stopped clock. Stop Simulator if unresponsive and fully reset before repeating.

## Evidence and decision

Accept the calculation when measured direction and ten-second level agree with .29 m within the predeclared 0.001 m comparison tolerance. Record that accumulated level holds after equal-flow restoration. Do not claim a gravity result here.

## Conclusion

A=.5 m² converts net .002 m³/s into .004 m/s level rise. Conservation predicts 20 L and .29 m after ten seconds.

## Reason for the next lesson

The balance explains accumulation when outlet flow is held constant. A passive outlet opening does not generally keep that flow constant: raising the water level increases the pressure driving water through it.

[Gravity outflow](../16-gravity-tank-nonlinearity/README.md) derives the outlet relationship while retaining the same tank and volume balance.
