# 41. Gravity outflow

You will learn why flow through a fixed tank outlet increases as the water level rises. You will derive the gravity-outflow relationship and use it to predict the settled level.

## The problem

Lesson 40 held the outlet at a regulated .010 m³/s to isolate accumulation. A fixed opening behaves differently: a higher liquid level creates greater pressure head and therefore greater outflow. We now release the held-outlet assumption without changing the tank.

## What we need

We need an outlet-flow relationship that accounts for the water height
driving drainage. Combined with the volume balance, it must predict how
level changes and where a held inlet flow reaches equilibrium.

## The new idea

**Gravity outflow** is drainage driven by the height of liquid above the
outlet. A taller water column creates greater pressure, so the fixed opening
does not deliver one constant flow at every level.

Use A=.5 m², initial h0=.25 m, inlet qin=.010 m³/s, and level bounds 0–1 m. Assume an open tank and outlet both at atmospheric pressure, negligible surface velocity, constant liquid density and a rapid outlet-flow response relative to changing level. The fixed outlet has area Ao in m² and dimensionless discharge coefficient Cd. The coefficient Cd represents contraction and losses; on real equipment it comes from measured flow at known head. For this teaching case, combine the opening quantities into c=.020 m^(5/2)/s.

A water column of height h creates pressure p=ρgh, where ρ is density in kg/m³ and g=9.81 m/s² is gravitational acceleration. Its available gravitational energy per unit mass is gh. Equating this to velocity energy v²/2 gives v=√(2gh), with outlet velocity v in m/s. The opening flow is qout=Cd Ao v. Define c=Cd Ao√(2g), giving:

$$
q_{out}=c\sqrt h.
$$

The units are explicit: √h has units m^(1/2), so c must have m^(5/2)/s to produce qout in m³/s. The level balance is:

$$
A\frac{dh}{dt}=q_{in}-c\sqrt h.
$$

At h=.25 m, √h=.5 and qout=.020(.5)=.010 m³/s, matching the initial inlet. If qin steps to .011, the initial level rate is (.011−.010)/.5=.002 m/s. Equilibrium satisfies qin=c√heq, hence:

$$
h_{eq}=\left(\frac{q_{in}}{c}\right)^2=\left(\frac{0.011}{0.020}\right)^2=0.3025\ \mathrm m.
$$

Doubling h multiplies outflow by √2, not by two. This square-root dependence is the nonlinearity.

## Prediction

Calculate the initial outflow and rate after qin=.011, the equilibrium .3025 m, and the outflow at h=.50 m. Predict why the level approaches a finite value while inlet remains fixed. Explain what happens if qin is set to zero while the opening remains present.

## Implementation

Open src/Main.st. Keep measured level and model level as separate REAL values. Initialize ModelLevelM at 0.25 m and establish that physical level separately in Simulator. Check model level is nonnegative before calling SQRT; a negative level is invalid and must not be repaired by taking its absolute value. Write `OutletFlowM3PerS := DischargeCoefficientM52PerS * SQRT(ModelLevelM);`, then divide inlet minus outlet by area to obtain level rate. `SQRT` returns the nonnegative square root. Advance ModelLevelM by that rate times the 1 s interval, using the same update reasoning as the volume-balance lesson. Keep qin in m³/s and level in m. Never assign the model prediction into the sensor variable.

## Experiment in Control Studio

1. Stop and restart Simulator. Confirm measured/model level=.25 m, qin=.010 m³/s, calculated outflow=.010 m³/s, valid timestamp, and dt=1 s.
2. In Monitor → Response display measured level, model level, inlet, calculated outflow, level rate, equilibrium prediction, and elapsed time.
3. Before viewing results, declare the illustration criteria: run 200 s; final-10 s level drift must be ≤.001 m; endpoint must be within .002 m of analytical equilibrium .3025 m.
4. At Confirm and start define time zero and apply qin=.011 m³/s. Leave the fixed opening active and record 200 s. This allows the initial rise to slow and provides a late window for checking whether the record supports equilibrium. At normal completion restore qin=.010 and wait for the documented reset.
5. Stop for level outside 0–1 m, inlet outside 0–.020 m³/s, invalid data, or stopped time. Setting inlet to zero does not isolate the tank; gravity continues draining. Stop Simulator and fully reset before repeating.

## Evidence and decision

Compare measured direction, final-10 s drift, and endpoint with the analytical equilibrium. These are exercise criteria, not a universal settling-time claim. Preserve the fixed-opening assumption and the c coefficient. A record with continuing drift may be physically plausible but fails the declared endpoint criterion.

## Conclusion

Pressure head and energy produce $q_{out}=c\sqrt h$. Combining that outflow with volume storage gives a nonlinear level balance whose equilibrium changes with the square of inlet flow.

## Reason for the next lesson

The square-root outlet law explains why the tank approaches a level-dependent equilibrium. To use the linear PI-tuning approach already taught, we need a process gain and time constant around a selected operating level.

[Linearization](../17-gravity-tank-linearization/README.md) derives that local approximation and checks how far from the operating point it remains useful.
