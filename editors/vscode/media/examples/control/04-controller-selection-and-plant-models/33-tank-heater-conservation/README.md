# Derive tank and heater models from conservation laws

Course position: Lesson 33 of 84. Previous lesson: [Physical plant-response
classification](../32-plant-response-classes/README.md). Next lesson: nonlinear
gravity-tank linearisation.

**Lesson objective:** Derive an executable buffer tank and heated vessel model
from conservation laws, predict their responses with units, and decide whether
the simulator agrees.

You will not tune a controller or fit a curve here.

This is simulation evidence, not physical-plant evidence.

## The problem

**Equipment:** Constant-area buffer tank and electrically heated vessel.

The workshop contains two independent real-world modules. A constant-area
buffer tank receives measured inlet flow, loses measured outlet flow, and
reports liquid level. An electrically heated, well-mixed vessel receives a
one-sided 0–100% heater command and reports temperature while losing heat to
the room.

The tank has constant area, constant liquid density, perfectly mixed contents,
and no overflow. Its two flows are independently specified. The heater has
constant thermal capacitance and linear heat loss in this operating region.
Sensor dynamics, evaporation, temperature-dependent properties, nonlinear
gravity outflow, and unmeasured disturbances are absent.

Lesson 32 classified the tank as integrating and the heater as
self-regulating. Those response names do not explain why the curves differ or
how equipment dimensions affect them. We now derive both behaviors from the
physical quantities that are stored and transported.

## What we need

We need the tank-level and heater-temperature responses to follow directly from material and energy balances. Each term must have a physical meaning and compatible unit so the model predicts more than the shape of one curve.

## The new idea

A **stored quantity** is material or energy retained by equipment. A **state
variable** is a quantity, such as level or temperature, whose present value is
needed to predict future behavior. The stored quantity accumulates when the
rate entering exceeds the rate leaving. A conservation balance says in words:

> Rate of accumulation = total rate entering − total rate leaving.

For the tank, define the tank cross-sectional area ($A$) in square metres, liquid
level ($h$) in metres, inlet volumetric flow ($q_{\mathrm{in}}$) in cubic metres per second,
and outlet volumetric flow ($q_{\mathrm{out}}$) in cubic metres per second. Only after those
physical meanings and units are known can we write the mass-volume balance.
The plant flowmeters display litres per second, so convert them first: one litre
is 0.001 cubic metres. The balance is:

$A \cdot dh/dt = q_{in} - q_{\mathrm{out}}$

The liquid-level rate (dh/$\Delta t$) is the change in level per second. Equal flows
give zero accumulation and therefore equilibrium. Unequal constant flows give
a continuing level slope when tank area is constant.

For the heater, define thermal capacitance ($C$) in kilojoules per degree
Celsius, vessel temperature ($T$) in degrees Celsius, ambient temperature
($T_{\mathrm{ambient}}$) in degrees Celsius, electrical heating power ($P_{\mathrm{heater}}$) in
kilowatts, and heat-loss coefficient ($H$) in kilowatts per degree Celsius. The
energy balance is:

$C \cdot dT/dt = P_{\mathrm{heater}} - H \cdot (T - T_{\mathrm{ambient}})$

The temperature rate (dT/$\Delta t$) is zero at equilibrium. Equilibrium therefore
means zero accumulation of stored energy. Heat loss grows with the
temperature difference, so a fixed heater power reaches a finite equilibrium.
Thermal capacitance changes response speed, not that equilibrium. The static
gain relates steady temperature change to command. The time constant describes
response speed. Transport delay postpones the visible response without changing
the final balance.

## Prediction

The tank area is 0.5 square metres. It starts at 50 centimetres. Inlet flow is
12 litres per second and outlet flow is 10 litres per second. Net inflow is 2
litres per second, or **0.002 cubic metres per second**. Divide by area:

$level rate = 0.002 m_{3}/s / 0.5 m_{2} = 0.004 m/s = 0.4 cm/s$

The predicted rate is therefore **0.4 centimetres per second**. After 20
seconds, predict $50 cm + 0.4 cm/s \cdot 20 s = 58 centimetres$. With
equal inlet and outlet flows of 10 litres per second, predict zero level slope.

The heater has 50 kilojoules per degree Celsius thermal capacitance, 10
kilowatts per degree Celsius heat-loss coefficient, and 200 kilowatts at full
output. Ambient and initial temperature are 20 °C. A 20-percent command gives
40 kilowatts. The predicted equilibrium is `20 °C + 40 kW / 10 kW/°C = 24
degrees Celsius`. Static gain is **0.2 degrees Celsius per percentage point**,
and the time constant is $50 / 10 = five seconds$.

After the one-second delay and one time constant, a first-order response has
completed about 63.2% of its four-degree rise. Predict **22.53 degrees Celsius
at six seconds**.

## Implementation

There is no feedback controller under test. The actual Structured Text makes
the unit conversion and physical predictions visible:

```st
TankNetFlowLitresPerSecond := TankInletFlow - TankOutletFlow;
TankNetFlowCubicMetresPerSecond := TankNetFlowLitresPerSecond * LREAL#0.001;
TankPredictedRate :=
    (TankNetFlowCubicMetresPerSecond / TankAreaSquareMetres) * LREAL#100.0;
TankNetFlowCommand := TankNetFlowLitresPerSecond;

HeaterPowerKilowatts :=
    HeaterFullScalePower * HeaterAppliedCommand / LREAL#100.0;
HeaterTimeConstant := HeaterThermalCapacitance / HeaterHeatLossCoefficient;
HeaterPredictedEquilibrium :=
    HeaterAmbientTemperature + HeaterPowerKilowatts / HeaterHeatLossCoefficient;
```

The tank simulator integrates net flow. The heater simulator uses the same
gain, time constant, delay, bias, and initial state derived above.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Derive both balances, run the declared unequal-flow and heater case, restore the tank, then run the equal-flow equilibrium case. The declared event lasts 30 seconds. It restores Tank net-flow command to 0 L/s afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Tank level (PV, cm) and Heater temperature (PV, degC); Tank net-flow command (MV, L/s) and Heater applied command (MV, %), including units, limits, mode, and quality. The initial operating point is Tank inlet flow = 12 L/s, Tank outlet flow = 10 L/s, Tank net-flow command = 2 L/s, Tank level = 50 cm, Heater command request = 20 %, Heater applied command = 20 %, Heater temperature = 20 degC. The unresolved baseline problem is: Integrating and self-regulating labels describe curve shapes but do not explain their physical causes or parameter dependence.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept each model only when its unit-consistent first-principles prediction matches the independent simulator response within declared tolerances.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **30**. The observed lesson signal is **Tank net-flow command**. Control Studio observes the project's existing timed sequence and does not write a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to Tank net-flow command. Confirm the live values are current and the reviewed plan stays inside Tank level 0 cm to 100 cm; Heater temperature 0 degC to 100 degC. Use this run only to check the conservation prediction; do not Identify, Tune, or Apply.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: tank_level_rate >= 0.39 cm/s (tolerance 0.01 cm/s); tank_level_at_20_seconds >= 57.95 cm (tolerance 0.05 cm); heater_temperature_at_six_seconds >= 22.448 degC (tolerance 0.08 degC); heater_final_temperature >= 23.97 degC (tolerance 0.03 degC).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

**Tank level-rate error** is the absolute difference between the measured
late-window slope from 28.0 to 29.9 seconds and 0.4 centimetres per second.
**Tank 20-second prediction error** is the absolute measured-minus-predicted
level at 20 seconds, in centimetres.

**Equal-flow drift** is maximum level minus minimum level over the independent
20-second equal-flow run, in centimetres.

**Heater six-second prediction error** is the absolute measured-minus-predicted
temperature at six seconds. **Heater final-window equilibrium error** is the
absolute difference between 24 °C and the final sample after confirming that
the 28.0-to-29.9-second slope is below 0.01 °C/s. Both use degrees Celsius.

![Conservation balances predict both accumulated level and stored heat](figures/tank-heater-conservation.svg)

[Retained run](figures/tank-heater-conservation-run.csv) ·
[Plot data](figures/tank-heater-conservation-trace.csv) ·
[Decision summary](figures/tank-heater-conservation-summary.csv)

### Validation

The native executable run produced:

| Evidence | Result | Decision |
|---|---:|---|
| Tank late slope | 0.4000 cm/s | Matches balance |
| Tank level at 20 s | 58.000 cm | Matches prediction |
| Equal-flow drift | 0.000000 cm | Equilibrium held |
| Heater at 6 s | 22.528 °C | Matches prediction |
| Heater at 30 s | 23.988 °C | Approaching 24 °C equilibrium |

The heater remains at 20 °C during its declared one-second delay. A run with
different units, initial state, timing, or simulator parameters is invalid for
this comparison rather than contrary evidence for these balances.

## Conclusion

The tank integrates because every second of nonzero net flow changes stored
liquid volume. Larger tank area produces a smaller level slope for the same net
flow. The heater self-regulates because rising temperature increases heat loss
until loss equals heating power. Thermal capacitance sets how quickly it gets
there; heater power and heat-loss coefficient set the equilibrium; delay shifts
when the response becomes visible.

## Reason for the next lesson

First-principles models explain causal structure, units, equilibrium, and the
effect of physical parameters. Their conclusions are only as good as their
assumptions and equipment data. In Lesson 34, nonlinear gravity outflow makes
the tank slope depend on level, creating a reason to derive a local linear
model around an operating point.
