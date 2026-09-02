# Audit a complete Automatic Tune sequence

Course position: Lesson 52 of 84. Next: use measured disturbances in feedforward
control without weakening the feedback loop.
Prerequisite: `control-tuning-method-comparison`.

**Lesson objective:** Run and audit Automatic Tune for a recirculating water heater, explain why each stage passes or stops, recover from the taught failures, and export one validated but unapplied controller recommendation.

## The problem

The equipment in this lesson is Recirculating electric water heater with independent candidate-validation copy.

Equipment: Recirculating electric water heater with independent
candidate-validation copy.

A one-sided electric heater adds heat to recirculating water. Heater power is
the manipulated variable from 0 through 100 percent. Water temperature is the
process value in degrees Celsius. The controller runs every 0.1 seconds.

The saved local model has plant gain 0.20 degrees Celsius per heater percentage
point, a 5.0-second time constant, and 1.0-second dead time. At the operating
point, 20 percent power holds 20 degrees Celsius. The running controller is the
reviewed Robust PI from Lesson 44. An independent simulated copy executes the
candidate only for validation. This is Simulation evidence, not physical
evidence.

Lesson 51 separated method eligibility, candidate feasibility, and engineering
preference. Repeating every decision manually is slow, but automation becomes
unsafe if it hides reused evidence, performs an unapproved test, weakens a
requirement, or applies its own recommendation. This lesson teaches what the
automation is allowed to sequence and what it must leave visible.

## What we need

We need a visible, recoverable sequence that qualifies data, accepts a model, generates a candidate, validates it, and stops honestly when any prerequisite fails. Completion must produce an unapplied recommendation rather than silently changing the running heater.

## The new idea

Automatic Tune is an orchestrator over the existing **Test**, **Identify**,
**Tune**, and **Validate** functions. It must not contain private copies of
their excitation, identification, formula, numerical, validation, application,
or rollback behavior.

A stage is one bounded engineering decision with declared inputs, outputs,
pass condition, failure reason, and recovery. A stage may reuse evidence only
when source, identity, revision, units, operating region, and freshness match
the next stage's contract.

Before running, the plan states whether evidence is reused, any proposed test
and restoration, model and controller structures, tuning methods, validation
requirements, and whether the result may only be exported. A Simulation plan
is confirmed once. A physical action requires an independently reviewed
topology and explicit approval immediately before that exact action.

Data qualification checks that signals are trustworthy and informative. It
checks quality, freshness, units, ordered sample times, missing or forced
values, limits, operating range, excitation, noise, stop state, and
restoration. This lesson has an exact saved simulated model as configuration
evidence, so no excitation is required. That shortcut is not an identified
physical model and must never be presented as one.

Model structure describes the dynamics represented by a model. Model
parameters give their numerical scale. Identification normally fits one data
interval and uses independent validation data to test prediction. Here the
**Identify** stage verifies the saved first-order-plus-dead-time structure,
revision, operating region, units, positive gain and time constant, and
nonnegative delay. It does not pretend to estimate already known parameters.

The **Tune** stage evaluates SIMC, AMIGO, and the qualified bounded P/PI/PD/PID
comparison. Every attempt remains `Feasible`, `Rejected`, or `Unavailable` with
its raw measures and reason. It may not substitute another formula or change a
requirement to obtain a result.

A lower comparison score is treated as a meaningful improvement only when it
is at least 10 percent lower than the current choice. Smaller differences stay
inside a 10-percent material-benefit band, where the declared deterministic
method order breaks the tie. In this experiment AMIGO's raw score is about 7.4
percent lower, below the 10-percent material-benefit threshold, so SIMC remains
the selected method. The visible explanation matters because the score table
alone would otherwise appear to contradict the recommendation.

The **Validate** stage compares the running controller and candidates with one
immutable validation-conditions record. That record fixes initial state,
setpoint and load events, noise and seed, sample times, duration, limits, and
model variations. Each requirement reports `Pass`, `Fail`, or `Inconclusive`
with value, limit, unit, evidence source, and reason.

An abort stops an actual experiment early. Restoration is the observed return
to the reviewed setpoint, mode, and safe applied output. Sending a restoration
request is not proof that restoration completed. A restoration failure is a
distinct terminal result.

A recommendation means that a candidate passed the declared comparison. It is
not permission to change a controller. **Apply** is a separate review and
authorization with current and candidate settings, revision checks, read-back,
and rollback. This lesson neither applies nor rolls back gains.

## Prediction

Predict this exact stage sequence:

$Test -> Identify -> Tune -> Validate$

The Test stage should reuse the exact saved Simulation model with zero
excitation. Identify should accept plant gain 0.20 degrees Celsius per percent,
time constant 5.0 seconds, and dead time 1.0 second without fitting.

For the Balanced response objective, predict a SIMC PI recommendation with
proportional gain ($K_p$) `5.769 %/degC`, integral time ($T_i$) `5.000 s`, and
integral gain ($K_i$) `1.154 %/(degC*s)`. Model prediction should settle in about
`13.5 s`, overshoot by about `0.094 percent`, and end with normalized absolute
error below `0.001`. These are predictions to verify, not PLC constants.

## Implementation

The confirmed Robust controller and the independent validation candidate use
the same real `Control.PID` form. Only the candidate gains differ.

```st
CurrentParameters.ProportionalGain := LREAL#3.260869565;
CurrentParameters.IntegralGain := LREAL#0.652173913;
CurrentController.Configure(Parameters := CurrentParameters);

CandidateParameters := CurrentParameters;
CandidateParameters.ProportionalGain := LREAL#5.769230769;
CandidateParameters.IntegralGain := LREAL#1.153846154;
CandidateController.Configure(Parameters := CandidateParameters);
```

Both paths retain requested and applied heater power separately. The program
contains no model fit, stage verdict, method winner, or controller write.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Confirm one exact-simulation Automatic Tune plan and separately exercise each deterministic failure and recovery branch. The declared event lasts 220 seconds. It restores Water-temperature setpoint to 20 degC afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Water-temperature setpoint (SP, degC); Current water temperature (PV, degC) and Candidate water temperature (PV, degC); Current applied heater power (MV, %) and Candidate applied heater power (MV, %), including units, limits, mode, and quality. The initial operating point is Experiment enable = 0 (dimensionless), Water-temperature setpoint = 20 degC, Current requested heater power = 20 %, Candidate requested heater power = 20 %, Current applied heater power = 20 %, Candidate applied heater power = 20 %, Current water temperature = 20 degC, Candidate water temperature = 20 degC. The unresolved baseline problem is: A manually reviewed controller does not yet prove that the complete automatic evidence, candidate, failure, and recovery sequence is inspectable and fail-closed.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept the workflow only when every stage and failure remains inspectable, the recommendation is validated, and the running controller revision remains unchanged.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **220**. The observed lesson signal is **Water-temperature setpoint**. Control Studio uses the existing Boolean project authority **P1.ExperimentEnabled** at **%MX96.0**; it does not inject a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to A minus-1-degC equilibrium-temperature load at 100 s. Confirm the live values are current and the reviewed plan stays inside Current applied heater power 0 % to 100 %; Candidate applied heater power 0 % to 100 %; Current water temperature 0 degC to 60 degC; Candidate water temperature 0 degC to 60 degC.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Leave **Model selection** at **Automatic · simplest acceptable**, select **Qualify recording**, and inspect usable samples, duration, sample interval, signal/noise, and qualification details.
   - **Decision before action:** Decide whether excitation and data quality are sufficient before selecting **Fit estimation run**; the first fit is only a preview.
   - **Successful evidence:** Record a distinct equivalent validation run, qualify it, select **Fit validation run**, and inspect parameters, units, residuals, uncertainty, and validity range. Select **Accept model for design** only when independent validation meets the lesson criteria.
   - **Recovery:** If qualification or fitting is rejected, retain the displayed reason and repeat Test rather than reusing validation data or weakening a limit. Remove the selected saved plant, provide a negative gain candidate, return to Identify, and confirm Request excitation is rejected with the displayed reason. Remove the selected saved plant, provide a negative gain candidate, return to Identify, and confirm Request excitation is rejected with the displayed reason. Then inject poor data, confirm tuning cannot proceed to Validate, and retain the terminal message No acceptable controller found. Remove the selected saved plant, provide a negative gain candidate, return to Identify, and confirm Request excitation is rejected with the displayed reason. Then inject poor data, confirm tuning cannot proceed to Validate, and retain the terminal message No acceptable controller found. Exercise user abort, automatic abort, and restoration failure as distinct terminal states.
   - **Record:** State the accepted model structure and parameters, or the exact reason no model was accepted.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** Continue only when Identify shows an accepted model. Set **Design method** = **SIMC** and set **Response** = **Balanced** for the first candidate.
   - **Decision before action:** Calculate the expected gains and predict the response/robustness trade-off before selecting **Generate candidate**.
   - **Successful evidence:** Generate the unapplied candidate and compare Response, controller output, Bode evidence, and margins using the same accepted model.
   - **Recovery:** If generation is disabled, return to Identify and accept a qualified independent model. If the method is rejected, retain the reason instead of substituting another formula.
   - **Record:** Save the candidate settings and explain why the trade-off is eligible or rejected without applying it.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** Continue only after Tune shows an unapplied candidate, then select **Run required validation**.
   - **Decision before action:** Open **Requirement details** and use the predeclared comparisons: current_final_mean_absolute_error <= 0.25 degC (tolerance 0.01 degC); candidate_final_mean_absolute_error <= 0.25 degC (tolerance 0.01 degC); candidate_iae_improvement < 0 degC*s (tolerance 0.01 degC*s); candidate_movement_cost > 0 pp (tolerance 0.01 pp).
   - **Successful evidence:** Inspect Response, constraints, and robustness using the same retained model and conditions. Every requirement reports Pass, Fail, or Inconclusive with value, unit, limit, source, and reason.
   - **Recovery:** If validation is disabled, stale, or failed, return to the named prerequisite stage, correct the cause, and retain both rejected and replacement evidence; do not open **Review application** for a failed result.
   - **Record:** Select **Save result** only after source, units, tolerances, and restoration agree, then state the final disposition and failed requirement if any.

## Evidence and decision

### Measures and comparison

Use one saved conditions record for the running Robust PI and the unapplied
candidate. Retain setpoint, process value, requested and applied power, IAE,
ISE, ITAE, overshoot, settling, final-window error, applied-power movement in
percentage points, saturation, margins, noise response, and bounded variation.

Also retain exact orchestration evidence: planned and executed stage order,
evidence revisions, confirmation count, excitation count, completed-stage
prefix, failed stage, stop reason, abort state, restoration state, candidate
$applied = false$, and active-controller revision before and after each path.

### Validation

The successful path must execute exactly Test, Identify, Tune, and Validate;
reuse the exact Simulation model with zero excitation; retain every method
attempt and requirement; reproduce the predicted Balanced PI within relative
tolerance `1e-7`; and leave the active-controller revision unchanged.

Every failure must stop at the first failed stage, execute no later stage,
preserve completed evidence, and offer the nearest recovery. Abort must stop
output changes. Restoration must be proven from observed state. Silent
excitation, repeated approval, weaker requirements, missing reasons, fabricated
models, evidence promotion, a changed active controller, or $applied = true$
fails the lesson.

### Retained evidence

The isolated validation compares the running Robust PI with the candidate
Balanced PI under one saved conditions record. The candidate trace demonstrates
performance; it does not grant permission to replace the running controller.

![Automatic Tune isolated validation](figures/automatic-water-heater-evidence.svg)

[Complete trace](figures/automatic-water-heater-evidence-trace.csv) ·
[Decision summary](figures/automatic-water-heater-evidence-summary.csv) ·
[Orchestration record](figures/automatic-water-heater-orchestration-evidence.json)

## Conclusion

This path is short because the exact saved Simulation model already supplies
accepted configuration evidence. Identification is not optional on an unknown
plant. With recorded or physical evidence, the same orchestrator must request a
bounded approved Test and independently validate the fitted model.

Stopping early with a specific recovery is correct automation. Continuing with
bad data or an unsupported model would create a plausible-looking but
unqualified controller.

## Reason for the next lesson

Automatic Tune is useful when it makes the engineering sequence reproducible
while leaving every decision inspectable. It must never turn Simulation
evidence into physical evidence, perform an unapproved action, conceal a
rejected result, weaken a requirement, or silently apply gains. Completion
means validated and unapplied; Apply and rollback remain separate authorized
operations.
