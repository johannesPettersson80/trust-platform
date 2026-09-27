# 10. Loop diagnosis

You will choose a check that separates a tuning problem from a measurement, actuator, capacity, or disturbance problem. Diagnosis comes before changing gains, even when an earlier response test passed.

## The problem

PV below SP is a symptom. It can result from a heater already at full power, a restricted command, a stale or biased sensor, an unexpected load, or unsuitable gains. A stale reading is an old measurement presented as current; a sensor bias is a systematic offset from the actual temperature. Increasing gain is useful only when the signal and actuator path are credible and the intended condition is feasible.

## What we need

We need a check that distinguishes unsuitable gains from measurement,
actuator or capacity problems. Its possible results must support different
decisions, so we know whether changing the gains is justified.

## The new idea

**Loop diagnosis** identifies the cause of unsatisfactory control using checks
that distinguish possible explanations. A low temperature alone cannot tell
us whether to change gains, repair an actuator or investigate a measurement.

For each observation, choose a check whose result would separate those causes:

| Observation | Candidate explanation | Discriminating check |
| --- | --- | --- |
| Request is above applied output | Saturation or another output restriction | Compare the configured output limit with the applied command. |
| Applied command changes but PV does not | Delivery fault, normal delay, or changed process | Check delivered power and compare timing with the known 1 s delay. |
| Two sensors disagree | Bias, location, or acquisition-time difference | Compare the same physical quantity at the same time with a verified reference. |
| Display repeats a value | Steady process or stale sample | Inspect the measurement acquisition time. |
| Load changes before PV moves | Disturbance | Check event size and available heating capacity. |

A 100% command is not proof of delivered rated power. A flat display is not proof of a fresh measurement. The acquisition timestamp records when the sensor obtained a reading; refreshing the display does not make an old reading new. If each sensor has ±0.1 °C verified error, their largest permitted disagreement is $0.1-(-0.1)=0.2$ °C. A larger disagreement contradicts at least one assumption but does not identify which instrument is wrong. Maintaining 35 °C normally needs 75% output. Since one output point supplies 2 kW, adding 30 kW requires $30/2=15$ more points: $75+15=90\%$. The 38 °C target alone needs 90%; combining that target with the load would need $90+15=105\%$, beyond the 100% actuator limit. Increasing a gain cannot create that missing capacity.

A useful diagnostic check gives different predictions for the competing causes. Start at the first unexpected event and follow causality forward: did the command change, did the actuator apply it, did delivered heat change after its delay, and did the sensor acquire a new value? If PV changes before the new command, that command is a response to the temperature change. Look for an earlier disturbance or an earlier command whose heating effect was delayed. This ordering keeps a plausible story from becoming an unsupported diagnosis.

## Prediction

Choose one prior requirement or response feature to investigate. State a primary cause, an alternative cause, and the observation that would distinguish them. Does changing Kp make a 105% steady requirement feasible? Does an unchanged PV alone prove a valid steady state? What evidence would make a gain change reasonable?

## Implementation

Write ST assignments for `ErrorC`, `PairedRequestPct - AppliedPowerPct`, and Boolean comparisons indicating positive error and an applied output at its upper limit. Expose the quantities needed for reasoning: error, requested-minus-applied output, active limit, delivered-power feedback if available, and measurement acquisition time. Keep these observations separate; do not compress `AtUpperLimit AND PositiveError` into an unexplained “insufficient capacity” flag. That condition can also occur during a valid transient. Check error 5 °C with request/applied pairs 120/100% and 90/90%: both have positive error, but only the first has an unavailable command. Neither pair alone proves that steady heating capacity is insufficient.

The prepared project executes the same `Control.PID` as Lesson 9, through
`Configure`, one-time `Initialize` and cyclic `Update`, with unchanged gains and
`TrialKind` set to `0` for the target or `1` for the load. Select the initial
value in [Main.st](src/Main.st) before starting a fresh simulator run.
The channel-0 `set_sensor_bias` event in [simulation.toml](simulation.toml)
defaults to `value = 0.0`; changing it to `0.3` °C provides the optional
reference-disagreement check through the native sensor input. The reference is simulation truth, not a calibrated physical sensor.
Independent acquisition-time and measured delivered-power channels are
unavailable here: record that limitation rather than infer them from a scan
clock or an actuator command. A biased repeat does not satisfy the nominal
measurement assumptions merely because its physical water starts at 35 °C.

The PLC reads measured temperature at `%IL0`, the unbiased simulated reference
at `%IL16`, and applied heater feedback at `%IL8`. Its actual PID command drives
`%QL0`; extraction drives `%QL8`. Both temperature channels come from one native
process state, with the heater's 1 s delay and immediate load path preserved.
The independent reference is simulation evidence, not a calibrated second probe.

`RequestedPowerPct` observes the unlimited request for this fixed PI setup;
`PairedRequestPct` retains the preceding request corresponding to the currently
observed actuator input. Compare the paired value with applied feedback so one
scan of I/O delay is not mistaken for lost heater capacity. These diagnostics
own no control output. The complete `Parameters` record remains authoritative.

## Experiment in Control Studio

1. In the Lesson 9 project, use **Validate → Saved results → Open** to inspect each of its two saved responses, then view **Monitor → Response**. Confirm event type, initial 35 °C/75% state, gains, duration and load. These records belong to that project; inspect them there before opening this diagnosis project for a repeat.
2. In this project, inspect the declared signals in **Monitor → Signals**: measured temperature and simulated reference in °C, requested/applied output and I in percent, and heat load in kW. The simulated reference supports a controlled bias experiment. Delivered-power measurement and sensor acquisition time are unavailable; record those as unresolved checks.
3. Locate the first departure from the expected trace. Decide whether the measurement changed first, a limit blocked the request, or the physical response simply follows the known delay.
4. Choose one bounded repeat as your discriminating check. For a reference-disagreement check, keep the chosen `TrialKind` unchanged, set `value = 0.3` in the channel-0 `set_sensor_bias` event in `simulation.toml`, and restart. Predict a 0.3 °C difference against the simulated reference, exceeding the 0.2 °C comparison threshold. In **Test**, use **Review test** for the original 60 s trial, then select **Confirm and start**. Test automatically records the reviewed sequence. After terminal restoration, use **Validate → Save result**. Keep gains unchanged during diagnosis. If both earlier tests passed, identify which recorded evidence supports their measurement and capacity assumptions. Repeat a test only if the chosen observation can resolve an unanswered diagnostic question; keep the Lesson 9 requirements unchanged.
5. Abort outside 0–80 °C, outside 0–100% applied output, or on unusable measurements. Select **Abort test**, cancel the load, disable heating and confirm 0% output. Stop the simulator if it is unresponsive. Before a repeat, reset PV/SP to 35 °C, P to zero, I and applied output to 75%, and the preceding 1 s command history to 75%.

## Evidence and decision

Write the chosen cause, alternative, expected observation, observed result, and remaining uncertainty. Timing correlation can support a hypothesis but does not prove it alone. A measured limit and a verified lack of delivered power support an actuator or capacity investigation; a credible measurement and available heating support a later bounded tuning trial. If the check does not discriminate, choose another check before tuning.

A passed test is evidence for the tested condition, not proof that every signal path is healthy. For example, a sensor can be fresh during one run and stale after a communications interruption, or capacity can be adequate without load and insufficient with an unrecorded load. The diagnostic repeat deliberately tests one such boundary while keeping gains fixed, so a later tuning decision has a known operating context.

## Conclusion

A poor trace is not a tuning diagnosis. The signal path and physical evidence determine whether changing a gain is justified.

For adaptation, use the real `Control.PID` output and explicit sensor/actuator
bindings, provide a valid timing source, and requalify limits, initialization,
stop/restart and input-quality handling for that plant. This example rejects
out-of-range measured temperatures and latches Manual zero heating/extraction;
the runtime rejects nonfinite typed I/O before ST. Physical sensor freshness,
reference calibration and site qualification remain application responsibilities.

## Reason for the next lesson

Once measurements are credible, the actuator is available, and the operating condition is feasible, [manual PI tuning](../../06-pid-tuning-methods/28-manual-pi-tuning/README.md) changes one gain at a time against unchanged requirements.
