# 15. Step-response identification

You will estimate the heater’s gain, time constant and input delay from a measured step response. You will use measurement uncertainty to decide how precisely those parameters can be known.

## The problem

The step experiment gave us an applied-power record and a temperature response.
We now need to turn them into model parameters. A noisy first movement may
precede the real response, and a short record may end before equilibrium, so
we must distinguish what the data establish from what we merely assume.

## What we need

We need to turn the measured step response into estimates of process gain,
response time and delay, and establish how precisely the data support those
estimates. A visible first movement or last sample alone is not enough.

## The new idea

**Step-response identification** estimates a process model from its response
to a held input change. For our delayed first-order model, we estimate:

- **Gain $K$:** final temperature change per applied-command change.
- **Time constant $\tau$:** the scale of the gradual approach to that value.
- **Delay $L$:** when the changed input begins affecting the output.

We calculate a first estimate, then compare the model with the complete record.
The result includes uncertainty rather than one falsely precise set of numbers.

Use the qualified five-second pre-step window from −5 to 0 s for the baseline mean $T_0$. For the final mean $T_f$, use the last five seconds of the hold, excluding samples at or after command restoration. Require valid samples, temperature range at most .10 °C and absolute window slope at most .01 °C/s, using the slope calculation from the preceding lesson. If the final window still drifts, obtain a new, longer bounded record rather than calling its last value an equilibrium. Then

$$
\Delta T=T_f-T_0,\qquad K=\frac{\Delta T}{\Delta u}.
$$

For the supplied illustration, 75% held at 35 °C and 85% held at 37 °C gives Δu=10 points, ΔT=2 °C, and $K=0.2$ °C/point. If each baseline and final window is only known within ±0.05 °C, then ΔT is 1.9–2.1 °C and K is 0.19–0.21 °C/point. Do not report more precision than the windows support.

Define a sustained departure as the first of three consecutive .1 s samples above $T_0+.15$ °C for this positive heater step. The .15 °C threshold exceeds the .10 °C possible difference between a baseline and reading each uncertain by ±.05 °C. Report that first timestamp and the .1 s sampling resolution. This gives a **detectable delay**, not necessarily true transport delay L. Thermal rise, sensor filtering, sample spacing, and threshold bias can make that crossing later than L. Use the whole record to fit or compare the delayed response rather than renaming the threshold time as physical delay.

For the model derived in the energy-balance lesson, the temperature gap remaining after the delay is

$$
T_f-T(t)=(T_f-T_0)e^{-(t-L)/\tau}.
$$

Divide by the initial gap, then take the natural logarithm, which undoes the exponential:

$$
q(t)=\ln\left(\frac{T_f-T(t)}{T_f-T_0}\right)=-\frac{t-L}{\tau}.
$$

Here q is a dimensionless logarithm of the remaining fraction. Plotting q against time gives a straight line for an ideal first-order response. Its slope is −1/τ. This calculation uses samples after the response begins and before the remaining gap becomes comparable with measurement noise.

Consider illustrative rounded readings T(6 s)=36.264 °C and T(11 s)=36.729 °C, with T0=35 °C and Tf=37 °C. They give q(6)=ln(0.736/2)≈−1 and q(11)=ln(0.271/2)≈−2. The slope is approximately

$$
\frac{-2-(-1)}{11-6}=-0.2\ \mathrm{s}^{-1},
$$

so τ≈−1/(−0.2)=5 s. Rearranging q=−(t−L)/τ gives L=t+τq; at 6 s this gives L≈6+5(−1)=1 s. Both points are needed to obtain the slope. These rounded teaching values demonstrate the calculation; use the complete valid record and its uncertainty for your model.

The 63.2% temperature is $T_{63}=T_0+0.632\Delta T$. Once L has been estimated from the response, $\tau=t_{63}-L$ gives another check. A threshold onset can help initialize a fit, but does not by itself establish either parameter.

To fit the whole record, calculate a candidate temperature at every sample time:

$$
T_{pred}(t)=
\begin{cases}
T_0,&t<L,\\
T_0+K\Delta u\left(1-e^{-(t-L)/\tau}\right),&t\ge L.
\end{cases}
$$

A **fit** chooses parameters that make these predictions agree with the observations. For each candidate, calculate measurement minus prediction, square each difference and add them. Squaring prevents positive and negative differences from canceling. The candidate with the smallest sum is the best fit within the searched set, not automatically a physically adequate model.

In Identify, start with K from the two window means, search $0\le L\le2$ s and $.1\le\tau\le20$ s, both in .1 s increments, and compare all valid samples during the hold. These initial search limits cover the planning model's 1 s delay and 5 s time constant with room for disagreement. If the best value reaches the upper L boundary or either positive τ boundary, examine the record and refine or extend the justified search before claiming an estimate. Zero delay is admissible: at L=0 report one-sided delay uncertainty rather than searching for negative delay.

A single best fit is not an uncertainty range. To examine the stated ±.05 °C measurement bound, also vary $T_0$ across its baseline-mean bound in .01 °C increments and K across the gain interval from the window bounds in .001 °C/point increments. Retain candidates only if their final temperature lies within the final-mean bound and every predicted hold sample differs from its measured value by at most .05 °C. Report the range of K, L and τ among these retained candidates, together with the grid spacing and timestamp resolution. These are grid-resolved approximations to the consistent parameter ranges, not exhaustive bounds or statistical confidence intervals. Refine the grid around the accepted range and check sensitivity to the .1 s timestamp resolution. If the coarse grid contains no acceptable candidate, refine it and check timing before attributing the failure to the model or error assumptions. A minimum squared error alone cannot resolve that failure.

## Prediction

Before calculating, predict the sign and approximate size of ΔT, the K interval from ±0.05 °C windows, and whether the record supports a final window. Predict how threshold bias changes detectable delay and why two rounded samples do not justify precise τ or L.

## Implementation

Use the same Lesson 14 record: known applied command, 0.1 s timestamps, qualified baseline, and a held step. Estimate Δu in command points, ΔT in °C, gain K in °C/point, a defensible transport delay L in seconds, and time constant τ in seconds. Keep the raw record, windows, threshold, and uncertainty with the estimates.

Use **Identify** for window averages and fitting when the required recording is actually available. Write ST calculations for `CommandChangePct`, `TemperatureChangeC`, `ProcessGainCPerPoint := TemperatureChangeC / CommandChangePct;` and `Temperature63C := BaselineTemperatureC + 0.632 * TemperatureChangeC;`. These are learner calculation variables, not existing symbols in the current `Main.st`; the shipped program exposes `AppliedPowerPct`, `TemperatureC`, and the `Lesson23ProcessGain`, `Lesson23Delay`, and `Lesson23TimeConstant` helpers. Guard division with an `IF` that requires a nonzero command change. The helper contract uses the standard `ENO` output: bind `ENO => GainValid` and use the result only when `GainValid` is true. A zero command change must return normally with `ENO = FALSE`; its zero result is an unused placeholder, not an identified zero gain. A nonzero command change with unchanged temperature instead returns a valid zero gain. This follows the EN/ENO call contract; the lesson helper does not change the runtime's division semantics. Check data validity and a supported final window before accepting the calculation; logarithms also require a positive argument.

The prepared program holds 75% until `ExperimentEnabled` starts the 85% step. Test records 5 s at the baseline before enabling, then holds the step for 40 s within a 45.1 s recording envelope. This uses the upper end of the preceding lesson's 20–40 s hold range; its worked 30 s hold instead needs 35.1 s including baseline and restoration. Normal completion restores 75%. No fitted parameters are pre-filled as measured results. Control Studio observes the project-owned step; its observation amplitude of zero does not add another command to the one in `Main.st`.

Set `SafetyAbort` to stop heating immediately; its latch also sets on invalid
measurement. An abort commands zero heating. Normal
completion restores 75% heating. After an abort, keep stopped and restart from
the full hot reset before clearing the latch and collecting a new trial.

For another plant, first select the I/O driver in `io.toml`, update the addressed
bindings in `Main.st` and `control-loop.json`, and check units and scaling. Set
the initial operating point and dynamics in
`simulation.toml`, and the step, bounds and abort behavior in `Main.st`. Match
the task/sample period and the baseline/hold durations to the plant. The helper
only performs arithmetic on already-qualified finite measurements; it cannot
establish data quality or a steady final value. Requalify the baseline and
response windows, then validate the estimate on a distinct input step before
using it for tuning. The supplied simulation is rehearsal, not qualification
of the new physical installation.

## Experiment in Control Studio

Use the qualified imported record to estimate gain, delay and time constant
with the stated windows and uncertainty grid. Then acquire and qualify a
distinct held-step record, compare it with the frozen estimate, and keep both
records attached to your decision. Reject the estimate when the step, windows
or samples do not support the model.

1. In the Lesson 14 project, open the completed recording in **Control Studio → Validate** and choose **Export result**. Open this lesson's project, then choose **Identify → Import recording…** and select that result JSON. Inspect the measured response and expand **Recording source** to confirm the original project, result revision, units and sample count. Choose **Measure a held step**, expand **Measurement and search settings**, and review the stated window, measurement and grid values. Choose **Check recording**, then **Fit estimation run** for a preview. Import preserves the original samples; it does not load the previous project's controller or accept a model. **Record validation** opens Test for a separately reviewed experiment. Expand **Step-response measurements** to inspect the qualified windows, fitted parameters and consistent-grid ranges. This remains estimation evidence until a distinct validation recording is checked.
2. Only when a qualified record is visibly available, set both window durations to 5 s in **Measurement and search settings**. **Check recording** selects the measured pre-step and pre-restoration windows and reports their means, ranges and slopes. Confirm the applied-step timestamp, .15 °C onset threshold and three-sample rule before viewing estimates.
3. Calculate K, detectable onset, the explained grid fit for L/τ, and $T_{63}$. Plot measured and predicted temperature, baseline, final estimate, onset threshold and 63.2% level. Inspect the differences across the whole response, not only at the two worked timestamps.
4. Report the **Best gain**, **Transport delay** and **Time constant** separately from the **Consistent grid** ranges. Refine the delay/time-constant bounds around the retained range and halve their grid steps; also halve the baseline and gain grid steps. Check and fit again. Repeat with **Sample time offset** at −0.1 and +0.1 s to inspect timestamp sensitivity, then return it to zero for the nominal estimate. Keep the record’s command amplitude, hold duration, windows and search spacing attached. Preserve an empty or unresolved candidate set as a finding.
5. Reject the estimate if the step is ambiguous, final window is still rising, a disturbance overlaps, or samples are invalid. Do not substitute .2, 5, or 1 as learner values.

## Evidence and decision

Accept a model estimate only when the applied step and windows are traceable and the whole response supports a consistent delayed approach. Report K in °C/point and L/τ in seconds with uncertainty. A first threshold crossing is labelled detectable delay; a whole-curve fit supplies the physical estimate only under the stated model assumptions.

If no first-order candidate explains the full record within the stated bounds, retain the pattern of prediction errors. A correct final value with a repeatable mismatch early in the response raises a different question from parameter arithmetic: does the model need to describe another physical response, such as the sensor itself? If many different parameter combinations remain possible, retain that ambiguity for the identifiability lesson. Do not discard either finding to obtain a neat parameter list.

For the separate validation experiment, choose **Record validation**, then
**Review test**. Confirm the 45.1 s maximum duration and 5 s baseline before
activation. **Confirm and start** runs this project's 75% to 85% step and
restores 75%; it does not reuse the imported 75% to 87% excitation. After
completion, return to **Identify**, choose **Check recording**, then **Fit
validation run**. Review the validation response and its measurement windows;
the estimation parameters stay fixed. Choose **Use fitted model** only after
reviewing this independent agreement. Retain both original records.

## Conclusion

Step identification turns a qualified applied-command record into a traceable, uncertain model. Gain comes from equilibrium change; delay and time scale require the complete response and explicit assumptions.

## Reason for the next lesson

The fit has assumed one temperature state and a pure delivery delay. Adjusting K, L and τ can improve agreement within that assumption, but it cannot explain the physical origin of a repeatable response feature the model misses.

[Model structure](../24-choose-model-structure/README.md) examines the same heater when the temperature sensor has its own response and asks whether an additional state is justified. Keep your identified model and record for that comparison.
