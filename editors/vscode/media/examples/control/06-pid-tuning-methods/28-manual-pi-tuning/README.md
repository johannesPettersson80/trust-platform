# 11. Manual PI tuning

In this lesson, you will tune the heater's PI controller by changing one gain at a time. You will predict what the change should do, test it against the existing temperature requirements, and decide whether to keep the new settings.

## The problem

Diagnosis has separated measurement, delivery and capacity questions from
tuning. We now test whether a deliberate gain change improves the response.

The anti-windup controller has fixed $K_p=2$ percentage points/°C and $K_i=0.5$ percentage points/(°C s). In the prepared Lesson 9 simulation, these gains pass the setpoint requirement but narrowly miss the disturbance-settling requirement: the largest error during 30–60 s is about 0.212 °C against a 0.2 °C limit. Confirm that baseline in your own matched records before changing a gain. A stronger response can reduce error sooner while increasing peak temperature or actuator use.

## What we need

We need a deliberate way to improve the response by changing a gain, predict
the benefit and cost, then decide whether to keep the setting. The comparison
must hold the physical conditions and requirements fixed.

## The new idea

**Manual PI tuning** means choosing the proportional gain $K_p$ and
integral gain $K_i$ by deliberately changing a setting and observing the
response in repeatable experiments against unchanged requirements. We choose which gain to change from the
response we want to improve. The baseline and every candidate use the Lesson 9 requirements:

| Trial | Requirement through 60 s |
| --- | --- |
| SP 35→38 °C | Peak ≤39 °C; $\vert PV-38\vert≤0.2$ °C by 30 s and thereafter |
| SP 35 °C, load +30 kW | Minimum ≥33 °C; $\vert PV-35\vert≤0.2$ °C by 30 s and thereafter |

Compare the baseline first. A factor 1.2 applied to $K_p=2$ gives 2.4 percentage points/°C; a factor 0.8 gives 1.6. A factor 1.2 applied to $K_i=0.5$ gives 0.6 percentage points/(°C s); a factor 0.8 gives 0.4. These are candidate settings, not guaranteed improvements. For the 3 °C setpoint step, increasing Kp from 2 to 2.4 adds $0.4(3)=1.2$ percentage points to the first proportional correction: the first complete request changes from 81.15% to 82.35%. Increasing Ki from 0.5 to 0.6 instead adds only $0.1(3)(0.1)=0.03$ point on the first scan, making 81.18%, but the extra accumulation continues on later scans while error remains. Anti-windup remains enabled and applied output must stay 0–100%.

Changing Kp changes the output produced by the current error immediately. Changing Ki changes the rate at which the retained contribution moves while error persists. Because this heater stores energy and has a delivery delay, the effect of earlier heating commands can continue after the error has changed sign. That is why peak, minimum, band entry, and output range must all be recorded instead of selecting a gain from the first visible movement.

## Prediction

Which signal changes first after the setpoint step and after the load event? Predict what increasing Kp changes immediately and what increasing Ki changes over time. Which requirement might become worse with a stronger gain? Define in advance what result would make you retain a candidate, reject it, or stop because the data are inconclusive.

## Implementation

Keep the heater, initial state, event, duration, limits, and reset identical. Change one gain per trial, record what improved and worsened, and test at most three changed candidate parameter sets in addition to the baseline. Each candidate requires a setpoint trial and a separately reset disturbance trial before it can qualify. Choose a factor from 0.8 to 1.2 for one gain per candidate, keeping Kp within 1–4 percentage points/°C and Ki within 0.2–1 percentage points/(°C·s). These are bounds for this exercise, not a guarantee of stability; never alter both gains to explain one result.

The project runs the same `Control.PID` with proportional and integral action,
back-calculation anti-windup and a 1 s tracking time. Each 100 ms task configures
the complete accepted parameter record before `Update`; initialization runs
once at the hot reset. Changing a gain does not replace the other parameters
or reset the retained controller state.

The native heater supplies measured temperature on `%IL0` and applied heating
feedback on `%IL8`. The PID drives the heater through `%QL0`; `%QL8` requests
heat extraction. [simulation.toml](simulation.toml) owns the 1 s heating delay,
immediate load response, 0–100% actuator and 35 °C/75% initial state.

Restart to the hot reset first, then open **Tune → Inspect runtime parameters**.
Edit **Proportional gain** or **Integral gain**, then use
**Review candidate → Apply reviewed values**. These numeric fields address the
actual `LREAL` record leaves `P1.Parameters.ProportionalGain` and
`P1.Parameters.IntegralGain`
while the experiment is disabled. Only one field changes for each candidate;
the review shows both current and proposed values. Wait for confirmation that
the values were observed after ST execution before starting the trial.

Applying before restart would lose the candidate. Set `TrialKind` to `0` or `1`
in [Main.st](src/Main.st) before each restart, then reapply the same accepted
candidate for its separate target/load records. `ObservedKp` (`%ML96`) and
`ObservedKi` (`%ML104`) record the actual accepted LREAL gains with every sample;
the gain mirrors are observations, not tuning inputs. Inspect their numerical
values in **Monitor → Signals**, leaving them unselected when plotting the
temperature and heater response.

For each trial, retain the exact gains, initial I=75% and event type. Keep the
reset, tracking time, setpoints, load, sample interval and output limits fixed.
The valid 100% output limit is an observation, not an automatic stop. Abort for
the stated temperature, applied-output or measurement limits. Keep each
candidate and its one changed gain with the record; a method label alone does
not qualify it.

## Experiment in Control Studio

1. Leave the source `TrialKind` initializer at `0`, start Simulator and inspect the baseline gain values in **Tune → Inspect runtime parameters**. In **Control Studio → Monitor → Response**, inspect SP/PV in °C and requested/applied output and I in percent. Use **Signals** for the recorded gains; select Error separately when judging the ±0.2 °C band. In **Test**, review the baseline setpoint trial: reset to SP=PV=35 °C, I=75%, applied output=75%, P=0% and no load, with the preceding 1 s command history at 75%. Use **Review test**, then **Confirm and start** to establish time zero and apply SP 35→38 °C for 60 s. Test automatically records the reviewed sequence.
2. After terminal restoration, use **Validate → Save result**. Stop Simulator, set the `TrialKind` initializer to `1`, restart and restore the same state. Run the baseline disturbance trial: keep SP=35 °C and introduce 30 kW at time zero for 60 s. Decide whether the baseline qualifies before selecting a gain change.
3. Choose the response feature to improve, one gain and a factor from 0.8 to 1.2 within the stated gain bounds. After each reset, inspect runtime parameters, edit only the chosen numeric gain field, then use **Review candidate → Apply reviewed values** and wait for observed confirmation. Predict the benefit and possible cost. Reset fully and repeat the most informative trial first; if that passes, save its Test result after terminal restoration, reset with the other `TrialKind`, reapply the same candidate gains, and record that event too.
4. Retain or reject that candidate using both event types. For the next change, start from the last retained gains so a rejected change does not silently become the new baseline. Test at most two further candidates, again changing only one gain at a time. You may stop earlier when a candidate qualifies, when no candidate qualifies, or when the evidence is not discriminating.
5. Record peak or minimum PV, first entry into the ±0.2 °C band that remains to 60 s, output range, and any saturation. Abort outside 0–80 °C, outside 0–100% applied output, or on unusable measurements. Select **Abort test** when a stopping condition occurs. After each run, cancel the load, restore SP35, disable heating and confirm 0% output; stop the simulator if it is unresponsive. Reset the complete hot state before the next trial.

## Evidence and decision

Judge each trial against both requirements when both are required for the decision. Keep a candidate only if it passes the stated temperature limits and applied-output bounds with the exact reset and event. A quicker first movement does not compensate for a failed peak or minimum. If the baseline qualifies, you may retain it or investigate a candidate for a stated secondary benefit, such as shorter settling, while preserving every temperature and output requirement. If all bounded candidates fail, report that result and the unresolved cause; it does not prove that no other tuning exists.

Keep the event records separate: the setpoint trial changes error immediately, while the disturbance trial must first change PV. Compare candidates only within the same event and matched initial state.

Use the same candidate on both event types when the tuning decision covers both requirements. If one event fails and the other passes, do not average them into one score: identify which physical demand is limiting the decision. A trial stopped for an output or temperature boundary is a failed requirement for that condition, but it does not prove that every possible gain would fail.

## Conclusion

Manual PI tuning is a finite decision loop: establish a baseline, change one gain within declared bounds, repeat the requirements, and retain or reject the candidate from evidence. The result can be baseline retained, a candidate retained, no qualifying candidate, or an inconclusive experiment.

## Reason for the next lesson

These trials show how the tested feedback controllers behave. They do not
separate the heater's own response from the controller continually changing
its power. [Process dynamics](../../01-feedback-foundations-and-temperature-control/13-process-response-classes/README.md)
identifies the gain, speed and delay information that a useful heater model must supply.
