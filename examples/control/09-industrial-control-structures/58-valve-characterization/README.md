# 31. Valve characteristics

You will measure how valve position affects flow and how pipe pressure loss changes that relationship. You will use the flow to estimate heating power at a stated water temperature.

## The problem

The position-tracking lesson assumed flow proportional to stem position at a regulated pressure drop. We now test that assumption while keeping the same valve and vessel. The shared vessel has $C=50$ kJ/°C and loses $10(T-20)$ kW. Hot inlet water is 60 °C with $c_p=4$ kJ/(kg·°C), and flow q is kg/s. At T=35 °C, q=1.5 kg/s delivers $4(60-35)(1.5)=150$ kW, balancing the 150 kW loss. The valve is at 75%, with actual stem and flow history reset to 75% and 1.5 kg/s.

## What we need

We need to know how actual valve position and pressure determine delivered
flow, and therefore heating. A position reading alone cannot establish the
heat input needed for the temperature-control decision.

## The new idea

A **valve characteristic** describes how opening relates to flow. The installed
relationship also depends on the pressure drop across the valve. A position
measurement tells us where the stem is; it does not by itself establish how
much hot water reaches the vessel.

Here $x$ is stem opening in percent and $q_{ss}$ is steady mass flow in kg/s.

At regulated valve pressure drop $\Delta p=100\pm1$ kPa, the supplied local characteristic is $q_{ss}=2x/100$ kg/s. Measure its slope rather than treating it as an electric-heater rating. For 75→80%, q=1.5→1.6 gives

$$
\frac{dq}{dx}=\frac{1.6-1.5}{80-75}=0.02\ \mathrm{kg/(s\ point)}.
$$

At T=35 °C, the local heat slope is $c_p(60-35)(0.02)=2$ kW/point; it changes when T changes.

For water treated as incompressible with density ρ≈1000 kg/m³, pressure work per unit mass is Δp/ρ. Converting that work to flow kinetic energy gives a velocity proportional to sqrt(Δp). Effective opening area and a discharge coefficient account for valve geometry and losses. Calibrating full opening at 2 kg/s and 100 kPa gives

$$
q_{ss}=2z\sqrt{\frac{\Delta p}{100\ \mathrm{kPa}}}\ \mathrm{kg/s},\qquad z=x/100.
$$

This approximation assumes effective area proportional to opening. The actual installed characteristic must be measured; the formula supplies a comparison case.

Now keep the same valve and pipe, but hold total supply pressure at 128.125 kPa rather than regulating pressure across the valve. The pipe-loss coefficient is 12.5 kPa/(kg/s)². At q=1.5 kg/s the pipe loses 12.5(1.5)²=28.125 kPa, leaving 100 kPa across the valve, so the original operating point is unchanged.

At other flows, valve pressure is Δp=128.125−12.5q², in kPa when q is expressed in kg/s. Substitute into the calibrated flow equation and square it:

$$
q^2=4z^2(1.28125-0.125q^2),
$$

$$
q^2(1+0.5z^2)=5.125z^2.
$$

Taking the nonnegative flow solution gives

$$
q_{ss}=2z\sqrt{\frac{1.28125}{1+0.5z^2}}\ \mathrm{kg/s}.
$$

At 85%, this is approximately 1.649 kg/s, unlike the regulated-pressure value
1.7 kg/s. The installed characteristic changes because pressure drop changes
with flow even though the valve's internal geometry is unchanged. These
coefficients are model assumptions; the activity measures local slopes and uncertainty.

Use valve rate 0.5 percentage points/s, flow lag 0.5 s, delivery delay 1 s, temperature bounds 33–37 °C, stem/target bounds 0–100%, and flow bounds 0–2.1 kg/s. Hold each target until the actual stem arrives and the measured flow range (maximum minus minimum) is at most 0.015 kg/s over the final 3 s. For the worked calibration, assume each qualified steady-flow mean has uncertainty ±0.005 kg/s. Then the 75→80% slope lies between (0.1−0.01)/5=0.018 and (0.1+0.01)/5=0.022 kg/(s·percentage point).

## Prediction

Predict flow at 70, 75, 80 and 85% under regulated pressure, then predict why the fixed-total line gives a different 85% value. Predict heat at T=35 °C and explain why T does not jump when the stem target changes. State how flow lag and 1 s heat-delivery delay will order the observations.

## Implementation

With PI disabled, write the scalar calculations `FlowSlopeKgPerSPerPoint := FlowChangeKgPerS / StemChangePct;` and `EstimatedHeatKW := 4.0 * DelayedFlowKgPerS * (60.0 - MeasuredTemperatureC);`. Guard a zero stem change before division, and check the 75→80% example. Read actual stem and measured flow rather than assigning model predictions to them. Log actual stem, not request alone. Use a hold for calibration only after arrival and the stated steady-flow range rule. Keep flow lag and thermal delivery delay as separate states. A stop request sets target to 0% but closure and flow decay continue.

## Experiment in Control Studio

1. Reset T=35 °C, stem/target 75%, q=1.5 kg/s, pressure condition, and history. Set time zero at **Confirm and start**.
2. In **Control Studio → Test**, review the regulated-pressure staircase 75→70→75→80→85→75%, holding each new target 30 s for a 150 s total record. Display target, actual stem, q, pressure, temperature, heat estimate and validity.
3. Accept a steady sample only after stem arrival and the final 3 s flow-range rule. Record local slope and uncertainty.
4. Stop if regulated valve pressure leaves 100±1 kPa. Fully reset and repeat under fixed total supply 128.125±0.5 kPa. Stop on T outside 33–37 °C, target/stem outside 0–100%, q outside 0–2.1, invalid data, fixed supply outside 128.125±0.5 kPa, or normal completion at 150 s.
5. Issue stop/target 0%, wait for slow closure, then stop the simulator if needed. Zero target does not mean instant zero heat. Fully reset before repeating.

## Evidence and decision

Report each accepted hold with target, actual stem, pressure, q, heat, arrival time and uncertainty. Compare regulated and fixed-total slopes; normal pressure-dependent differences are expected. Reject holds that never settle or mix transient flow with steady calibration. Do not infer heat from request or call nominal coefficients measured facts.

## Conclusion

The installed valve characteristic belongs to pressure, stem, flow and thermal conditions. Separating actuator travel, hydraulic lag and heat delivery makes a local calibration useful for control and diagnosis.

## Reason for the next lesson

The steady characteristic assumes the stem reaches its target. It does not
explain a small request that is ignored, or an accepted target with a stem
that remains stuck. [Valve stiction](../59-valve-stiction/README.md) distinguishes
command deadband from mechanical hold-and-release on the same installation.
