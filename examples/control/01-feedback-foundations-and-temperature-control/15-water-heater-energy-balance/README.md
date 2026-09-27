# 13. Energy balance

You will use energy conservation to predict a heater's equilibrium temperature and speed of response from stored thermal energy, delivered power and heat loss.

## The problem

The water warms because the heater supplies energy, and it stops warming when that input balances energy leaving the vessel. To predict both facts, we need a physical balance rather than a command-to-temperature rule. The model below is a supplied teaching case; its parameters have physical measurement origins, but this lesson does not claim to identify them from a run.

## What we need

We need a physical relationship that predicts how delivered heating and
heat loss change water temperature over time. It must explain both the final
equilibrium and the speed of approach, rather than supply unexplained model
coefficients.

## The new idea

An **energy balance** states that stored energy changes by energy entering
minus energy leaving. For our water, delivered heating adds energy and heat
loss removes it. Their difference determines the temperature rate.

Assume a well-mixed vessel, constant effective capacity and loss coefficient over the range, and one temperature state.

Thermal capacitance is the energy needed for a one-degree temperature rise. The value $C=50\ \mathrm{kJ/°C}$ combines fluid mass times specific heat with the vessel's heat capacity. Specific heat is energy per unit mass per degree, so multiplying it by mass gives energy per degree. The loss coefficient $H=10\ \mathrm{kW/°C}$ combines ambient heat transfer and any fixed throughflow loss, whose physical contribution would be mass flow times specific heat. Both quantities are supplied teaching values and would be established on real equipment from dimensions, material data, flow measurements and thermal tests. Take the surroundings and nominal inlet to be at 20 °C; a colder inlet creates the separate additional loss D.

Let $T$ be vessel temperature in °C, $u$ applied heater command in percent, and $D$ positive extra heat removal in kW. The 200 kW heater produces $2\ \mathrm{kW}$ per command point. A colder inlet at unchanged flow is represented by positive $D$. For constant C, stored thermal energy relative to 20 °C is E = C(T − 20), so its rate of change is C·dT/dt. One kJ/s is one kW. Equating that rate to heater input minus heat leaving the water gives

$$
C\frac{dT}{dt}=2u(t-1)-H(T-20)-D.
$$

The one-second argument is the heat-delivery delay: the balance receives $u(t-1)$, not the current request. It is separate from thermal storage; it shifts when input arrives and does not add to $C$.

At equilibrium, set the derivative to zero. With $C=50$, $H=10$, command 75% and $D=0$, $T_{eq}=20+(150/10)=35$ °C. At 85%, $T_{eq}=37$ °C. At 75% with $D=30$ kW, $T_{eq}=20+(150-30)/10=32$ °C. The nominal steady gain is $K=2/H=.2$ °C per percentage point.

Now use deviations about the hot operating point: $x=T-35$ °C and $v=u-75$ percentage points. Dividing the balance by $H$ gives

$$
5\frac{dx}{dt}+x=.2v(t-1)-\frac{D}{10},
$$

where $\tau=C/H=5$ s and $K=2/H=.2\ \mathrm{°C/point}$. For a held $v=10$ and $D=0$, the final deviation is $x_f=Kv=2$ °C. The remaining gap $z=x_f-x$ obeys $dz/dt=-z/\tau$. The function $z(t)=z(1)e^{-(t-1)/5}$ satisfies this equation by differentiation, so for $t\ge1$

$$
T(t)=35+2\left(1-e^{-(t-1)/5}\right),
$$

while $T=35$ °C before delayed power arrives. At $t=6$ s, one time constant after the delay, $T=35+2(1-e^{-1})\approx36.264$ °C: 63.2% of the final change has occurred. At $t=26$ s, five time constants after the delay, about 99.3% has occurred. A time constant is a response scale, not a claim that the process is settled exactly at 5 s. Increasing $C$ increases $\tau$ but does not change $K$ when $H$ stays fixed.

## Prediction

Before running, calculate:

1. equilibrium at 75% with $D=0$;
2. equilibrium at 85% with $D=0$;
3. equilibrium at 75% with $D=30$ kW;
4. the final temperature change for a 10-point command step;
5. the initial temperature slope after the one-second delay for that step; and
6. temperatures at 6 s and 26 s for the held 75→85% case.

The initial post-delay slope is $20\ \mathrm{kW}/50\ \mathrm{kJ/°C}=.4$ °C/s because the loss is still balanced at the starting temperature. Predict why changing $C$ alters speed but not equilibrium.

## Implementation

Inspect the existing calculation in `src/Main.st`. This excerpt uses the supplied delayed applied command and keeps model temperature distinct from measured PV:

```st
HeaterPowerKw := REAL#2.0 * DelayedCommandPct;
LossPowerKw := EffectiveHeatLoss * (ModelTemperatureC - REAL#20.0);
NetPowerKw := HeaterPowerKw - LossPowerKw;
IF ThermalCapacitance > REAL#0.0 AND EffectiveHeatLoss > REAL#0.0 THEN
    ModelRateCPerS := NetPowerKw / ThermalCapacitance;
    ModelTemperatureC := ModelTemperatureC + REAL#0.1 * ModelRateCPerS;
ELSE
    SafetyAbort := TRUE;
    AppliedPowerPct := LREAL#0.0;
END_IF;
```

The project declares these model quantities as `REAL`, initializes `ModelTemperatureC` to 35.0 and supplies `DelayedCommandPct`. The final assignment is a 0.1 s forward-Euler approximation, meaning it advances the differential equation using the current rate over one small interval. The plant and model keep separate temperatures and delivery histories. Extra heat removal is fixed at $D=0$ in this runnable case; the $D=30$ calculation above is a paper prediction.

The provided command-history interface retains ten 100 ms samples for the
model's 1 s delay. The simulator owns a separate physical delivery history.
`ModelTemperatureC` is the REAL Euler state; `TemperatureC` is the independent
simulated PV. Both start at 35 °C, with 75% delayed history. The prepared program
holds this reset while you review, applies 85% when enabled, and stops to zero.

## Experiment in Control Studio

1. Start Simulator and open **Signals** under **Identify → Method → Project experiment**. Select measured output, model temperature and applied input, then inspect the response plot below. Confirm both temperatures are 35 °C and applied input is 75%. In **Signals**, check delayed command is 75%, experiment authority is off and Safety abort is off. Record your predictions.
2. Choose **Project experiment** from **Method**, then select **Review project test**. The 30.1 s recording includes the full 30 s comparison interval. **Confirm and start** establishes time zero and changes applied input from 75% to 85%. The project holds this input while enabled; leave the test running until completion.
3. After completion, confirm applied input is zero. Stay on **Identify** to inspect the complete recorded response. Use **Signals** to select one group at a time, inspecting the response plot after each selection: measured output/model temperature/applied input; heater power/loss power; model temperature rate; delayed command/applied input. Hover over the response to compare values at the same timestamp when model and measured curves nearly overlap.
4. Compare the recorded values with your delay, slope and 6 s/26 s predictions. Record the measured values and differences in your notes. This compares the supplied energy-balance model with measured data; it does not identify a model.
5. Select **Abort test** if measured temperature leaves 32–39 °C, applied input leaves 0–100%, or measurements become invalid or stop updating. Confirm zero applied input; stop Simulator if it is unresponsive. An out-of-range temperature latches Safety abort even after valid feedback returns. An ordinary operator abort removes heat without setting that fault. Restart Simulator before repeating to restore both temperatures to 35 °C and both 1 s histories to 75%.

## Evidence and decision

Check equilibrium, initial post-delay slope, and the 6 s and 26 s values separately. The expected held-step curve is 35 °C until the delayed input arrives, 36.264 °C at 6 s, and approximately 36.986 °C at 26 s. Do not call 5 s “settled”; it is one time constant after the delay.

Use the model temperature to assess the equation and PV to assess the visible simulated measurement. The supplied $C,H,K,\tau$ values are teaching quantities, not learner identification results. Real equipment may have changing capacity, imperfect mixing, variable heater efficiency, or unmeasured throughflow. Lesson 14 later designs a bounded identification experiment, and Lesson 15 estimates parameters from its record.

## Conclusion

Energy conservation gives $C\,dT/dt=2u(t-1)-H(T-20)-D$. The teaching case has 35 °C equilibrium at 75%, 37 °C at 85%, 32 °C under a 30 kW extra removal, gain .2 °C/point, time constant 5 s, and a separate one-second delivery delay. The exponential response describes how stored energy approaches its new equilibrium; it does not turn a time constant into an exact settling time.

## Reason for the next lesson

The balance gives predictions for a known model. To obtain equipment parameters responsibly, the next experiment must choose an informative input, duration, limits and restoration procedure.

[A bounded identification experiment](../../05-experiments-identification-and-validation/21-safe-informative-experiment/README.md) helps you plan that experiment.
