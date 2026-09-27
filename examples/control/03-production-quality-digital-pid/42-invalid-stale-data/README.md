# 28. Measurement validity

You will detect invalid or stale temperature measurements, set the heater command to zero, and control when automatic operation can resume.

## The problem

A plausible temperature can still be unusable. A bad quality flag, a nonfinite or out-of-range value, an old acquisition timestamp, or an invalid controller interval must not change proportional or integral state. If automatic control remains selected during that fault, a retained request can continue driving the heater without trustworthy feedback.

## What we need

We need to recognize readings that cannot support control, prevent them
from updating the controller, and define when operation may deliberately
resume from trustworthy data. A plausible displayed value is not sufficient.

## The new idea

**Measurement validity** means deciding whether a reading is suitable for
the control calculation. A plausible value can still be old, and a fresh
value can still have bad quality or be outside the sensor's range.

Measurement age is $a=t_{now}-t_{acquired}$, in seconds. With a 0.5 s limit,
a 0.7 s old reading fails even if it still says 35 °C. Refreshing a display
does not change when the sensor acquired the value.

A **latched fault** remains active until its recovery conditions are met.
Here we remove heat, freeze integral/filter state, and require acknowledgement
and three fresh valid acquisitions before deliberate re-enabling. The code
must apply that fault policy before ordinary PI updates:

```st
MeasurementUsable := MeasurementValid
        AND MeasuredTemperatureC >= 0.0
        AND MeasuredTemperatureC <= 80.0
        AND AcquisitionAgeS >= 0.0
        AND AcquisitionAgeS <= ActiveParameters.MaximumAgeS
        AND ElapsedS > 0.0
        AND ElapsedS <= ActiveParameters.MaximumGapS;

    IF FaultLatched THEN
        AutomaticOwnsOutput := FALSE;
        Mode := HeaterMode#Manual;
        Ready := FALSE;
        RequestedPowerPct := 0.0;
        AutomaticCommandPct := 0.0;
    ELSIF NOT MeasurementUsable THEN
        FaultLatched := TRUE;
        AutomaticOwnsOutput := FALSE;
        Mode := HeaterMode#Manual;
        Ready := FALSE;
        RequestedPowerPct := 0.0;
        AutomaticCommandPct := 0.0;
        RecoveryCount := 0;
    ELSIF AutomaticOwnsOutput THEN
        (* execute the normal Lesson 8 control order *)
    ELSE
        (* use the valid Manual tracking rule from Lesson 26 *)
    END_IF;
```

`MeasurementValid` is the input interface's quality and numeric-validity flag; it must be false for a nonfinite payload. The range and age comparisons provide additional checks. No assignment to I or the filter occurs in the fault branches, so their previous values remain. The calling program's single output selector from Lesson 25 gives `FaultLatched` priority and writes the actual applied command to zero. At fault entry, it also clears the manual request to zero, so clearing the fault cannot restore an old 75% manual command. `HeaterMode#Manual` names the Manual value of the declared mode type. Changing a block's input value alone would not actuate the heater.

The first branch has highest priority on following scans and freezes $I$ and the filter. Outside the state-update branches, calculate the scan interval, then store the current update timestamp for the next scan, while the accepted acquisition timestamp used for recovery must be new. A repeated old timestamp therefore cannot count even when its numeric PV is unchanged; a fresh repeated value can count. Any bad sample resets the consecutive count.

## Prediction

Predict that a 0.7 s old sample fails the .5 s age limit, a negative age fails, and a fresh 35 °C sample with good quality passes even if its value equals the previous sample. Predict zero requested and applied heat within one controller update after fault detection, with $I$ and filtered PV unchanged. Predict that acknowledgement alone does not enable Auto, and that a bad sample resets an incomplete count, or revokes readiness and relatches the fault if qualification has just completed.

At recovery, if applied output is zero and current weighted $P$ is 4 points, $I=0-4=-4$ is correct: it reconstructs the fallback output. A held pre-fault Auto request must not bypass the new explicit enable edge. The eventual temperature may fall while heat is disabled; that is expected and not an accuracy requirement for this fault test.

## Implementation

Use the same controller interface and parameter record as Lessons 25–27. Extend the parameter record with `MaximumAgeS : REAL` and `FreshSamplesRequired : UDINT`. For this exercise, the active record supplies $MaximumAgeS=.5$ s, $MaximumGapS=.25$ s and $FreshSamplesRequired=3$. A sample is valid only when quality is good, PV is finite and in 0–80 °C, acquisition age is $0\le age\le.5$ s, and scan interval is $0<\Delta t\le.25$ s.

Choose one policy: on any fault, freeze the integral and filter state, clear Automatic and Ready, latch the fault, and force requested/applied control to zero heat. The previous scan timestamp is updated every scan so the fault does not repeatedly reuse the same timestamp. Recovery requires acknowledgement, three consecutive valid samples with new acquisition timestamps, filter reset from the current fresh PV, $I=u_{\mathrm{applied}}-P$ using the current weighted $P$, a new current timestamp baseline, and a new explicit Automatic-enable edge. The controller remains Manual/Ready after reset; a held pre-fault Auto request cannot restart it.

Add a recovery counter of type UDINT and compare acquisition timestamps strictly with the last accepted timestamp. On acknowledgement, retain the latched fault until three consecutive new valid acquisitions pass. Then set FilteredTemperatureC to the current fresh PV, calculate current weighted $P$, set $I=AppliedPowerPct-P$, set the controller time baseline to current, clear the fault and enter Manual with Ready true, `AutomaticOwnsOutput` false and the manual request still zero. Only a new accepted Auto request may select the automatic command. Keep the transfer rule from Lesson 26: the first new Auto scan skips ordinary integral integration, then following scans resume normal Lesson 8 order.

Do not use a filtered value to disguise age. Do not integrate over the fault gap. During normal automatic operation, this exercise permits a held measurement while its quality remains valid and its age stays within 0.5 s. Recovery counts only new acquisition timestamps; repeated copies cannot advance the count. Once faulted, ordinary PI remains stopped until the complete recovery sequence finishes.

## Experiment in Control Studio

1. Start at true simulated temperature 35 °C, sensor PV 35 °C, applied power 75%, setpoint 35 °C, valid quality, age zero and controller clock zero, I=75%, P=0%, filtered PV=35 °C and the preceding 1 s command history at 75%. Display true temperature separately from the fault-injected PV, acquisition timestamp, age, quality, fault, Ready, Auto, recovery count, $I$, filter, requested/applied power and revision.
2. In **Control Studio → Test**, review the 20 s sequence and use **Confirm and start** as time zero. Inject bad quality from 5.0 s until just before 7.0 s, or run a separate stale-timestamp case with the same old acquisition time. The planned fault is not an abort criterion.
3. Verify zero requested/applied command within one update, Auto/Ready clear, fault latch set, and $I$/filter frozen. At 7.0 s acknowledge; provide fresh valid acquisitions at 7.0, 7.1 and 7.2 s, all with increasing timestamps. At 7.4 s issue a new Auto-enable edge and inspect the first request.
4. Run a separate isolated counter check with two fresh samples followed by a bad one; the count must return to zero. Also check a bad sample after qualification: readiness must be revoked and the fault latched again. Do not merge it with the primary fault-to-recovery record.
5. Abort only if zero heat is not applied within one update after detection, true temperature leaves 0–80 °C, applied power leaves 0–100%, or the controller clock/output monitor stops updating. On stop cancel Auto, command zero heat, wait for the one-second delivery effect and restoration, then fully reset before repeating. Stop the simulator if the output selector or monitor is unresponsive.

## Evidence and decision

The primary record must show fault priority, zero fallback, frozen state, clock advancement, acknowledgement, three new acquisition timestamps, filter/PV reset, $I=u_{\mathrm{applied}}-P$, Ready/Manual state, and a new Auto edge. A repeated old timestamp cannot satisfy the count. A bad sample resets qualification; after readiness, it revokes that readiness and latches a new fault.

Distinguish age-limit rejection from recovery freshness. The sensor PV can remain numerically 35 °C while true simulated temperature changes after heat is disabled; this does not make the faulty PV trustworthy. The one-second delivery delay can allow earlier nonzero commands to continue delivering heat briefly after the zero command. Do not judge eventual temperature accuracy while the fallback intentionally disables heat.

## Conclusion

Fail-closed measurement handling must take priority over normal PI updates. Freezing state, forcing zero heat, clearing automatic authority and requiring three consecutive new acquisitions prevents a stale integral or held Auto request from restarting control. Recovery is a deliberate sequence, not a single plausible sample.

## Reason for the next lesson

The heater path now combines physical modelling, tuning, mode ownership, transfer, parameter publication and measurement recovery. The commissioning assessment asks the learner to defend a complete decision from those pieces.

[Heater commissioning assessment](../../14-integrated-capstones/81-water-heater/README.md) uses the same evidence boundaries.
