# 21. Tuning comparison

You will compare your manual, model-based and relay-derived PI settings on the same heater. You will decide which settings meet the temperature requirements and compare their command movement before choosing a controller.

## The problem

A controller can pass a setpoint step and fail a thermal load, or look quiet in one run because it started with a different integral state. A fair comparison therefore requires both required conditions for every candidate. A candidate qualifies only when it passes every requirement in both trials. If none qualifies, that is the engineering result and the unresolved requirement must remain visible.

## What we need

We need to decide which available settings meet both target and disturbance
requirements under the same conditions. If several qualify, we need a stated
way to choose among them without hiding a failed requirement in an average.

## The new idea

**Tuning comparison** selects between controller settings using matched
experiments and requirements fixed before viewing the results. The method
that produced a candidate does not establish that it is suitable.

First check every requirement. Only among candidates that pass do we compare
preferences such as settling time and command movement. Freeze this table
before predictions and before opening results:

| Trial | Requirement |
| --- | --- |
| Setpoint | SP 35→38 °C at time zero, D=0, duration 60 s; peak PV≤39 °C; absolute error≤.2 °C at every sample from 30 through 60 s |
| Load | SP=35 °C, D=30 kW applied at time zero, duration 60 s; minimum PV≥33 °C; absolute error≤.2 °C at every sample from 30 through 60 s |
| Both | applied command remains 0–100%; PV remains 0–80 °C; data remain valid |

The trials are separate. The load trial begins at the hot reset with D=0, then applies D=30 at time zero; it does not begin with the load already present. The setpoint trial begins at the same reset and changes SP at time zero. Normal transient command saturation at 100% is allowed if the applied command stays within 0–100%, the anti-windup policy operates, and every requirement still passes. Do not average away a failed peak, minimum, or error window.

Lesson 8's anti-windup sequence remains part of every candidate: use tracking time Tt=1 s, dt=.1 s, and the existing routine that reconciles integral state with the applied limited output. This comparison tests controller settings, not a new integral implementation.

Declare the secondary choice before running. Among candidates that qualify in both trials, compare the worst of their two settling times. Among candidates within 2 s of the shortest worst-case settling time, prefer lower total applied-command activity across both trials. This is a stated design choice for this exercise, not a universal controller rule.

Define activity in percentage points:

$$
A=\sum_k |u_k-u_{k-1}|.
$$

Here k labels consecutive controller samples; the command preceding the first sample is the initial 75%.

Use the .1 s samples in the enabled 60 s window, including the first movement from the initial 75% output and excluding shutdown. For 75%→80%→78%, A=5+2=7 percentage points. Activity is command movement; it is not energy consumption or valve wear.

## Prediction

Before running, write the actual Kp, Ki, anti-windup source, and candidate provenance for each candidate. Predict:

1. which candidate is likely to reach the setpoint error window first;
2. which may saturate during the setpoint trial;
3. whether each candidate can recover from D=30 without a permanent error;
4. whether any candidate is likely to fail the peak or minimum requirement; and
5. which candidate would win the declared worst-of-two settling/activity tie-break if all requirements pass.

A prediction is not a result. Keep the requirements and the possibility that none qualifies.

## Implementation

Open src/Main.st and select the candidate in Tune. Write its actual Kp, Ki and
source revision while stopped. The existing `Control.PID` owns integral state,
output limiting and 1 s back-calculation; Main publishes diagnostics from that
state and does not maintain a second PI loop.

For each .1 s scan, calculate error and the existing PI output, apply Lesson 8 tracking with Tt=1 s, and retain requested and applied output. Use **Validate** to calculate peak PV, minimum PV, error-window maximum and settling time. For the ST exercise, initialize `PreviousAppliedPct` to 75.0 and `CommandActivityPoints` to 0.0. Each scan, add `ABS(AppliedPowerPct - PreviousAppliedPct)` to activity, then store the current applied value as previous. `ABS` returns magnitude regardless of sign. Verify that 75→80→78 adds 7 points. Guard invalid PV, timestamps, and command bounds. Keep shutdown samples out of A and do not let state from one candidate or trial enter the next.

The prepared project uses one selected candidate at a time. While stopped, set symbolic `KpPctPerC` and `KiPctPerCs` and record the candidate name/source; defaults5/1 are a nominal illustration, not a measured relay result. `TrialKind` at `%MW80` selects0 for the target trial or1 for the load trial. Restart the simulator for each60s run. `CommandActivityPoints` includes the first movement from75% and retains the final total without counting shutdown.

Set `SafetyAbort` to stop heating immediately; its latch also sets on invalid
measurement. An abort commands zero heating and zero extraction. Normal
completion restores75% heating. After an abort, keep stopped and restart from
the full hot reset before clearing the latch and collecting a new trial.

## Experiment in Control Studio

The project executes the existing `Control.PID` over normal PLC I/O. Its hot
heater reset is 35 °C, 75% applied output and 75% retained integral state. Tune
exposes writable Kp, Ki and candidate-revision bindings; a gain edit during a
trial aborts to zero heat. `TrialKind` selects separate target and load
records. Control Studio retains each run so Validate can compare paired records
and candidate provenance; a retained record remains simulation evidence.

Use the common hot heater state for every run:

- PV=35 °C;
- SP=35 °C;
- applied output=75%;
- integral state I=75%;
- proportional correction=0%;
- delayed command history=75%;
- D=0;
- dt=.1 s; and
- valid, advancing timestamps.

The candidates are your Lesson 11 manual PI settings, your Lesson 19 model-based settings, and your Lesson 20 relay settings when a valid cycle was obtained. A method may have produced no supported candidate, for example if its model approximation failed validation or the relay cycle was not repeatable. Record that reason and compare only the candidates you actually obtained. Carry each candidate's actual Kp, Ki, source record, and model revision; do not invent gains or treat supplied examples as learner findings. A simple baseline candidate may be retained as a comparison if its definition is stated, but it must meet the same requirements. The current Tune/Validate UI has no built-in roster that links these three lesson records into one candidate selector; enter or import one exact candidate at a time and preserve its provenance with the saved run.

1. Select a candidate in **Control Studio → Tune** and record its scalar Kp, Ki, source, and anti-windup parameters.
2. Run the **Test** workflow for the setpoint trial. Restart Simulator first and confirm PV35, SP35, I75, P0, delayed history75, D0, valid data, and dt=.1. At time zero apply SP35→38 with D still zero. Run 60 s and record PV, error, requested/applied output, saturation, I, and elapsed time.
3. Cancel the setpoint test, set output to zero, and restart. Restore the complete hot reset. Then at time zero apply D=30 with SP fixed at 35. Run 60 s and record the same signals.
4. Stop either trial on PV outside 0–80 °C, applied output outside 0–100%, invalid/stale data, stopped time, or hard fault. Abort to zero heat, cancel D, and reset before continuing.
5. Repeat both separately reset trials for every candidate. Use **Validate** to compare the retained records; keep each candidate's setpoint and disturbance records paired.
6. Compute the two requirement decisions, settling times, and total A for each candidate. Report every failed requirement separately.

## Evidence and decision

A candidate qualifies only if it passes the peak-PV and error-window requirements in the setpoint trial, the minimum-PV and error-window requirements in the load trial, and the common validity/output limits. Normal 100% transients are permitted; a passing candidate is not rejected solely for reaching that limit.

Among all qualifiers, use the predeclared worst-of-two settling comparison. Among candidates within 2 s of the best worst-case settling time, compare total applied-command activity from both 60 s windows. If no candidate qualifies, state which requirement remains unresolved and retain the records for later redesign. Do not select a candidate by averaging a failed trial with a passed one.

## Conclusion

A fair controller comparison uses identical hot resets, separate setpoint and load trials, explicit anti-windup, fixed requirements, and applied-command evidence. Manual, model-based, and relay-derived candidates remain tied to their learner records. The result may be a qualified candidate, a baseline, or none.

## Reason for the next lesson

A qualifying controller may still make a larger proportional command change than desired when the setpoint steps. Reducing Kp would also weaken its correction of a measured-temperature change caused by a disturbance. We need to separate those two proportional effects while keeping the selected gains.

[Setpoint weighting](../../01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting/README.md) changes the setpoint contribution to P and tests whether the smaller command step is worth its response cost.
