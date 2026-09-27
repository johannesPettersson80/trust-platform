# 17. Identifiability

Different heater models can fit a short, noisy temperature record. You will decide whether an experiment can distinguish those models and choose an additional observation if it cannot.

## The problem

Gain and time constant can trade off in an early response. A short record may show the same initial slope for different $K,\tau$ pairs, while a long record may separate their final temperatures. Structural ambiguity is different: two hidden physical lags can produce exactly the same measured response, so more of the same sensor record cannot assign their origins.

## What we need

We need to determine whether our measurements can distinguish competing
model parameters. If they cannot, we need to identify an additional observation
or experiment that would separate the possibilities.

## The new idea

**Identifiability** is the ability to distinguish different model parameter
values from the available inputs and measurements. A good fit does not
necessarily identify one unique model.

**Practical ambiguity** comes from limited duration, noise or weak excitation;
better observations can help. **Structural ambiguity** remains even with
perfect data from the same input and measured output. We will see both kinds
in the heater and sensor example.

For a delayed first-order step, $x(t')=K\Delta u(1-e^{-t'/\tau})$, where $t'=t-L$. When $t'\ll\tau$, use $e^{-t'/\tau}\approx1-t'/\tau$, giving

$$
x(t')\approx\frac{K}{\tau}\Delta u\,t'.
$$

Thus early data mainly identify the ratio $K/\tau$. Consider local hypotheses K=.2 °C/point, τ=5 s and K=.4 °C/point, τ=10 s for the same 10-point step after the same delay. At 1 s after delay, the predictions are $2(1-e^{-0.2})=0.36254$ °C and $4(1-e^{-0.1})=0.38065$ °C. Their difference is 0.01811 °C, smaller than twice the ±0.05 °C measurement-error bound. The candidates' uncertainty bands overlap at that time, so this early portion does not guarantee a distinction.

At 10 s after delay, the predictions are $2(1-e^{-2})=1.72933$ °C and $4(1-e^{-1})=2.52848$ °C, differing by 0.79915 °C. These are local curve hypotheses, not two asserted physical heater parameters. A separately known steady heat balance can reject the K=.4 candidate even when the short record cannot.

A longer hold can resolve this practical ambiguity if PV bounds allow it. If the larger-gain hypothesis would end at 39 °C, choose a 5-point step when planning a new bounded experiment; its predicted difference scales with command amplitude. Structural ambiguity is different: a process lag of 5 s followed by sensor lag 2 s and a process lag 2 s followed by sensor lag 5 s give the same measured response when both start from the same zero-deviation initial states and their total gain matches. For arbitrary input v, eliminate the intermediate water state from the two lag equations. The measured deviation m obeys $\tau_p\tau_s\ddot m+(\tau_p+\tau_s)\dot m+m=Kv(t-L)$. Swapping 5 s and 2 s leaves both the product 10 s² and sum 7 s unchanged, so the measured input/output equation is identical. More data from that same channel cannot assign the two time constants to water or probe. A fast independent water reference or known probe calibration can.

The worked $(K,\tau)=(.2,5)$ and $(.4,10)$ cases are both first-order parameter hypotheses. They are separate from Lesson 16's Control Studio structure comparison, where Candidate A is the retained first-order reference and Candidate B is the additional-lag structure. The worked pair tests practical parameter ambiguity; the Lesson 16 pair tests whether the measured record supports an added structure.

## Prediction

Calculate the two candidate temperatures at 1 s and 10 s after delay. Compare their difference with the 0.1 °C combined bound. Predict how a longer hold, smaller bounded step, or independent reference changes the decision. State whether the question is practical parameter ambiguity or structural physical-origin ambiguity.

## Implementation

Create or open this lesson's project, open [src/tests.st](src/tests.st), and run **ST: Run All Tests**. One calculation checks the two first-order hypotheses at 1 s and 10 s after delay. The other checks the swapped lag assignments. Both should pass; these are calculated hypotheses, not measured-record fits.

`SeparationC := ABS(CandidateAC - CandidateBC);` measures the prediction difference regardless of sign. `ClearlySeparated := SeparationC > 2.0 * MeasurementErrorBoundC;` checks whether the two prediction intervals are disjoint at that time. With the worked bound, the 0.01811 °C early difference is too small to guarantee separation; the 0.79915 °C late difference is large enough. Change `LateObservationS` from `10.0` to `1.0`, predict what happens, and rerun. The late-separation check now fails because that observation is also early. Restore `10.0` and rerun.

For measured-record work, preserve command amplitude, sampling, baseline and final-window uncertainty, and parameter provenance. Use **Check recording** to reject missing timestamps, invalid samples or an unusable step. Overlapping prediction intervals do not justify retaining a candidate whose residual contradicts the actual measurement bound.

The native simulation supplies water temperature at `%IL0`, ideal probe temperature at `%IL8`, and noisy measured probe temperature at `%IL16`. `Main.st` reads those inputs and exposes their diagnostic difference as `MeasurementErrorC` at `%ML0`. It does not emulate the probe or publish candidate-selection results.

The native state-space plant owns the probe state and adds deterministic bounded measurement error of ±.05 °C after that state. `MeasurementErrorC` is a diagnostic difference, not a second measured process variable. Candidate selection and distinguishability analysis remain Control Studio tasks.

Set `SafetyAbort` to stop heating immediately; its latch also sets on invalid
measurement. An abort commands zero heating. Normal
completion restores 75% heating. After an abort, keep stopped and restart from
the full hot reset before clearing the latch and collecting a new trial.

## Experiment in Control Studio

The calculation exercise and the Control Studio record answer different questions. The calculations use explicit local hypotheses at 1 s and 10 s; the Control Studio fit uses measured samples and its retained first-order reference. Do not present the calculated values as measured evidence.

1. Return to the Lesson 16 project using **File → Open Recent** or **File → Open Folder…**. In **Control Studio → Validate → Saved results**, select its saved comparison and choose **Open**, then return to **Identify**. This lesson inspects that existing study; no new import, fit or heater command is needed.
2. Use **Fit** and **Residuals** to inspect the measured record and retained predictions. Expand **Parameters and uncertainty** and read the parameter ranges, consistent-set counts and search-boundary warnings. These ranges describe the searched models and bounds; a narrow search alone cannot rule out alternatives it never included.
3. Relate the worked calculations to the record's duration and uncertainty. The two fixed first-order hypotheses differ by about 0.018 °C at 1 s and 0.799 °C at 10 s after delay. Compare those differences with the 0.1 °C combined bound, then ask whether the actual record covers informative times. The plotted Lesson 16 A/B are different structures, not those two fixed worked hypotheses. The retained 0.2 °C peak and 0.1 °C final model-error limits also remain separate from measurement uncertainty.
4. Distinguish practical ambiguity from structural ambiguity. A longer bounded hold or different excitation can separate K and τ when the record is too short. A fast independent water reference or known probe calibration is needed to assign two indistinguishable lags to water or probe; more samples from the same measured channel cannot do that.
5. Record the supported ranges, rejected alternatives, physical assumptions and proposed next measurement in your lesson notes. Keep the saved study unchanged. Independent validation on unused data is Lesson 18's task.

## Evidence and decision

Declare candidates distinguishable only when prediction differences exceed the predeclared uncertainty in an informative portion of the record and the result persists under the stated sampling and window rules. A solver converging is not evidence of identifiability. If separation appears only after the record ends, report practical ambiguity and plan a longer bounded hold. If no measured channel can assign physical origins, report structural ambiguity and obtain an independent reference.

Carry the retained parameter interval and ambiguity into Lesson 18. Independent validation tests the frozen alternatives on unused data; it may validate the measured input/output prediction without identifying the hidden physical origin. A claim about unmeasured water temperature still needs the independent reference or physical calibration.

## Conclusion

Identifiability asks what this experiment can distinguish. Early heater data reveal $K/\tau$, longer data reveal equilibrium gain and time scale, and a single measured channel may never identify which physical store caused a lag.

## Reason for the next lesson

We can now distinguish parameters supported by the record from alternatives that remain unresolved. That does not yet show whether the retained model predicts a different command or disturbance accurately enough to guide tuning.

[Model validation](../26-independent-model-validation/README.md) keeps the model and its uncertainty unchanged while testing unused records.
