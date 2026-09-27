# 81. Fault-aware control

You will classify range, age and disagreement faults from two heater sensors, transfer to one valid sensor only when justified, and latch zero heat when no trustworthy PV can be selected.

## The problem

Two fresh temperature values can both be plausible and still disagree. Disagreement identifies an evidence problem; it does not identify which sensor is healthy. Selecting the higher or lower value by a naive vote can drive feedback in the wrong direction.

## What we need

We need to decide when the sensor evidence supports a usable measurement
and when normal control must stop. A clearly invalid sensor and two fresh
but disagreeing sensors require different decisions; disagreement alone
does not tell us which sensor is right.

## The new idea

**Fault-aware control** changes its behavior when measurements or components
can no longer support the normal control decision. Here we classify each
sensor's validity and their disagreement before choosing a measurement or
removing heat.

The disagreement is $d=|T_A-T_B|$ in °C. Two sensors each within ±0.1 °C
can differ by 0.2 °C from error alone. A larger unexplained difference does
not identify which one is right, even if both readings are fresh.

Range and age failures are detectable from one channel. Fresh disagreement is different: without an independent reference, a model can diagnose inconsistency but cannot automatically prove which sensor is correct. A validated model may provide diagnostic context, not an automatic truth vote.

Use heater heat capacity 50 kJ/°C and heat loss 10 kW/°C to 20 °C, 2 kW/point, one-second delivery delay, command 0–100%, and lifecycle rules from Lessons 25–28. Each sensor must be finite and in 0–80 °C with age $0\le age\le.5$ s and a credible acquisition timestamp. A held reading remains usable until its age exceeds .5 s; a new acquisition is required for counting recovery samples. With one valid sensor, use it with bumpless integral alignment. If both are valid but differ by more than .4 °C continuously for .2 s, declare disagreement. With .1 s scans and the first excessive difference at 5.0 s, start the timer at zero and latch at 5.2 s if the difference remains excessive at all three observations. Reset the timer when the difference is at most .4 °C or either sensor becomes invalid. Separate excessive-difference intervals must not accumulate as one continuous disagreement. If neither is valid, or both fresh sensors disagree, latch fault and command zero heat.

Normal sensor A is primary and B is a monitor. For the normal case assume both have zero noise. A supplied ±.1 °C error bound for each sensor permits .2 °C disagreement from measurement uncertainty alone. The .4 °C threshold leaves another .2 °C for small differences in sensor response; it is an exercise choice to check against actual sensor behavior. Use the selected PI gains, beta=1, filter bypass, .1 s updates and 1 s tracking time. Requirements remain the fixed 60 s cases: SP35→38 peak≤39 °C and absolute error≤.2 °C from 30–60 s; separate 30 kW extraction at SP35 minimum≥33 °C and the same late error band; fault tests assess policy and continuity rather than eventual temperature accuracy while heat is disabled.

When only A remains valid, select A and align the integral from applied output and current proportional term. For example, with applied 75%, $K_p=2$, SP=35 °C, and A=35.2 °C, $P=-.4$. If B is stale at 35 °C and the old request is 75%, set $I=75-(-.4)=75.4$ so the output remains continuous. If both sensors are invalid or disagree latched, clear Auto/Ready, freeze ordinary state, command zero heat and require the Lesson 28 recovery sequence.

## Prediction

Classify normal fresh agreement, one stale sensor, one 90 °C range fault, one fresh A offset by +1 °C while B remains 35 °C, and both invalid. Predict that only the first three permit a selected valid PV; fresh disagreement cannot identify truth and therefore latches zero heat. Predict the .4 °C sustained threshold and the output-preserving $P=-.4$ alignment.

## Implementation

Track each sensor's value, quality, finite/range flag, acquisition age, last timestamp and validity. Write `DifferenceC := ABS(SensorAC - SensorBC);` and retain the time when a continuously excessive difference first appeared. `ABS` returns the nonnegative magnitude. Compare elapsed time with .2 s only while both sensors are valid; invalid channels follow the validity-selection branch. A valid single sensor may select local PI; the chosen PV change must use $I=u_a-P$, skip ordinary integration on the transfer scan, and resume normal updates afterward. A disagreement latch has highest priority over normal PI and cannot be cleared by merely choosing one reading.

On acknowledgement, require three consecutive new acquisitions from both valid sensors agreeing within .4 °C after a disagreement or no-valid-sensor latch, return to normal primary A only through that deliberate recovery, reset any enabled filter from current PV, align $I$, establish the current timestamp baseline, remain Manual/Ready, and require a new Auto-enable edge. Model diagnosis must not silently select a sensor. Keep true simulated temperature separate from both sensor values.

## Experiment in Control Studio

1. Reset true temperature and both sensors to 35 °C, SP=35 °C, applied power 75%, valid timestamps, and local integral 75%, plus the full one-second applied-command history 75%. Display both values, ages, timestamps, disagreement timer, selected PV, mode, $I$, request and applied output.
2. Run normal 60 s SP35→38 and separate 60 s 30 kW tests with both sensors fresh and agreeing. Reset fully between cases.
3. Run separate 20 s fault records at fixed SP35 and D=0, with faults from 5 until just before 7 s: A stops acquiring so its last 4.9 s sample becomes stale after .5 s; A reports 90 °C; A reports a fresh +1 °C offset from actual water temperature while B follows actual temperature; and both sensors become invalid. At 5.0 s the offset example begins with 36/35 °C. A last acquisition at 4.9 s remains within the .5 s age bound through 5.4 and becomes stale at 5.5. These planned faults test the selection and recovery logic; they are not abort criteria.
4. At 7 s acknowledge the latched case, provide fresh consistent acquisitions at 7.0, 7.1 and 7.2 s, and issue a new Auto edge at 7.4 s. Record the first request and transfer state. For an ordinary single-sensor fallback, require output continuity within .1 point at the sensor transfer and stay on that source until a deliberate return; the acknowledgement and Auto edge are required after a latched zero-heat fault.
5. Abort only if zero heat is not applied within one update after a latched fault, true temperature leaves 0–80 °C, applied output leaves 0–100%, or the monitor/clock fails. On stop cancel events, command zero heat, restore and fully reset before each case.

## Evidence and decision

A disagreement record must show both fresh timestamps, values, threshold duration and no automatic sensor vote. A single-sensor record must show selected source, P, aligned I and command continuity; a latched-fault record must additionally show qualified recovery and the new Auto edge. The normal records establish control requirements; fault records establish classification and output policy.

If the PV jumps from A=35.2 to B=35 with applied output 75%, the $P$ change from −.4 to 0 must be compensated in $I$, not hidden by changing the gain. If both channels disagree, zero heat is the honest response under this policy even if a model prediction appears plausible.

## Conclusion

Fault-aware redundant sensing separates detectable range/age failures from unresolved truth disagreement. One valid sensor can be selected with bumpless state alignment; two disagreeing fresh sensors require a latched zero-heat recovery rather than an unsupported vote.

## Reason for the next lesson

We can now classify the taught faults and recover control deliberately.
That does not yet establish an independently chosen solution for a complete
application. [Independent design](../../14-integrated-capstones/84-independent/README.md)
asks you to select the necessary taught methods, compare two implementations
and defend a recommendation against the application's requirements.
