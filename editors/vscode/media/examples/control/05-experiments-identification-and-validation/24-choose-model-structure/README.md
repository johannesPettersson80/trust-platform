# 16. Model structure

You will decide whether the heater model also needs to describe the temperature sensor’s response. You will test whether sensor lag explains a feature that the original model misses.

## The problem

The step-response fit assumed one temperature state. If the final value agrees but a repeatable bend remains unexplained, changing its parameters may not describe the missing physics. An extra bend can come from another physical store in the water process or from the measurement probe. Both can make a first-order fit look wrong. Adding a term because it lowers residuals does not identify its physical origin.

## What we need

We need to decide whether a repeatable prediction error comes from incorrect
parameter values or a missing physical response. Any additional state must
explain the observed feature and have a defensible physical meaning.

## The new idea

**Model structure** is the choice of physical relationships and stored quantities
represented in the model. Changing coefficients cannot always explain a missing
mechanism. Here a probe may warm more slowly than the water, so we give water
and sensor temperature separate states and compare that prediction with the
instantaneous-sensor model.

Let $T$ be water temperature and $T_s$ the measured sensor temperature. For a possible sensor thermal response, use

$$
C_s\frac{dT_s}{dt}=G_s(T-T_s),\qquad \tau_s=\frac{C_s}{G_s}=2\text{ s}.
$$

Here Cs is the probe's thermal capacitance in J/°C and Gs is heat conductance from water to probe in W/°C. Illustrative values Cs=5 J/°C and Gs=2.5 W/°C give the 2 s ratio. Probe mass and material properties determine Cs; a separate immersion response can estimate the ratio. Its energy exchange is small compared with the vessel, so it is neglected in the water balance. The water physics is unchanged; the extra state belongs to the measurement. With deviations $x=T-35$, $m=T_s-35$, and applied command deviation $v=u-75$, the supplied process example is

$$
5\dot x+x=0.2v(t-1),\qquad 2\dot m+m=x.
$$

Candidate A predicts $m=x$, an instantaneous sensor. Candidate B predicts the second equation as well. The model must say which signal each state represents; a second process store could produce the same measured shape.

For a held 10-point step after the 1 s transport delay, Candidate A gives $x(t')=2(1-e^{-t'/5})$, where $t'=t-1$. Try $m(t')=2+a e^{-t'/5}+b e^{-t'/2}$ in the sensor equation. Substitution into $2\dot m+m=x=2-2e^{-t'/5}$ gives $3a/5=-2$, so $a=-10/3$ °C. The initial condition m(0)=0 then gives b=4/3 °C. Thus

$$
m(t')=2\left[1-\frac{5e^{-t'/5}-2e^{-t'/2}}{3}\right].
$$

At 6 s, $t'=5$: $x=1.26424$ °C and $m\approx0.88318$ °C, so the instantaneous-sensor prediction differs by about 0.38106 °C. This is a model calculation, not a recorded result. It shows what a 2 s sensor response could explain; it does not prove the probe has that response.

To distinguish probe lag from a process lag, use a fast independent water reference or a known probe-immersion/step calibration. A curve fit alone cannot assign the extra time constant to water or sensor.

Use the qualified applied-command record, baseline 35 °C, and learner K, τ, and L from Lesson 15. Retain the first-order fit as Candidate A. Its τ and L may already include part of the probe response, so they are starting estimates for an extended model, not independently known water parameters. Define two candidates before comparison. A residual is measured minus predicted temperature. For this exercise, require absolute residual no greater than 0.2 °C throughout the valid record and final-temperature prediction within 0.1 °C; keep the same fitting window for both candidates. An added state must explain a repeatable feature larger than the measurement uncertainty. The preceding lesson used the sensor error bound to examine parameter consistency. The .2 °C and .1 °C limits here are declared allowances for model-approximation error, not a larger sensor-error claim; keep them fixed for both structures. Candidate A keeps the sensor instantaneous. Candidate B adds a sensor state.

## Prediction

Calculate Candidate A and B at 6 s for the 75→85 step. Predict whether an extra lag changes early slope, delay, or late value. Decide whether the existing learner record has enough resolution and duration to distinguish a 0.381 °C difference, using its actual K, τ, L and uncertainty rather than replacing them with the worked values.

## Implementation

Use Identify to compare Candidate A and Candidate B predictions against the qualified applied-command history. Export the saved Lesson 15 result from **Validate → Export result**, then use **Identify → Import recording…** in this project. Inspect the original source, units and samples. Keep the Lesson 15 first-order parameters and uncertainty with Candidate A; importing a recording does not accept a model or load another project’s controller. For the worked second-order case, update old/new $x$ and $m$ states with the supplied input and sample interval. Keep the first-order values with Candidate A and as the starting estimates for Candidate B; do not simply add sensor lag while freezing fitted parameters that may already absorb it. Refit Candidate B on the same identification record using K, water time constant τp, sensor time constant τs and L, holding a parameter fixed only when independent physical evidence establishes it. Extend the explained fit from the preceding lesson with a positive sensor-time search, initially .1–10 s; retain the other stated bounds, investigate active search boundaries and refine the grid. Evaluate both state equations against the measured probe temperature. The .1 s lower search limit is not proof that faster sensor dynamics are absent or identifiable. The 2 s example remains an illustration, not a substitute for the fit. The ST exercise names learner state variables; current `Main.st` exposes `WaterTemperatureC`, `TemperatureC`, and `AppliedPowerPct`, and does not expose a fitted Candidate A/B result. Compute both next states from the previous states before assigning either:

```st
NextWaterDeviationC := WaterDeviationC + ElapsedS
    * (ProcessGainCPerPoint * DelayedCommandChangePct - WaterDeviationC)
    / ProcessTimeS;
NextSensorDeviationC := SensorDeviationC + ElapsedS
    * (WaterDeviationC - SensorDeviationC) / SensorTimeS;
WaterDeviationC := NextWaterDeviationC;
SensorDeviationC := NextSensorDeviationC;
```

For the worked model, initialize both deviations to zero. With a 10-point delayed input and a 0.1 s interval, the first update gives water deviation 0.04 °C and sensor deviation zero; the following update gives 0.0792 °C and 0.002 °C. These are finite-step approximations to the two balances, not measured readings.

Open [src/tests.st](src/tests.st) and run **ST: Run All Tests** from the command
palette. The first calculation executes these two updates; the second checks
the continuous 6 s prediction above. To see why update order matters, move the
water assignment before the sensor calculation and predict the first sensor
value before running again. It becomes 0.002 °C instead of zero because it uses
the new water value. Restore the original order and rerun. These calculations
do not drive the heater or replace comparison against recorded measurements.

The native simulation supplies water temperature at `%IL0` and probe temperature at `%IL8`. Its two thermal balances retain the 1 s command delay, 2 s probe response and 35 °C / 75% operating point. `Main.st` reads both temperatures; it does not simulate the probe. Restarting resets both to 35 °C. The model uses deviations internally: command limits −75..25 correspond to physical heating limits 0..100%.

Set `SafetyAbort` to stop heating immediately; its latch also sets on invalid
measurement. An abort commands zero heating. Normal
completion restores 75% heating. After an abort, keep stopped and restart from
the full hot reset before clearing the latch and collecting a new trial.

## Experiment in Control Studio

Use the saved Lesson 15 result for this comparison. The runtime can remain
stopped: importing and fitting the existing record do not require another
heater experiment. The worked 2 s probe response is a hypothesis to examine,
not a result the learner must obtain.

1. In Lesson 15, open the saved result in **Validate** and choose **Export result**. Open this lesson's project and choose **Identify → Import recording…**. Select that JSON file and expand **Recording source**. Confirm the original source, temperature and applied-command units, and estimation record. An export may also contain the later validation recording; the comparison uses the original estimation data. If that prerequisite is missing, finish Lesson 15 first.
2. Keep **Measure a held step** enabled and select **Compare additional lag**. Open **Measurement and search settings**. Keep the imported baseline/final windows, measurement bound and first-order search settings. Set the additional time-constant range to **0.1–10 s**, with **0.1 s** spacing. Use **0.2 °C** for the peak model-error limit and **0.1 °C** for the final model-error limit. These limits allow model approximation error; they do not replace the separate **±0.05 °C** measurement bound.
3. Choose **Check recording**, then **Fit estimation run**. If Check rejects the record, follow the named coverage, window or measurement issue before fitting. Candidate A retains the original first-order fit; Candidate B searches the additional-lag model against the same estimation samples.
4. Use **Fit** to compare the measured temperature and both predictions on the same axes. Switch to **Residuals** to inspect measured minus predicted temperature. Look for a repeatable error pattern, not just a small reduction in the total error. Read the peak and final errors in **Model comparison** against the limits declared in step 2.
5. Expand **Parameters and uncertainty**. Record each candidate's equation, parameters, consistent ranges and search-boundary warnings. **No consistent candidates** means no searched parameter set explains every sample within the measurement bound. It does not by itself mean the model misses the looser approximation-error limits. A fitted pair of time constants cannot identify which lag belongs to the water or the probe.
6. If the first-order model meets the limits, choose **Keep first-order model**. If the additional lag is supported, inspect its error improvement and choose **Keep additional-lag model**; without an independent reference, record its physical origin as unresolved. If neither is supported, use **Review settings** and investigate the reported issue. Keep the original error requirements when comparing refinements.
7. Choose **Save comparison**. To revisit it, open **Validate → Saved results**, select the saved revision and choose **Open**, then return to **Identify**. Keep the equations, uncertainty and your reason for the provisional choice for Lesson 17. Independent validation remains the task of Lesson 18.

## Evidence and decision

Keep Candidate A when it meets the declared model-error limits. Candidate B is justified when A misses those limits, B meets them, and its repeatable error improvement exceeds the measurement uncertainty. The extra state represents a possible additional physical response; the fit does not establish whether it belongs to the water or the probe. Without an independent reference, report “additional lag observed; physical origin unresolved.” A short or noisy record may be insufficient to justify the extra state.

Do not infer precision from the rounded 6 s calculation. Carry uncertainty from K, τ, L, sample interval, sensor noise and final-window choice. A selected structure is still provisional until unused records pass Lesson 18.

## Conclusion

Model structure connects a measured feature to a physical state. A sensor response can produce a second lag without changing water dynamics, but the record alone may not distinguish it from a process store. Use the simplest structure that meets evidence and physical provenance requirements.

## Reason for the next lesson

An additional state can explain a measured lag, but the fitted curve alone may not tell us whether it belongs to the water or the probe, or which parameter values are distinguishable from the record.

[Identifiability](../25-parameter-identifiability/README.md) uses these same candidates to decide what information the experiment actually supplies.
