# 18. Model validation

You will test a fixed heater model against unused command and disturbance records, and decide whether its timing, shape, and final-temperature predictions are accurate enough for the next tuning decision.

## The problem

A model can fit its identification record and still fail on a different command or disturbance. Final-temperature agreement alone can hide an incorrect delay or rise. Looking at validation data and then changing the model turns validation into fitting.

## What we need

We need to establish whether the unchanged model predicts new commands and
disturbances accurately enough for a tuning decision. Agreement with the
record used to fit it does not answer that question.

## The new idea

**Model validation** checks whether an unchanged model predicts new observations
well enough for its intended purpose. We keep the fitting record separate:
otherwise we would only check how well the model reproduces data used to
choose it. Here the intended purpose is a later controller-tuning decision.

Freeze the learner’s equation and K, τ, and L, including uncertainty, before viewing validation traces. Retain any additional states justified in the model-structure and identifiability lessons. If there is a probe-temperature state, predict that state for comparison with the probe measurement; use an independent water reference for claims about actual water temperature. The single-state physical model from Lesson 13 is:

$$
x=T-35,\qquad \tau\dot x+x=K v(t-L)-\frac{K}{2}D(t),
$$

where v=u−75 is applied-command deviation in points, D is extra heat extraction in kW, K is °C/point, and the known 2 kW/point conversion gives $H=2/K$. D acts on water immediately in this model; it does not pass through the heater command delay. For nominal K=.2 and τ=5, a held 30 kW extraction gives initial rate $-KD/(2\tau)=-0.6$ °C/s when x=0 and v=0.

Use three unused 40 s records from the same hot reset: 75→90% with D=0 (nominal final 38 °C), 75→65% with D=0 (nominal final 33 °C), and D=30 kW introduced while command remains 75% (nominal final 32 °C). Set the existing project variable `TrialKind` to 0, 1 or 2 before each run. These values are illustrative; predictions use your unchanged model parameters and each actual applied-command history.

For each record, calculate predicted temperature using its applied-command history and disturbance history. A **residual** is measured minus predicted temperature: $r_i=T_{measured,i}-T_{predicted,i}$, where i labels a sample. For N samples, the mean residual is their sum divided by N. The root mean square error (RMSE) squares the residuals before averaging, then takes the square root:

$$
\overline r=\frac{1}{N}\sum_{i=1}^{N}r_i,\qquad
\mathrm{RMSE}=\sqrt{\frac{1}{N}\sum_{i=1}^{N}r_i^2}.
$$

Both results have units °C. For residuals +0.1 and −0.1 °C, the mean is zero but RMSE is sqrt((0.01+0.01)/2)=0.1 °C. Positive and negative model errors cannot cancel in RMSE.

Declare requirements before looking: every measured/predicted difference within ±0.2 °C through 40 s, final-10-s RMSE ≤0.1 °C, 10% response-crossing error ≤0.3 s, and 63.2% crossing error ≤1 s when the crossing exists. For each trace, normalize progress as (T−T0)/(Tfinal−T0), using a supported late temperature. This handles rising and falling tests with the same sign convention. Mark the first of three consecutive samples at or beyond 10% and 63.2%; apply the identical rule to the model trace. If the final change or a crossing is unsupported, report that metric as indeterminate. Validation temperature bounds are 31–39 °C, deliberately broad enough for the 32 °C disturbance case; command must remain 0–100%.

The early 10% crossing includes delay and initial-rise effects; do not call it a pure L measurement. Check final value, residual pattern, onset, 63.2% crossing, and shape separately. A coherent residual sign may suggest gain bias; a delayed residual may suggest L; curvature may suggest τ or extra lag. These are diagnostic patterns, not automatic refits.

## Prediction

Before opening the records, calculate nominal final temperatures and predict which record is most likely to expose a missing disturbance input. State which model defect would produce each residual pattern; do not claim a residual sign without a mismatch hypothesis. Predict whether the learner’s uncertainty overlaps each requirement.

## Implementation

Keep the first-order model coefficients, revision and uncertainty from the exported Lesson 15 estimation unchanged. Import or recompute that estimation from its original recording with the same Fit settings before viewing any holdout. Do not add prediction logic to `Main.st`: the native simulator supplies the water and measured-probe inputs, while the learner compares the frozen model with recorded data in Control Studio.

For each record, retain the measured PV, applied command, disturbance channel, timestamps and the model revision. Calculate `Residual := MeasuredPV - FrozenModelPV` from the actual command and disturbance history. Use the existing Identify evidence for residuals and validation RMSE. If a crossing or other metric is not available for a record, mark it indeterminate rather than treating an unavailable value as a pass. Do not refit the frozen model on the validation record.

The project setting `TrialKind` selects the trial: 0 selects 75→90%, 1 selects 75→65%, and 2 selects 30 kW extraction with command 75%. Set the initial `TrialKind` value in [src/Main.st](src/Main.st) while stopped, then restart the simulator before each record. Restarting restores 35 °C water/probe state and 75% delayed history. The native simulator owns the delayed heating path, immediate load and probe lag. Main.st reads water at `%IL0` and measured probe temperature at `%IL8`; it does not calculate a frozen-model prediction.

`SafetyAbort` stops heating immediately and latches on invalid measurement. An abort commands zero heating and zero extraction. Normal completion restores 75% heating. After an abort, keep stopped and restart from the full hot reset before clearing the latch and collecting a new trial.

## Experiment in Control Studio

Keep the first-order model revision, coefficients and uncertainty from Lesson 15 unchanged.

1. Open the Lesson 18 project in the workspace. In **Identify**, import the exported Lesson 15 estimation recording/result. Check the recording, restore the same first-order/FOPDT settings and bounds, then **Fit** the estimation record. Inspect **Parameters and uncertainty** and write down the model revision, coefficients and uncertainty before collecting a holdout. This frozen first-order baseline tests whether its timing and shape are adequate.
2. Prepare one unused Lesson 18 record at a time. Set `TrialKind` to 0, 1 or 2 in the Lesson 18 [src/Main.st](src/Main.st) while stopped, then restart the simulator. In **Identify**, choose **Record validation** to open **Test**, then **Review test → Confirm and start** with the project-scripted run: 5 s at the hot baseline, then 40 s after activation. Let the run complete, then restore the normal 75% state and restart from the hot reset before the next case.
3. For the 90% and 65% command holdouts, save the result in **Validate**, return to **Identify**, then **Check recording** and **Fit validation run** to check the distinct recording and evaluate it against the retained Lesson 15 estimation fit. Inspect **Fit**, **Residuals**, **Parameters and uncertainty**, and the displayed validation RMSE. Preserve the original estimation coefficients and model revision; do not change them to improve a holdout.
4. The 30 kW case is a disturbance observation with no command change. Record and retain its applied command, disturbance and measured PV, then compare it with the frozen model using the recorded histories. The current held-step Identify path requires a distinct command change, so do not claim that this disturbance case passed the held-step fitter; mark unsupported metrics indeterminate and retain it as evidence for the disturbance-model question.
5. Decide Pass, Fail or Indeterminate separately for each usable record. A failed or unsupported result identifies a limit of the frozen prediction or current evidence route; it does not justify silently refitting the validation record.

## Evidence and decision

Pass requires every declared criterion for every usable record. Fail identifies an inadequate prediction for the tuning decision; it does not prove the physical plant is wrong. Indeterminate means invalid data, missing crossing, insufficient coverage, or uncertainty prevents a decision. Do not refit using the same validation record. A revised structure or parameter requires a new frozen model and new unused data.

## Conclusion

Independent validation tests a frozen model against changed commands and an immediate physical load. Residual magnitude, timing, shape, and final behavior each matter; final agreement alone cannot validate dynamics.

## Reason for the next lesson

The model can now be judged against unused temperature, timing and disturbance observations. When it meets the declared accuracy requirements, we can use it to calculate a controller for a chosen response time; validation itself has not chosen that response or those gains.

[Model-based PI tuning](../../06-pid-tuning-methods/30-model-based-pi-tuning/README.md) derives the starting settings from your retained model.
